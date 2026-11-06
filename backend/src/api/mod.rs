//! This backend's own HTTP surface, consumed by the Dioxus frontend.
//!
//! Loopback only by default. It holds GLPI OAuth credentials, so exposing it
//! to a network would hand anyone who can reach it the ability to read GLPI
//! through this process.
use std::sync::Arc;

use axum::{
    extract::State,
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::glpi::{
    agents::Agent, computers::Computer, entities::Entity, legacy::Legacy, posture, refused::Db,
    rights::RightsReport, session::SessionInfo, Client,
};

pub struct AppState {
    pub glpi: Client,
    /// GLPI's base URL, echoed to the UI so info panels can link into GLPI's
    /// own screens. Not a secret - it is the address the user already types -
    /// unlike the credentials beside it, which never leave this process.
    pub glpi_url: String,

    /// What this dashboard binds to, reported by the posture page. Loopback
    /// is the default and anything wider is a deliberate choice worth seeing.
    pub bind: String,

    /// The legacy v1 client, used only for what v2.3 does not expose: who may
    /// use which profile, on which entity, recursively or not. See
    /// glpi::legacy for why a second API is tolerated here.
    pub legacy: Legacy,

    /// Direct database access, when configured. None is the normal case and
    /// the app is fully functional without it - only the Refused page needs
    /// it, because that data is in neither API.
    pub db: Option<Db>,
    /// Where the dx-built web bundle lives, if it is being served.
    pub static_dir: Option<std::path::PathBuf>,
    /// Hours without contact before an agent is called stale. 26 rather than
    /// 24: the agent timer runs daily with a RandomizedDelaySec, so a strict
    /// 24 would flag healthy machines whose run drifted by minutes.
    pub stale_after_hours: i64,

    /// Reader for two of GLPI's log files, when configured. None is the
    /// normal case: the Logs page then reports that it has no route to them
    /// rather than rendering empty panes, because an empty log and an
    /// unreadable one mean opposite things.
    pub logs: Option<crate::logs::LogDir>,
}

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/agents", get(agents))
        .route("/api/entities", get(entities))
        .route("/api/computers", get(computers))
        .route("/api/session", get(session))
        .route("/api/authorizations", get(authorizations))
        .route("/api/posture", get(security_posture))
        .route("/api/refused", get(refused))
        .route("/api/field-gaps", get(field_gaps))
        .route("/api/system-status", get(system_status))
        .route("/api/login-sessions", get(login_sessions))
        .route("/api/infocom/{itemtype}/{id}", get(infocom))
        .route("/api/rights", get(rights))
        .route("/api/hardware", get(hardware))
        .route("/api/software", get(software))
        .route("/api/network", get(network))
        .route("/api/logs", get(log_list))
        .route("/api/logs/{file}", get(log_tail))
        .with_state(state.clone());

    match &state.static_dir {
        // fallback_service, not nest("/"): the frontend is a single-page app,
        // so a path it owns client-side must still return index.html rather
        // than 404. ServeDir's own fallback does that; routing it as a nested
        // service would shadow /api instead.
        Some(dir) => api.fallback_service(
            tower::ServiceBuilder::new()
                .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
                    CACHE_CONTROL,
                    cache_control,
                ))
                .service(
                    tower_http::services::ServeDir::new(dir)
                        .fallback(tower_http::services::ServeFile::new(dir.join("index.html"))),
                ),
        ),
        // No bundle built yet: the API still works, which is what `just probe`
        // and curl use. Serving a 404 here is honest - there is nothing to
        // serve - and better than failing to start.
        None => api,
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
    glpi_url: String,
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        glpi_url: state.glpi_url.clone(),
    })
}

/// Errors are returned as 502 with GLPI's own message, not swallowed.
///
/// A dashboard that renders an empty list when its upstream is unreachable
/// is worse than one that says it could not look: an empty agent list and a
/// broken connection mean opposite things.
struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let msg = format!("{:#}", self.0);
        tracing::error!("upstream GLPI error: {msg}");
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

async fn agents(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let raw = state.glpi.get("Inventory/Agent").await?;

    // Computers too, for the third failure state. An agent whose submissions
    // are all refused keeps a current last_contact - GLPI stamps it from the
    // metadata before the import rules run - so the only way to notice is
    // that the asset it points at stopped being updated. See
    // glpi::agents::not_landing.
    //
    // A failure here degrades rather than failing the page: without the
    // computer list every agent simply reports no inventory age, which the
    // check treats as "cannot say" and leaves unflagged. Losing one state is
    // better than losing the fleet view that already works.
    //
    // Live machines only, matching the Computers page. Two reasons, and the
    // second is the one that matters: a trashed asset is not one this agent
    // maintains, so its frozen inventory date would mark the agent as
    // not-landing; and every page here has to mean the same thing by "every
    // machine" or the two disagree about the same fleet.
    let live = crate::glpi::rsql::term("is_deleted", crate::glpi::rsql::Op::Eq, Some("0"));
    let inventory_age: std::collections::HashMap<i64, i64> = match state
        .glpi
        .get(&crate::glpi::rsql::with_filter("Assets/Computer", &live))
        .await
    {
        Ok(raw) => collection(&raw)
            .iter()
            .filter_map(|c| {
                let computer: Computer = serde_json::from_value(c.clone()).ok()?;
                // Re-checked locally, exactly as computers() does and for the
                // same reason: the filter above FAILS OPEN. GLPI drops a term
                // naming a field it does not know and answers 200 with the
                // whole collection, so a renamed `is_deleted` would quietly
                // refill this map with trashed machines' frozen inventory
                // dates - flagging every agent that points at one as
                // not-landing, which is the bug the filter exists to prevent.
                if computer.is_deleted == Some(true) {
                    return None;
                }
                let hours = computer.hours_since_inventory()?;
                Some((computer.id, hours))
            })
            .collect(),
        Err(e) => {
            tracing::warn!("no computer list, so no not-landing detection: {e:#}");
            Default::default()
        }
    };

    let mut out = Vec::new();
    for item in collection(&raw) {
        // Skip rather than fail the whole request: one agent row GLPI
        // reports in an unexpected shape should not blank the dashboard.
        match serde_json::from_value::<Agent>(item.clone()) {
            Ok(a) => {
                // Only an agent attached to a Computer can be matched; GLPI
                // can attach one to other itemtypes, where an id collision
                // across types would otherwise read the wrong asset's age.
                let hours = (a.itemtype.as_deref() == Some("Computer"))
                    .then(|| a.items_id.and_then(|id| inventory_age.get(&id).copied()))
                    .flatten();
                out.push(a.status(state.stale_after_hours, hours));
            }
            Err(e) => tracing::warn!("skipping unparseable agent row: {e}; raw={item}"),
        }
    }

    // Most overdue first: the reason to open this page is to find what has
    // stopped reporting.
    out.sort_by_key(|s| std::cmp::Reverse(s.hours_since_contact.unwrap_or(i64::MAX)));
    Ok(Json(out))
}

/// The rows of a GLPI collection response.
///
/// GLPI returns a bare array. Tolerate an object with a data/items key too,
/// since the HL API's envelope has changed across its own versions and
/// pinning to one shape would break on an upgrade.
fn collection(raw: &serde_json::Value) -> Vec<serde_json::Value> {
    raw.as_array()
        .cloned()
        .or_else(|| raw.get("data").and_then(|d| d.as_array()).cloned())
        .or_else(|| raw.get("items").and_then(|d| d.as_array()).cloned())
        .unwrap_or_default()
}

/// The entities this account can read, so the page can state the scope of
/// everything else it shows. See glpi::entities for why this is on screen at
/// all rather than left implicit.
async fn entities(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let raw = state.glpi.get("Administration/Entity").await?;

    let mut out = Vec::new();
    for item in collection(&raw) {
        // Skip rather than fail: one odd entity row should not cost the page
        // its scope line entirely.
        match serde_json::from_value::<Entity>(item.clone()) {
            Ok(e) => out.push(e),
            Err(e) => tracing::warn!("skipping unparseable entity row: {e}; raw={item}"),
        }
    }

    Ok(Json(out))
}

/// Cache policy for the web bundle.
///
/// The two halves need opposite answers, and getting it wrong is silent.
///
/// index.html is served from stable URLs - `/` and every client-side route -
/// but the filenames inside it change with every build. A cached shell
/// therefore names a *previous* build's .js, which is still sitting in the
/// same directory, so the browser loads the old application successfully.
/// Nothing errors. The new version simply "does not show up", which reads as
/// a broken deployment rather than a cached document.
///
/// Everything it names carries a content hash, so those can be cached
/// forever: a changed file is a changed URL by construction.
fn cache_control<B>(res: &axum::http::Response<B>) -> Option<HeaderValue> {
    let is_shell = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));

    Some(HeaderValue::from_static(if is_shell {
        "no-cache, must-revalidate"
    } else {
        "public, max-age=31536000, immutable"
    }))
}

/// Computers, each marked with whether any agent reports for it.
///
/// Two upstream calls rather than one, and both must succeed. Rendering the
/// computer list with the agent list missing would mark every machine
/// orphaned - the page's alarm state - on nothing worse than a transient
/// failure. An error here is honest; a screen of false alarms is not.
async fn computers(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let raw_agents = state.glpi.get("Inventory/Agent").await?;
    let covered: std::collections::HashSet<i64> = collection(&raw_agents)
        .iter()
        .filter_map(|a| {
            // Only agents attached to a Computer count. GLPI can attach an
            // agent to other itemtypes, and an id collision across types
            // would otherwise clear the wrong machine.
            let is_computer = a.get("itemtype").and_then(|v| v.as_str()) == Some("Computer");
            is_computer
                .then(|| a.get("items_id").and_then(|v| v.as_i64()))
                .flatten()
        })
        .collect();

    // Soft-deleted computers are excluded, by GLPI rather than here.
    //
    // GLPI has no default restriction on `is_deleted` - there is none in
    // Search.php or ResourceAccessor.php, and the schemas expose it as an
    // ordinary boolean - so an unfiltered Assets/Computer returns machines
    // sitting in GLPI's trash alongside live ones. That mattered more than a
    // stray row: a deleted machine has no agent reporting for it, so it would
    // be marked `orphaned`, which is the single state this page exists to
    // raise. Deleting a machine on purpose would have produced the page's
    // loudest alarm.
    //
    // `==0` and not `=isnottruthy=`: the column is `tinyint NOT NULL DEFAULT
    // '0'` in every definition in install/mysql, so there is no NULL to
    // catch, and `==0` works on 11.0.12 as well - `=isnottruthy=` is new in
    // 12.0.1. Equivalent here, and portable.
    let live = crate::glpi::rsql::term("is_deleted", crate::glpi::rsql::Op::Eq, Some("0"));
    let raw = state
        .glpi
        .get(&crate::glpi::rsql::with_filter("Assets/Computer", &live))
        .await?;

    let mut out = Vec::new();
    let mut filter_was_ignored = 0usize;
    for item in collection(&raw) {
        match serde_json::from_value::<Computer>(item.clone()) {
            Ok(c) => {
                // The same exclusion again, locally, because the server-side
                // filter FAILS OPEN.
                //
                // GLPI silently drops a filter term naming a field it does not
                // know and runs the query without it: `Search.php:443` pulls
                // `Error::UNKNOWN_PROPERTY` out of the set it throws on, so
                // only an unknown OPERATOR or a mapped property raise
                // RSQLException. Measured, not read: on this install
                // `filter=is_deletedX==0` returns the full collection with a
                // 200, identical to sending no filter at all.
                //
                // So if `is_deleted` is ever renamed, removed, or mistyped
                // here, GLPI hands back every machine including the trashed
                // ones and says nothing - reinstating the exact bug the filter
                // was added to fix. This guard makes that degrade to
                // "correct but one wasted round trip" instead of "wrong".
                if c.is_deleted == Some(true) {
                    filter_was_ignored += 1;
                    continue;
                }
                out.push(c.status(&covered))
            }
            Err(e) => tracing::warn!("skipping unparseable computer row: {e}; raw={item}"),
        }
    }

    // Count what was left out, and say so. Filtering silently is the same
    // mistake as rendering an empty list for an unreachable upstream: the
    // reader cannot tell a small fleet from a filtered one.
    //
    // A second walk, of a set that is normally empty - zero on this install.
    // A failure here degrades to "could not count" rather than failing the
    // page, because the list above is the answer and this is a footnote.
    let deleted_filter =
        crate::glpi::rsql::term("is_deleted", crate::glpi::rsql::Op::Ne, Some("0"));
    let deleted_excluded = match state
        .glpi
        .get(&crate::glpi::rsql::with_filter(
            "Assets/Computer",
            &deleted_filter,
        ))
        .await
    {
        // Counted locally for the same reason, and it matters more here: an
        // ignored filter returns the WHOLE collection, so trusting the row
        // count would report every machine in the fleet as deleted. Counting
        // the ones that actually say so turns that into a correct zero.
        Ok(v) => {
            let rows = collection(&v);
            // "Nobody said deleted" and "nobody could say" are different
            // answers. is_deleted carries x-rights-conditions like any other
            // field - the whole point of the field-gaps check is that GLPI
            // drops fields the account may not read from a successful 200 -
            // and the legacy API serves these tinyints as 0/1 rather than as
            // JSON booleans. Either way the filter below matches nothing and
            // the count would be a confident zero, which the Computers page
            // renders as "no machines were hidden": the one reading the
            // Option exists to prevent. Absent the field entirely, say None.
            let usable = rows
                .iter()
                .any(|item| item.get("is_deleted").is_some_and(|v| v.is_boolean()));
            if rows.is_empty() || usable {
                Some(
                    rows.iter()
                        .filter(|item| {
                            item.get("is_deleted").and_then(serde_json::Value::as_bool)
                                == Some(true)
                        })
                        .count(),
                )
            } else {
                tracing::warn!(
                    "is_deleted absent or not a boolean on {} rows; not claiming a count",
                    rows.len()
                );
                None
            }
        }
        Err(e) => {
            tracing::warn!("could not count deleted computers: {e:#}");
            None
        }
    };

    // Worth knowing about, because it means an upgrade moved a field name out
    // from under the filter. Not an error: the guard above already made the
    // answer correct, so this is a note rather than a failure.
    if filter_was_ignored > 0 {
        tracing::warn!(
            "GLPI returned {filter_was_ignored} soft-deleted computer(s) despite the \
             is_deleted filter, so it silently ignored the filter term - excluded them here \
             instead. Check that Assets/Computer still declares `is_deleted`."
        );
    }

    // Orphans first, then oldest inventory: both are reasons to look, and an
    // orphan is the worse one.
    out.sort_by_key(|c| {
        (
            std::cmp::Reverse(c.orphaned),
            std::cmp::Reverse(c.hours_since_inventory.unwrap_or(i64::MAX)),
        )
    });
    Ok(Json(ComputerReport {
        computers: out,
        deleted_excluded,
    }))
}

#[derive(Serialize)]
struct ComputerReport {
    computers: Vec<crate::glpi::computers::ComputerStatus>,
    /// How many soft-deleted machines were filtered out, or `None` if that
    /// could not be established. `None` is not zero: the page says "could not
    /// count" rather than implying nothing was hidden.
    deleted_excluded: Option<usize>,
}

/// The computers this account can see, as id -> name.
///
/// Every inventory page is scoped by this rather than by what v1 happens to
/// return. v1 applies the same profile and entity filtering, so the two
/// should agree - but "should agree" is not the same as "agrees", and a page
/// built from the v1 tables alone would quietly widen if they ever diverged.
/// Taking the fleet from v2.3 means Hardware, Software, Network and Computers
/// all mean the same thing by "every machine".
///
/// Which is why the trash exclusion belongs here too. Computers and Agents
/// both drop deleted machines now; without the same cut, deleting a computer
/// in GLPI would remove it from those two pages while its disks, software and
/// network ports went on being listed as live fleet inventory - the three
/// /inv pages disagreeing with the rest about the same word.
async fn visible_computers(
    state: &AppState,
) -> anyhow::Result<std::collections::HashMap<i64, String>> {
    let live = crate::glpi::rsql::term("is_deleted", crate::glpi::rsql::Op::Eq, Some("0"));
    let raw = state
        .glpi
        .get(&crate::glpi::rsql::with_filter("Assets/Computer", &live))
        .await?;
    Ok(collection(&raw)
        .iter()
        .filter_map(|c| {
            let computer: Computer = serde_json::from_value(c.clone()).ok()?;
            // And again locally, because the filter fails open. See
            // computers() for what GLPI does with a term it cannot resolve.
            if computer.is_deleted == Some(true) {
                return None;
            }
            // Named for the reader, not for GLPI: an unnamed asset still has
            // to be findable in the table.
            let name = computer
                .name
                .unwrap_or_else(|| format!("computer {}", computer.id));
            Some((computer.id, name))
        })
        .collect())
}

/// Run `f` against v1 inside one session.
///
/// One session for the whole page, not one per collection: initSession is a
/// full GLPI login and the three inventory pages each read between three and
/// ten collections.
async fn with_v1<T, F, Fut>(state: &AppState, f: F) -> anyhow::Result<T>
where
    F: FnOnce(crate::glpi::legacy::Session) -> Fut,
    Fut: std::future::Future<Output = (crate::glpi::legacy::Session, anyhow::Result<T>)>,
{
    state.legacy.with_session(f).await
}

/// What the agents found inside each machine.
async fn hardware(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let computers = visible_computers(&state).await?;
    let legacy = &state.legacy;
    let out = with_v1(&state, |session| async move {
        let out = crate::glpi::inventory::hardware(legacy, &session, &computers).await;
        (session, out)
    })
    .await?;
    Ok(Json(out))
}

/// Every software title installed across the fleet.
async fn software(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let computers = visible_computers(&state).await?;
    let legacy = &state.legacy;
    let out = with_v1(&state, |session| async move {
        let out = crate::glpi::inventory::software(legacy, &session, &computers).await;
        (session, out)
    })
    .await?;
    Ok(Json(out))
}

/// Every machine's network ports and addresses.
async fn network(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let computers = visible_computers(&state).await?;
    let legacy = &state.legacy;
    let out = with_v1(&state, |session| async move {
        let out = crate::glpi::inventory::network(legacy, &session, &computers).await;
        (session, out)
    })
    .await?;
    Ok(Json(out))
}

/// The GLPI identity this app is actually using.
///
/// Worth a page of its own: a 403 from the HL API never names the missing
/// right, and GLPI activates exactly one profile per session regardless of
/// how many authorizations the account holds.
async fn session(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let raw = state.glpi.get("session").await?;
    Ok(Json(SessionInfo::from_value(&raw)))
}

/// How many ProfileRight rows to ask v1 for.
///
/// 899 on a stock install - nine profiles at about a hundred rights each - so
/// this is roughly a tenfold margin. It is not a target.
///
/// Two independent guards sit behind it, and both are deliberate. `Legacy::get`
/// refuses any collection v1 served only partly, which is the real protection
/// and catches every call. `from_legacy` separately refuses a row count that
/// reaches this limit, because it is a pure function that must not hand back a
/// short rights list whatever it is fed - a right missing because it was never
/// fetched reads exactly like a right that was never granted.
const RIGHT_ROW_LIMIT: usize = 10_000;

/// Every right of every profile in GLPI, not just the session's own.
///
/// Runs against v1, because v2.3 cannot answer this: `ProfileRight` has no HL
/// route, and the session blob covers only the active profile - and reports 21
/// ordinary profile columns as rights with value 0 while doing it. Both are
/// laid out in glpi::rights.
async fn rights(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    // Which profile this app is actually working under. Read from v2.3, which
    // is the API every other page uses, so this reports the session those
    // pages really run in rather than the separate v1 one opened below.
    let active_profile_id = state
        .glpi
        .get("session")
        .await?
        .get("active_profile")
        .and_then(|p| p.get("id"))
        .and_then(|x| x.as_i64());

    // A second handle, so the closure can own one while with_session borrows
    // the other. Arc::clone is a refcount bump, not a copy of the client.
    let inner = Arc::clone(&state);
    let report = state
        .legacy
        .with_session(|session| async move {
            let out = collect_rights(&inner.legacy, &session, active_profile_id).await;
            (session, out)
        })
        .await?;
    Ok(Json(report))
}

async fn collect_rights(
    legacy: &Legacy,
    session: &crate::glpi::legacy::Session,
    active_profile_id: Option<i64>,
) -> anyhow::Result<RightsReport> {
    let arr = |v: serde_json::Value| v.as_array().cloned().unwrap_or_default();

    // Explicit ranges: v1 applies a default limit and truncates past it -
    // measured, a rangeless ProfileRight returns 15 of 899 rows. These are
    // generous rather than tight because Legacy::get now REFUSES a truncated
    // collection, so a range that turns out too small fails loudly and names
    // the number to raise instead of shortening a list.
    let profiles = arr(legacy.get(session, "Profile", Some("0-999")).await?);
    let range = format!("0-{}", RIGHT_ROW_LIMIT - 1);
    let rights = arr(legacy.get(session, "ProfileRight", Some(&range)).await?);

    RightsReport::from_legacy(&profiles, &rights, active_profile_id, RIGHT_ROW_LIMIT)
}

/// One authorization: a profile this account may use, on one entity.
#[derive(Serialize)]
struct Authorization {
    entity: String,
    profile: String,
    /// "central" or "helpdesk". The rights this app needs cannot exist on a
    /// helpdesk profile at all - the section is absent from GLPI's form.
    interface: String,
    /// Whether entities below this one are included. Not visible through the
    /// v2.3 API for any account, including the caller's own.
    is_recursive: bool,
    /// GLPI activates exactly one profile per session, so this is the one
    /// actually in force; the others are merely available.
    is_active: bool,
    right_agent: i64,
    right_entity: i64,
}

/// Every authorization this account holds, with the two rights each profile
/// grants.
///
/// The question the Status page could not previously answer: not "something
/// is missing" but "this profile, on this entity, is the one to fix". It
/// needs four collections that v2.3 does not expose, so it runs against v1 -
/// see glpi::legacy.
async fn authorizations(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    // A second handle, so the closure can own one while with_session borrows
    // the other. Arc::clone is a refcount bump, not a copy of the client.
    let inner = Arc::clone(&state);
    let rows: Vec<Authorization> = state
        .legacy
        .with_session(|session| async move {
            let out = collect_authorizations(&inner.legacy, &session).await;
            (session, out)
        })
        .await?;
    Ok(Json(rows))
}

async fn collect_authorizations(
    legacy: &Legacy,
    session: &crate::glpi::legacy::Session,
) -> anyhow::Result<Vec<Authorization>> {
    use anyhow::Context;

    let full = legacy.get(session, "getFullSession", None).await?;
    let me = full
        .get("session")
        .and_then(|s| s.get("glpiID"))
        .and_then(|v| v.as_i64())
        .context("getFullSession returned no glpiID")?;
    let active_profile = full
        .get("session")
        .and_then(|s| s.get("glpiactiveprofile"))
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_i64());

    // Explicit ranges throughout, for the reason in collect_rights: v1 limits
    // by default, and Legacy::get refuses anything it served only partly.
    let arr = |v: serde_json::Value| v.as_array().cloned().unwrap_or_default();
    let links = arr(legacy.get(session, "Profile_User", Some("0-999")).await?);
    let profiles = arr(legacy.get(session, "Profile", Some("0-999")).await?);
    let entities = arr(legacy.get(session, "Entity", Some("0-999")).await?);
    let rights = arr(legacy.get(session, "ProfileRight", Some("0-9999")).await?);

    let num = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_i64());
    let text = |v: &serde_json::Value, k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };

    let right_for = |profile_id: i64, name: &str| -> i64 {
        rights
            .iter()
            .find(|r| num(r, "profiles_id") == Some(profile_id) && text(r, "name") == name)
            .and_then(|r| num(r, "rights"))
            .unwrap_or(0)
    };

    let mut out = Vec::new();
    for link in links.iter().filter(|l| num(l, "users_id") == Some(me)) {
        let Some(pid) = num(link, "profiles_id") else {
            continue;
        };
        let eid = num(link, "entities_id");

        let profile = profiles.iter().find(|p| num(p, "id") == Some(pid));
        let entity = entities.iter().find(|e| num(e, "id") == eid);

        out.push(Authorization {
            entity: entity
                .map(|e| {
                    let c = text(e, "completename");
                    if c.is_empty() {
                        text(e, "name")
                    } else {
                        c
                    }
                })
                .unwrap_or_else(|| format!("entity {}", eid.unwrap_or(-1))),
            profile: profile
                .map(|p| text(p, "name"))
                .unwrap_or_else(|| format!("profile {pid}")),
            interface: profile.map(|p| text(p, "interface")).unwrap_or_default(),
            // v1 sends 1/0, not a JSON bool.
            is_recursive: num(link, "is_recursive").unwrap_or(0) != 0,
            is_active: Some(pid) == active_profile,
            right_agent: right_for(pid, "agent"),
            right_entity: right_for(pid, "entity"),
        });
    }

    // The profile actually in force first; it is the one a reader is looking
    // for when something is refused.
    out.sort_by_key(|a| std::cmp::Reverse(a.is_active));
    Ok(out)
}

/// Security posture: four tiles, each judged by glpi::posture from data GLPI
/// already reports.
///
/// One handler rather than four, and one legacy session rather than several:
/// the tiles are read together, and GLPI allocates server-side state per
/// session that must be handed back.
async fn security_posture(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    // v2.3 first - if these fail the whole page is wrong anyway, and failing
    // before opening a legacy session means there is none to leak.
    let users_raw = state.glpi.get("Administration/User").await?;
    let agents_raw = state.glpi.get("Inventory/Agent").await?;
    let session_raw = state.glpi.get("session").await?;
    let active_profile_id = session_raw
        .get("active_profile")
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_i64());

    let inner = Arc::clone(&state);
    let (report, auths, config) = state
        .legacy
        .with_session(|session| async move {
            let out = async {
                let report = collect_rights(&inner.legacy, &session, active_profile_id).await?;
                let auths = collect_authorizations(&inner.legacy, &session).await?;
                let config = inner.legacy.get(&session, "getGlpiConfig", None).await?;
                anyhow::Ok((report, auths, config))
            }
            .await;
            (session, out)
        })
        .await?;

    let cfg = config.get("cfg_glpi");
    let cfg_str = |k: &str| cfg.and_then(|c| c.get(k)).and_then(|v| v.as_str());
    // GLPI returns config values as STRINGS, including numeric ones:
    // system_user comes back as "6", not 6. Taking as_i64() alone yields None
    // and the caller silently loses the setting - which is how GLPI's own
    // system account stayed flagged on this page after being excluded.
    let cfg_i64 = |k: &str| {
        cfg.and_then(|c| c.get(k)).and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        })
    };
    // GLPI writes these as 1/0, and has used strings for them historically.
    let cfg_bool = |k: &str| {
        cfg.and_then(|c| c.get(k)).and_then(|v| {
            v.as_bool()
                .or_else(|| v.as_i64().map(|n| n != 0))
                .or_else(|| v.as_str().map(|s| s != "0" && !s.is_empty()))
        })
    };

    let active = report.profiles.iter().find(|p| p.is_active);
    let write_rights: Vec<String> = active
        .map(|p| {
            p.entries
                .iter()
                // `writes` travels with the bit on purpose - see RightBit.
                // Re-deriving it from the right's name here would be the
                // exact split that comment warns about.
                .filter(|e| e.bits.iter().any(|b| b.writes))
                .map(|e| e.name.clone())
                .collect()
        })
        .unwrap_or_default();

    let spare: Vec<String> = auths
        .iter()
        .filter(|a| !a.is_active)
        .map(|a| format!("{} on {}", a.profile, a.entity))
        .collect();

    let access = posture::access(&posture::AccessInput {
        profile_name: active.and_then(|p| p.name.as_deref()),
        interface: active.and_then(|p| p.interface.as_deref()),
        write_rights,
        spare_authorizations: spare,
    });

    let users: Vec<posture::UserRow> = collection(&users_raw)
        .iter()
        .map(|u| {
            let text = |k: &str| u.get(k).and_then(|v| v.as_str()).map(str::to_string);
            let flag = |k: &str| u.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
            posture::UserRow {
                id: u.get("id").and_then(|v| v.as_i64()).unwrap_or(-1),
                name: text("username")
                    .or_else(|| text("name"))
                    .unwrap_or_default(),
                is_active: flag("is_active"),
                is_deleted: flag("is_deleted"),
                last_login: text("last_login"),
                password_changed: text("date_password_change"),
            }
        })
        .collect();

    let agents: Vec<(String, Option<String>)> = collection(&agents_raw)
        .iter()
        .map(|a| {
            let text = |k: &str| a.get(k).and_then(|v| v.as_str()).map(str::to_string);
            (
                text("name")
                    .or_else(|| text("deviceid"))
                    .unwrap_or_default(),
                text("version"),
            )
        })
        .collect();

    Ok(Json(vec![
        access,
        posture::accounts(&users, cfg_i64("system_user")),
        posture::exposure(&posture::ExposureInput {
            bind: &state.bind,
            url_base: cfg_str("url_base"),
            enable_api: cfg_bool("enable_api"),
            enable_api_login_credentials: cfg_bool("enable_api_login_credentials"),
        }),
        posture::versions(cfg_str("version"), &agents),
    ]))
}

/// What this app can say about refused submissions, including "nothing,
/// because no database access is configured".
#[derive(Serialize)]
struct RefusedReport {
    /// False when GLPI_DB_URL is unset. The page says so rather than showing
    /// an empty table: "no refusals" and "cannot look" are opposite answers
    /// and must not render the same.
    configured: bool,
    /// Entities the rows were restricted to, so the page can state its scope
    /// the way every other page does.
    entities: Vec<i64>,
    rows: Vec<crate::glpi::refused::Refused>,
}

#[derive(Serialize)]
struct FieldGapReport {
    /// Whether GLPI served a usable schema document. False is a finding, not
    /// an error: an install that does not serve `doc.json` cannot be checked
    /// this way, and saying so beats reporting every endpoint as clean.
    doc_available: bool,
    reports: Vec<crate::glpi::schema_check::SchemaReport>,
    /// Every schema the document declares, counted but not compared. Carried
    /// in the same response because it comes out of the same `doc.json`, and
    /// a second endpoint would mean a second 4.6 MB fetch for data already in
    /// hand.
    catalogue: Vec<crate::glpi::schema_check::SchemaSummary>,
    /// How many operations the document says accept an RSQL filter. Also off
    /// the same `doc.json`, and the figure the RSQL reference needs so it can
    /// state its reach rather than assert it.
    filter_reach: crate::glpi::schema_check::FilterReach,
}

/// Fields the API's own schema promises that this account does not receive.
///
/// GLPI's `x-rights-conditions` drop a field the caller may not read instead
/// of refusing the call, and 12.0.1 adds ten more of them than 11.0.12 has.
/// An absent field is indistinguishable from a null one on a UI whose fields
/// are all `Option`, so without this the suppression is invisible. See
/// glpi::schema_check.
///
/// COST. `doc.json` is about 4.6 MB on 12.0.1 and is fetched per request, so
/// this is deliberately a diagnostic endpoint rather than something a page
/// polls. Caching it would mean holding a copy that goes stale exactly when it
/// matters - across an upgrade - which is the moment this answer has to be
/// current.
async fn field_gaps(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    use crate::glpi::schema_check as sc;

    let doc = state.glpi.get("doc.json").await?;

    // Probe with a schema every supported version declares. If that is absent
    // the document is not what we think it is, and comparing against it would
    // report every field of every endpoint as suppressed.
    if sc::schema_from_doc(&doc, "User").is_none() {
        return Ok(Json(FieldGapReport {
            doc_available: false,
            reports: Vec::new(),
            catalogue: Vec::new(),
            filter_reach: Default::default(),
        }));
    }

    let mut reports = Vec::with_capacity(sc::AUDITED.len());
    for (path, schema_name) in sc::AUDITED {
        let received = state.glpi.get(path).await?;
        let declared = sc::schema_from_doc(&doc, schema_name)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        reports.push(sc::compare(schema_name, &declared, &received));
    }

    Ok(Json(FieldGapReport {
        doc_available: true,
        reports,
        catalogue: sc::catalogue(&doc),
        filter_reach: sc::filter_reach(&doc),
    }))
}

/// GLPI's own health, including the automatic actions that quietly stopped.
///
/// The same failure this dashboard was built for, one layer down: a scheduled
/// job that stopped running looks exactly like one with nothing to do. GLPI
/// does report it and nothing here read it until now. See
/// glpi::system_status for the three ways this endpoint misleads a careless
/// reader.
async fn system_status(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(crate::glpi::system_status::fetch(&state.glpi).await?))
}

/// Who is currently inside GLPI, and whether that list is the whole truth.
///
/// New in 12.0.1. `/All` answers 200 whether or not this account may see
/// every user's sessions, narrowing silently when it may not, so the report
/// carries the scope it was actually given - see glpi::sessions.
async fn login_sessions(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(crate::glpi::sessions::fetch(&state.glpi).await?))
}

/// One asset's financial and administrative dates.
///
/// Per asset by necessity: Infocom has no collection route under Assets, so
/// there is no fleet view that would not cost a request per machine. A 404
/// from GLPI means the asset has no financial record - normal for a machine
/// created by inventory - and is reported as an absence rather than an error.
/// Returns a `Response` rather than `impl IntoResponse` so the bad-itemtype
/// branch can answer 400. `ApiError` is deliberately 502 - it exists to carry
/// GLPI's own failures through - and using it for a caller's mistake would
/// blame the upstream for a bad request.
async fn infocom(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((itemtype, id)): axum::extract::Path<(String, i64)>,
) -> Result<axum::response::Response, ApiError> {
    // Whitelisted against GLPI's own list rather than passed through. The
    // itemtype lands in a URL path, and GLPI's route only accepts these -
    // anything else would be a 404 from GLPI that reads like a broken
    // endpoint rather than a bad request.
    const ASSET_TYPES: &[&str] = &[
        "Cartridge",
        "CartridgeItem",
        "Consumable",
        "ConsumableItem",
        "Computer",
        "Monitor",
        "NetworkEquipment",
        "Peripheral",
        "Phone",
        "Printer",
        "Software",
        "Certificate",
        "Appliance",
        "Rack",
        "Enclosure",
        "PDU",
        "PassiveDCEquipment",
        "Cable",
    ];
    if !ASSET_TYPES.contains(&itemtype.as_str()) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("itemtype {itemtype} does not carry Infocom"),
                "accepts": ASSET_TYPES,
            })),
        )
            .into_response());
    }

    Ok(Json(crate::glpi::infocom::fetch(&state.glpi, &itemtype, id).await?).into_response())
}

#[derive(Serialize)]
struct LogListResponse {
    configured: bool,
    /// Every *.log GLPI's directory actually holds. Empty with `error` set
    /// when the directory cannot be listed - distinct from an empty
    /// directory, which is a real and different answer.
    entries: Vec<crate::logs::Entry>,
    error: Option<String>,
}

/// What logs exist, rather than what this app guessed would exist.
///
/// Listing is deliberately separate from reading: the names and sizes of
/// every log are cheap and harmless, while the contents are served for two
/// files only. GLPI's own viewer works the same way round - `LogParser`
/// scandirs the directory, then shows one file on request.
async fn log_list(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let Some(dir) = state.logs.as_ref() else {
        return Ok(Json(LogListResponse {
            configured: false,
            entries: Vec::new(),
            error: None,
        }));
    };

    Ok(Json(match dir.list() {
        Ok(entries) => LogListResponse {
            configured: true,
            entries,
            error: None,
        },
        Err(e) => LogListResponse {
            configured: true,
            entries: Vec::new(),
            error: Some(e),
        },
    }))
}

#[derive(Serialize)]
struct LogResponse {
    /// False when GLPI_INV_LOG_DIR is unset. The UI distinguishes this from
    /// a configured-but-unreadable directory, which is a different fix.
    configured: bool,
    #[serde(flatten)]
    tail: Option<crate::logs::Tail>,
    /// The names this endpoint will serve, so the UI need not hardcode them.
    allowed: Vec<&'static str>,
}

/// The tail of one of GLPI's own log files.
///
/// `file` is matched against an allowlist inside logs::LogDir, not joined
/// onto the directory and hoped for - a path parameter that reaches the
/// filesystem is how a diagnostic view becomes an arbitrary file read.
async fn log_tail(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let allowed = crate::logs::ALLOWED.to_vec();

    let Some(dir) = state.logs.as_ref() else {
        return Ok(Json(LogResponse {
            configured: false,
            tail: None,
            allowed,
        }));
    };

    Ok(Json(LogResponse {
        configured: true,
        tail: Some(dir.tail(&file)),
        allowed,
    }))
}

/// Submissions GLPI refused - the third failure, invisible to both APIs.
async fn refused(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(RefusedReport {
            configured: false,
            entities: Vec::new(),
            rows: Vec::new(),
        }));
    };

    // Scope comes from the API, not the database. The database account has no
    // notion of entities and could read every row; restricting to what the
    // API already shows keeps this page meaning the same thing by
    // "everything" as the others.
    let raw = state.glpi.get("Administration/Entity").await?;
    let entities: Vec<i64> = collection(&raw)
        .iter()
        .filter_map(|e| e.get("id").and_then(|v| v.as_i64()))
        .collect();

    let rows = db.refused(&entities).await?;
    Ok(Json(RefusedReport {
        configured: true,
        entities,
        rows,
    }))
}
