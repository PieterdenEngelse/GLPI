//! This backend's own HTTP surface, consumed by the Dioxus frontend.
//!
//! Loopback only by default. It holds GLPI OAuth credentials, so exposing it
//! to a network would hand anyone who can reach it the ability to read GLPI
//! through this process.
use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Path, Query, Request, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::IntoResponse,
    routing::{any, get, post},
    Json, Router,
};
use serde::Serialize;

pub mod timing;

use crate::glpi::{
    agents::Agent, computers::Computer, entities::Entity, legacy::Legacy, posture, refused,
    refused::Db, rights::RightsReport, session::SessionInfo, Client,
};

pub struct AppState {
    /// The Nix file the nginx page writes, when one is configured. Absent
    /// means the page shows its settings and explains that nothing can be
    /// changed until the one-time setup is done.
    pub nginx_tuning: Option<crate::glpi::nginx_tuning::TuningFile>,
    pub nginx_writes: bool,

    /// How long this process took to answer each of its own routes. Written
    /// by the middleware below on every /api request, read by one endpoint.
    pub timings: timing::Timings,

    /// When this process started, so the timings can say what window their
    /// request counts cover.
    pub started: std::time::Instant,

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

    /// Whether SET GLOBAL on an InnoDB setting is permitted. See
    /// config::Config::innodb_writes - off by default, and checked in the
    /// handler rather than only in the UI.
    pub innodb_writes: bool,

    /// Whether SET GLOBAL on ANY dynamic variable is permitted. See
    /// config::Config::variable_writes - a separate and much wider flag than
    /// innodb_writes, off by default, and checked in the handler.
    pub variable_writes: bool,

    /// Whether the restart links may run scripts/db-restart.sh. A separate
    /// flag from innodb_writes, and a bigger one: see config::Config.
    pub db_restart: bool,

    /// The startup file, when one is configured. See
    /// glpi::init_file - it is what makes a change outlive a restart, and
    /// its absence is why the panel otherwise says a change does not.
    pub init: Option<crate::glpi::init_file::InitFile>,
}

impl AppState {
    /// Whether the InnoDB panel may write.
    ///
    /// EITHER flag, and that is not a widening of the narrow one. The InnoDB
    /// routes reach exactly the dynamic settings under `innodb%` - 106 on
    /// this server. `variable_writes` reaches all 484 dynamic variables,
    /// which strictly contains them: with it set, every one of those 106 is
    /// already writable through `/api/mariadb/variables/{name}`. Refusing
    /// them here therefore bought no safety at all - it only made the same
    /// variable editable on one page and read-only on another, which reads
    /// as a broken panel rather than as a permission.
    ///
    /// The reverse does NOT hold and must never be added: `innodb_writes`
    /// says nothing about the other 378 and is not consent for them.
    pub fn innodb_writes_allowed(&self) -> bool {
        self.innodb_writes || self.variable_writes
    }
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
        .route("/api/perf", get(perf))
        .route("/api/mariadb", get(mariadb))
        .route("/api/mariadb/innodb", get(innodb))
        .route("/api/mariadb/plugins", get(mariadb_plugins))
        .route("/api/mariadb/attributes", get(mariadb_attributes))
        .route("/api/mariadb/slow-log", get(slow_log))
        .route("/api/mariadb/slow-log/entries", get(slow_log_entries))
        .route("/api/mariadb/history", get(status_history))
        .route("/api/php/history", get(php_history))
        .route("/api/nginx", get(nginx))
        .route("/api/nginx/tuning", get(nginx_tuning))
        .route("/api/nginx/tuning/{name}", post(set_nginx_tuning))
        .route("/api/self-timing", get(self_timing))
        .route("/api/mariadb/variables", get(variables))
        .route("/api/mariadb/variables/{name}", post(set_variable))
        .route(
            "/api/mariadb/variables/{name}/persist",
            post(persist_variable),
        )
        .route("/api/mariadb/innodb/{name}", post(set_innodb))
        .route("/api/mariadb/innodb/{name}/persist", post(persist_innodb))
        // The two restart links. One route, because what differs between
        // them is a mode the script understands and this process does not
        // interpret.
        .route("/api/db/restart", post(db_restart))
        .route("/api/field-gaps", get(field_gaps))
        .route("/api/system-status", get(system_status))
        .route("/api/login-sessions", get(login_sessions))
        .route("/api/infocom/{itemtype}/{id}", get(infocom))
        .route("/api/rights", get(rights))
        .route("/api/rules", get(rules))
        .route("/api/hardware", get(hardware))
        .route("/api/software", get(software))
        .route("/api/network", get(network))
        .route("/api/logs", get(log_list))
        .route("/api/logs/{file}", get(log_tail))
        // Last, and the reason is a bug this cost an hour of: without it an
        // unknown /api path falls through to the SPA fallback below and comes
        // back as index.html with 200, because a path the frontend owns
        // client-side has to. The frontend then fails to parse a web page as
        // JSON and reports "error decoding response body" - which names the
        // symptom and hides the cause, since the backend was reached and
        // answered perfectly. Any method, so a mistyped POST says the same.
        //
        // matchit prefers literal and named segments over a wildcard, so
        // every route above still wins; this only catches what nothing
        // matched. /api itself is spelled separately because {*rest} needs at
        // least one segment after the slash.
        .route("/api", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        // On the api router only, so serving the wasm bundle and every
        // asset beside it is not timed: those are ServeDir reading a file
        // and say nothing about what this process does.
        //
        // Applied after the routes, which is what puts it INSIDE routing -
        // MatchedPath only exists once the router has matched, and without
        // it the key would be the request URI.
        .layer(middleware::from_fn_with_state(state.clone(), record_timing))
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

/// Every /api path nothing else claimed.
///
/// 404 with a JSON body, so a caller that expected JSON gets JSON and a
/// caller reading the status sees the truth. The SPA fallback cannot do this
/// job: it has to answer index.html for unknown paths, because the frontend
/// owns its own routes client-side.
async fn api_not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("no such endpoint: {}", uri.path()),
        })),
    )
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
struct MariadbReport {
    /// False when GLPI_DB_URL is unset, exactly as the refused report uses
    /// it: "not configured" and "nothing to report" must not render alike.
    configured: bool,
    facts: Option<crate::glpi::refused::ServerFacts>,
}

/// The rule collections this page tiles, counted.
///
/// Four of GLPI's nine rather than all of them, because each one costs a walk
/// of its rules: the three it calls business rules - RuleTicket, RuleChange,
/// RuleProblem - and the import collection, which is the one the rest of this
/// app is really about. The other five are reachable the same way if a tile
/// ever wants them.
#[derive(serde::Serialize)]
struct RulesReport {
    /// False when GLPI did not serve the rule collections at all - an install
    /// whose profile cannot read rules, or a version without the controller.
    /// Distinct from three empty collections, which is a real answer.
    available: bool,
    collections: Vec<RuleCollection>,
    /// Collections GLPI declined to serve, by rule type.
    ///
    /// A 403 rather than an absence: the route is there and the account's
    /// profile lacks READ on that rule type, so the rules exist and are not
    /// countable here. Reported rather than dropped - a rule collection this
    /// app cannot see still runs on the install, and silence would read as
    /// "none", which is the one wrong answer.
    refused: Vec<String>,
}

#[derive(serde::Serialize)]
struct RuleCollection {
    /// GLPI's own identifier for the collection, as the path takes it.
    rule_type: String,
    /// GLPI's label, taken from the response rather than written here: it is
    /// translated, so the install's own wording is the only right one.
    name: String,
    total: usize,
    /// Counted from each rule's `is_active`, because a rule that exists and is
    /// switched off fires on nothing, and a bare total would hide that.
    active: usize,
    /// Rules whose action sets `_ignore_import`, which is how an import rule
    /// refuses a submission rather than turning it into an asset. Only the
    /// import collection has any; it is zero everywhere else, and that is the
    /// honest answer rather than a field that does not apply.
    denies: usize,
    /// Rules carrying GLPI's "Skip remaining rules" action,
    /// `_stop_rules_processing`. The mirror of `denies`: `Rule::getActions`
    /// offers that action only when the collection does NOT stop at the first
    /// match, so this can be non-zero for the business collections and is
    /// structurally impossible for the import one.
    stops: usize,
    /// The rules themselves, in GLPI's ranking order.
    ///
    /// Free: the handler already walks every rule to count the three figures
    /// above, so carrying them costs no extra request. They are here because
    /// none of those figures says what a rule DOES, and a list of GLPI's own
    /// names for them is the nearest readable thing to it.
    rules: Vec<RuleSummary>,
}

/// One rule, reduced to what can be read off it without interpreting it.
///
/// Deliberately not the criteria and actions themselves. Those are a tree and
/// a list that PHP evaluates, and rendering them as rows would suggest this
/// app knows what they will do - the same line the rest of this page draws
/// between GLPI's data and GLPI's program. The counts say how much there is
/// to interpret; the name is GLPI's own, and is the part a reader can act on.
#[derive(serde::Serialize)]
struct RuleSummary {
    name: String,
    /// GLPI's rank. The import collection is a ranked list where the first
    /// match ends the matter, so the order is the meaning, not a sort key.
    ranking: i64,
    is_active: bool,
    /// This rule sets `_ignore_import` - it can refuse a submission outright.
    denies: bool,
    /// This rule sets `_stop_rules_processing` - every rule ranked below it is
    /// skipped for whatever it matched.
    stops: bool,
    criteria: usize,
    actions: usize,
}

/// Every plugin the server has, grouped by type.
///
/// Separate from /api/mariadb for the same reason as the InnoDB settings:
/// 71 rows against that payload's ten, wanted by one sub-tab only.
#[derive(serde::Serialize)]
struct PluginsReport {
    /// False when GLPI_DB_URL is unset. An empty list and an unconfigured
    /// backend are opposite answers and the page renders them differently.
    configured: bool,
    types: Vec<crate::glpi::refused::PluginType>,
}

/// One GLPI table's attributes, and the properties a client shows for each.
///
/// The two travel together because the page reads them against each other:
/// five attributes and fourteen properties is the whole answer, and sending
/// them from two endpoints would let a page render half of it.
///
/// `properties` does not depend on `configured`. It is this app's list rather
/// than the server's, so it is answerable with no database at all - and a
/// page that hid it when the database was unreachable would withhold the half
/// that was never in doubt.
#[derive(serde::Serialize)]
struct AttributesReport {
    /// False when GLPI_DB_URL is unset. Applies to `attributes` only.
    configured: bool,
    /// The table these attributes belong to, echoed so the page names it
    /// rather than implying five is a fact about GLPI's schema at large.
    table: &'static str,
    /// How many tables the schema holds, so the page can say what the one
    /// below is one of. None when the database cannot be read.
    tables_total: Option<usize>,
    attributes: Vec<crate::glpi::refused::Attribute>,
    /// Heading, information_schema source, meaning. Source is empty where the
    /// server publishes nothing for that property.
    properties: &'static [(&'static str, &'static str, &'static str); 14],
}

#[derive(serde::Serialize)]
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

/// What the MariaDB instance behind GLPI actually is.
///
/// No entity scoping here, unlike `refused`: nothing in this answer is a
/// row of GLPI's data. It is the server's own description of itself, which
/// has no owner and no entity.
async fn mariadb(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(MariadbReport {
            configured: false,
            facts: None,
        }));
    };
    Ok(Json(MariadbReport {
        configured: true,
        facts: Some(db.server_facts().await?),
    }))
}

/// What the server has loaded, by kind.
///
/// Read live rather than listed here: the types are whatever this build
/// compiled in, and a list written into the app would answer for the server
/// it was written against and no other.
async fn mariadb_plugins(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(PluginsReport {
            configured: false,
            types: Vec::new(),
        }));
    };
    Ok(Json(PluginsReport {
        configured: true,
        types: db.plugins().await?,
    }))
}

/// What GLPI does on its own: to a ticket, change or problem, and to an
/// inventory submission arriving from an agent.
///
/// Through the API, not the database: `RuleController` models these, so the
/// counts arrive scoped to the caller's profile and entities the way every
/// other page's figures do. A COUNT over glpi_rules would be one query
/// instead of four requests and would quietly report rules the caller cannot
/// see.
async fn rules(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    // By the identifier the path takes, not by the collection's name: those
    // names are translated, so a test for "Business rules" answers correctly
    // on an English install and silently finds nothing on any other.
    const WANTED: [&str; 6] = [
        "Ticket",
        "Change",
        "Problem",
        "Right",
        "Asset",
        "ImportAsset",
    ];

    // The collections GLPI evaluates in FULL - every active rule tested, no
    // stopping at the first match - which is the set whose size is a linear
    // cost. Read off the collection classes rather than guessed: RuleCollection
    // leaves stop_on_first_match false and only RuleAsset and RuleRight join
    // the three ITIL ones in not overriding it. Everything else (import,
    // entity, itemtype, location, mail collector, software category, the
    // dictionaries) sets it true.
    //
    // Every one of them is in WANTED too, and that is the fix for a bug worth
    // recording: Asset was once listed here and deliberately left out of
    // WANTED, on the grounds that the API refused it for this account. When
    // the right was granted the collection appeared in GLPI's list, the walk
    // skipped it because WANTED did not name it, and the probe below found
    // nothing to refuse - so it was neither counted nor flagged, which is the
    // one outcome worse than either. A collection is walked if it is wanted
    // and reported if it is refused; nothing decides membership by what the
    // install happens to permit today.
    const FULL: [&str; 5] = ["Ticket", "Change", "Problem", "Right", "Asset"];

    let Ok(raw) = state.glpi.get("Rule/Collection").await else {
        return Ok(Json(RulesReport {
            available: false,
            collections: Vec::new(),
            refused: Vec::new(),
        }));
    };

    let mut collections = Vec::new();
    for item in collection(&raw) {
        let Some(rule_type) = item.get("rule_type").and_then(|v| v.as_str()) else {
            continue;
        };
        if !WANTED.contains(&rule_type) {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(rule_type)
            .to_string();

        // One request per collection, which is the cost of asking GLPI
        // rather than the database. Three of them, fixed - this does not
        // grow with the fleet.
        let rules = state
            .glpi
            .get(&format!("Rule/Collection/{rule_type}/Rule"))
            .await
            .unwrap_or_else(|_| serde_json::Value::Array(Vec::new()));
        let rules = collection(&rules);

        // Reduced once, here, and the three figures below are counted off the
        // reduction rather than off the JSON a second time. The previous shape
        // walked the raw items once per figure, which meant "a rule that
        // denies" was defined in one place and "the denies column" in another.
        let mut summaries: Vec<RuleSummary> = rules
            .iter()
            .map(|r| {
                let count = |key: &str| {
                    r.get(key)
                        .and_then(|v| v.as_array())
                        .map_or(0, |items| items.len())
                };
                RuleSummary {
                    name: r
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(unnamed)")
                        .to_string(),
                    ranking: r.get("ranking").and_then(|v| v.as_i64()).unwrap_or(0),
                    is_active: r.get("is_active").and_then(|v| v.as_bool()) == Some(true),
                    // A rule can carry several actions, so this is a rule that
                    // denies rather than a count of deny-actions - one rule
                    // setting it twice is still one rule that refuses.
                    denies: r
                        .get("actions")
                        .and_then(|a| a.as_array())
                        .is_some_and(|actions| {
                            actions.iter().any(|a| {
                                a.get("field").and_then(|f| f.as_str()) == Some("_ignore_import")
                            })
                        }),
                    // Counted the same way and for the same reason as
                    // `denies`. A rule that carries this ends the chain for
                    // the ticket it matched, so it is the one action whose
                    // effect is on the OTHER rules rather than on the ticket.
                    stops: r
                        .get("actions")
                        .and_then(|a| a.as_array())
                        .is_some_and(|actions| {
                            actions.iter().any(|a| {
                                a.get("field").and_then(|f| f.as_str())
                                    == Some("_stop_rules_processing")
                            })
                        }),
                    criteria: count("criteria"),
                    actions: count("actions"),
                }
            })
            .collect();
        // By rank, because for the import collection the order IS the rule:
        // the first match ends the matter. GLPI serves them in rank order
        // already; sorting here means the page does not depend on that.
        summaries.sort_by_key(|r| r.ranking);

        let active = summaries.iter().filter(|r| r.is_active).count();
        let denies = summaries.iter().filter(|r| r.denies).count();
        let stops = summaries.iter().filter(|r| r.stops).count();

        collections.push(RuleCollection {
            rule_type: rule_type.to_string(),
            name,
            total: summaries.len(),
            active,
            denies,
            stops,
            rules: summaries,
        });
    }

    // Anything evaluated in full that the collection list did not mention.
    // GLPI filters that list by right, so an absence here is a permission and
    // worth naming - asking costs one row and the answer is a finding.
    let mut refused = Vec::new();
    for rule_type in FULL {
        if collections.iter().any(|c| c.rule_type == rule_type) {
            continue;
        }
        if state
            .glpi
            .permits(&format!("Rule/Collection/{rule_type}/Rule"))
            .await
            .is_ok_and(|ok| !ok)
        {
            refused.push(rule_type.to_string());
        }
    }

    Ok(Json(RulesReport {
        available: true,
        collections,
        refused,
    }))
}

/// The table HeidiSQL's designer was open on, and the one the attributes link
/// reads.
///
/// One table rather than a picker over 447: the question this answers is what
/// an attribute is as against what a property is, and that is the same answer
/// whichever table is in front of you. `table_attributes` takes the name, so
/// a picker is a UI away rather than a rewrite.
const HEIDI_TABLE: &str = "glpi_alerts";

/// One nginx setting as the page needs it: where it goes, what it is now,
/// and whether it can be changed at all.
#[derive(Serialize)]
struct NginxTunable {
    name: &'static str,
    /// "append", "option" or "generated" - which of the three groups this is
    /// in, and the only thing deciding whether the page draws a field.
    group: &'static str,
    /// For append and option, the NixOS attribute it sets. For generated,
    /// the `recommended*` flag that already emits it.
    via: &'static str,
    /// What the tuning file currently asks for, if anything.
    wanted: Option<String>,
    /// What a value has to look like, for the field's own help.
    accepts: &'static str,
}

#[derive(Serialize)]
struct NginxTuningReport {
    /// False when GLPI_INV_NGINX_TUNING names no file - the one-time root
    /// step has not been done, so there is nowhere to write.
    configured: bool,
    /// False when the backend may read the file but not write it.
    writable: bool,
    path: Option<String>,
    settings: Vec<NginxTunable>,
}

fn describe(wanted: &[(String, String)]) -> Vec<NginxTunable> {
    use crate::glpi::nginx_tuning::{Kind, Route, TUNABLES};
    TUNABLES
        .iter()
        .map(|t| {
            let (group, via) = match t.route {
                Route::Append(o) => ("append", o),
                Route::Option_(o) => ("option", o),
                Route::Generated(f) => ("generated", f),
            };
            NginxTunable {
                name: t.name,
                group,
                via,
                wanted: wanted
                    .iter()
                    .find(|(n, _)| n == t.name)
                    .map(|(_, v)| v.clone()),
                accepts: match t.kind {
                    Kind::Count => "a whole number, or auto",
                    Kind::Size => "a number with an optional k, m or g",
                    Kind::Toggle => "on or off",
                    Kind::Duration => "a number with an optional s, m or h",
                    Kind::FileCache => "off, or max=N with an optional inactive=<time>",
                    Kind::TlsProtocols => "TLSv1.2 and TLSv1.3, space separated",
                },
            }
        })
        .collect()
}

async fn nginx_tuning(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let wanted = state
        .nginx_tuning
        .as_ref()
        .map(|f| f.current())
        .unwrap_or_default();

    Json(NginxTuningReport {
        configured: state.nginx_tuning.is_some(),
        writable: state.nginx_tuning.is_some() && state.nginx_writes,
        path: state
            .nginx_tuning
            .as_ref()
            .map(|f| f.path().display().to_string()),
        settings: describe(&wanted),
    })
}

/// Writes one setting into the tuning file. Applies nothing.
///
/// A rebuild is what makes this real, and the page says so - there is no
/// endpoint here that runs one. That is not caution about the command; it is
/// that `nixos-rebuild switch` needs root, rebuilds the whole system closure,
/// and restarts whatever changed, none of which belongs behind a dashboard
/// field.
async fn set_nginx_tuning(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Option<Json<VariableWrite>>,
) -> axum::response::Response {
    let Some(file) = state.nginx_tuning.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No tuning file is configured, so there is nowhere to keep this. \
                          Set GLPI_INV_NGINX_TUNING, import the same path from the system \
                          config, and git add it - /etc/nixos is a git repo and a flake \
                          cannot see an untracked file.",
            })),
        )
            .into_response();
    };

    if !state.nginx_writes {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "nginx settings are read-only in this backend. Start it with \
                          GLPI_INV_NGINX_WRITES=1 to allow changes.",
            })),
        )
            .into_response();
    }

    // No body clears the setting, exactly as the variables endpoint does.
    let value = body.map(|Json(b)| b.value);
    match file.set(&name, value.as_deref()) {
        Ok(set) => {
            tracing::info!("nginx tuning: {name} -> {value:?} ({} set)", set.len());
            Json(serde_json::json!({
                "name": name,
                "wanted": value,
                "note": "Written. A nixos-rebuild switch is what applies it.",
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GLPI's own response times over a window, for the PHP history page.
///
/// Answers in `StatusHistory`, the same shape as the MariaDB history, on
/// purpose: one set of chart components draws both, so the two pages cannot
/// drift apart in how they render a series.
///
/// `sampling` is false when the table is absent or the sampler never ran,
/// which is a different answer from a window with no samples in it. Unlike
/// the MariaDB history, that case is reachable by configuration:
/// GLPI_INV_PHP_SAMPLE_SECONDS=0 turns this off deliberately.
async fn php_history(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (window, hours) = history_window(q.window.as_deref());

    let Some(db) = state.db.as_ref() else {
        return Ok(Json(StatusHistoryReport {
            configured: false,
            sampling: false,
            window: window.to_string(),
            kept_days: crate::glpi::sampler::KEEP_DAYS,
            history: None,
        }));
    };

    let history = match db.php_history(hours, HISTORY_BUCKETS).await {
        Ok(h) => h,
        Err(_) => {
            return Ok(Json(StatusHistoryReport {
                configured: true,
                sampling: false,
                window: window.to_string(),
                kept_days: crate::glpi::sampler::KEEP_DAYS,
                history: None,
            }))
        }
    };

    Ok(Json(StatusHistoryReport {
        configured: true,
        // The table can exist with nothing in it - sampling switched off
        // after it had run once. An empty series list is that case, and
        // saying "sampling" there would point the reader at the data
        // instead of at the setting.
        sampling: !history.series.is_empty(),
        window: window.to_string(),
        kept_days: crate::glpi::sampler::KEEP_DAYS,
        history: Some(history),
    }))
}

/// Times one request and files it under the route that matched.
///
/// The clock starts after routing and stops when the handler's response is
/// ready, so what it measures is this process's work plus whatever the
/// handler waited on - GLPI, MariaDB - and not time on the wire.
async fn record_timing(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> axum::response::Response {
    // Cloned before the request is consumed. A route with no MatchedPath
    // is not recorded rather than recorded under a guess: an unkeyable
    // request is rare and a wrong key is worse than a missing one.
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());

    let started = std::time::Instant::now();
    let response = next.run(req).await;

    if let Some(route) = route {
        state.timings.record(&route, started.elapsed());
    }
    response
}

#[derive(Serialize)]
struct SelfTimingReport {
    /// Seconds this process has been up, so a count can be read as a rate
    /// and a route with one request is not mistaken for a quiet one.
    uptime_seconds: u64,
    routes: Vec<timing::RouteTiming>,
}

/// What this backend costs to answer, per route.
///
/// Measures the instrument rather than GLPI, which the rest of the perf page
/// deliberately does not - see `api::timing` for why this one is worth the
/// exception and what it does NOT justify.
async fn self_timing(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(SelfTimingReport {
        uptime_seconds: state.started.elapsed().as_secs(),
        routes: state.timings.report(),
    })
}

/// How far back the history panel looks, and how many points it draws.
///
/// 24 hours because the question the panel answers is "and what was it
/// overnight". 60 points because the panels are small multiples a couple of
/// hundred pixels wide - more points than pixels is detail nobody can see,
/// paid for in payload.
const HISTORY_HOURS: u32 = 24;
const HISTORY_BUCKETS: usize = 60;

/// The windows the history page offers, by the name its links use.
///
/// Named rather than taking an hour count from the query string: a window is
/// a reading decision - how much to average into one point - and four of them
/// cover the questions anyone asks of a counter. An hours parameter would also
/// let a caller ask for a window the sampler cannot answer without the page
/// being able to say so.
///
/// Note what the longer two are against the sampler's retention: samples are
/// pruned to `sampler::KEEP_DAYS`, so month and year show everything kept
/// rather than everything named. That is why the report carries `kept_days` -
/// the page states the gap instead of drawing empty time.
const HISTORY_WINDOWS: [(&str, u32); 4] = [
    ("day", HISTORY_HOURS),
    ("week", 24 * 7),
    ("month", 24 * 30),
    ("year", 24 * 365),
];

/// What the caller asked for, before it is matched against what exists.
#[derive(serde::Deserialize)]
struct HistoryQuery {
    window: Option<String>,
}

/// The window's name and hours, falling back to the day rather than failing.
///
/// An unknown name is a link this app does not have - a typed URL, or an old
/// bookmark - and answering it with the default is better than a 400 the page
/// would render as a broken history.
fn history_window(asked: Option<&str>) -> (&'static str, u32) {
    let asked = asked.unwrap_or("").trim().to_ascii_lowercase();
    HISTORY_WINDOWS
        .iter()
        .find(|(name, _)| *name == asked)
        .copied()
        .unwrap_or(HISTORY_WINDOWS[0])
}

#[derive(Serialize)]
struct StatusHistoryReport {
    /// False when GLPI_DB_URL is unset.
    configured: bool,
    /// The window actually served, by name. Echoed because the request may
    /// have asked for one this build does not have, and the page draws the
    /// link set from this rather than from what it sent.
    window: String,
    /// How long the sampler keeps a sample. The page needs it to say why a
    /// year-long window holds two weeks of data, which is a fact about the
    /// sampler rather than about the server being quiet.
    kept_days: u32,
    /// False when the sampler has never run - no schema, no event, or the
    /// event scheduler off. Distinct from a window with no samples in it,
    /// which is what a server that has been down looks like.
    sampling: bool,
    history: Option<crate::glpi::refused::StatusHistory>,
}

/// The sampled counters, as something the perf page can draw.
///
/// Reads `glpi_inv.status_samples`, which is this app's own schema and NOT
/// GLPI's - the rule that this app does not write behind GLPI is intact, and
/// a history table inside `glpi` would have broken it for a dashboard's
/// convenience.
async fn status_history(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (window, hours) = history_window(q.window.as_deref());
    let kept_days = crate::glpi::sampler::KEEP_DAYS;

    let Some(db) = state.db.as_ref() else {
        return Ok(Json(StatusHistoryReport {
            configured: false,
            window: window.to_string(),
            kept_days,
            sampling: false,
            history: None,
        }));
    };

    let history = match db.status_history(hours, HISTORY_BUCKETS).await {
        Ok(h) => h,
        // The table not existing is the un-set-up case, not a failure: the
        // page then says how to start sampling instead of showing an error
        // that reads as the database being broken.
        Err(_) => {
            return Ok(Json(StatusHistoryReport {
                configured: true,
                window: window.to_string(),
                kept_days,
                sampling: false,
                history: None,
            }))
        }
    };

    Ok(Json(StatusHistoryReport {
        configured: true,
        window: window.to_string(),
        kept_days,
        sampling: true,
        history: Some(history),
    }))
}

/// The web server in front of GLPI.
///
/// Configuration and two timings rather than counters, because nginx has no
/// counters unless somebody added a stub_status location - see the module for
/// the reasoning. Takes a moment: the two probes are five requests each, and
/// one of them is a full GLPI bootstrap.
async fn nginx(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(crate::glpi::nginx::report(&state.glpi_url).await))
}

/// Whether the slow query log is on, for the perf page's tile.
///
/// Cheap by construction: four variables and no table scan, so the tile can
/// be part of the page load. What the log CONTAINS is the next handler, and
/// it is a button rather than a fetch for exactly that reason.
#[derive(Serialize)]
struct SlowLogReport {
    /// False when GLPI_DB_URL is unset. "Cannot look" and "logging nothing"
    /// are opposite answers.
    configured: bool,
    settings: Option<crate::glpi::refused::SlowLogSettings>,
}

async fn slow_log(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(SlowLogReport {
            configured: false,
            settings: None,
        }));
    };
    Ok(Json(SlowLogReport {
        configured: true,
        settings: Some(db.slow_log_settings().await?),
    }))
}

/// How many of the slowest queries the button asks for.
///
/// Twenty because the tile is a starting point, not a report: past the first
/// handful the answer is a digest tool, and the tile says so rather than
/// paging a CSV table that cannot be indexed.
const SLOW_LOG_LIMIT: u32 = 20;

#[derive(Serialize)]
struct SlowLogEntriesReport {
    configured: bool,
    /// False when log_output does not include TABLE - there is then nothing
    /// in mysql.slow_log to read however much the server is logging, and an
    /// empty list would say the opposite.
    to_table: bool,
    limit: u32,
    entries: Option<crate::glpi::refused::SlowLogEntries>,
}

/// The query the perf tile's button runs.
///
/// A GET, because it reads and nothing else: the button is a control in the
/// page's terms, not a write in HTTP's.
async fn slow_log_entries(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(SlowLogEntriesReport {
            configured: false,
            to_table: false,
            limit: SLOW_LOG_LIMIT,
            entries: None,
        }));
    };

    // Checked before the scan rather than after: with log_output at FILE the
    // table is empty by design, and reporting that as "no slow queries" would
    // be the one wrong answer this endpoint can give.
    let settings = db.slow_log_settings().await?;
    if !settings.to_table {
        return Ok(Json(SlowLogEntriesReport {
            configured: true,
            to_table: false,
            limit: SLOW_LOG_LIMIT,
            entries: None,
        }));
    }

    Ok(Json(SlowLogEntriesReport {
        configured: true,
        to_table: true,
        limit: SLOW_LOG_LIMIT,
        entries: Some(db.slow_log_entries(SLOW_LOG_LIMIT).await?),
    }))
}

/// One table's attributes, with the properties a client shows for each.
///
/// Separate from /api/mariadb/plugins because it answers a different question
/// about the same client, and because the properties half is answerable with
/// no database at all.
async fn mariadb_attributes(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let properties = &refused::ATTRIBUTE_PROPERTIES;

    let Some(db) = state.db.as_ref() else {
        return Ok(Json(AttributesReport {
            configured: false,
            table: HEIDI_TABLE,
            tables_total: None,
            attributes: Vec::new(),
            properties,
        }));
    };
    Ok(Json(AttributesReport {
        configured: true,
        table: HEIDI_TABLE,
        tables_total: Some(db.glpi_table_count().await? as usize),
        attributes: db.table_attributes(HEIDI_TABLE).await?,
        properties,
    }))
}

/// Every InnoDB setting, grouped.
///
/// Separate from /api/mariadb because of its size: 137 rows on this server
/// against the facts payload's ten, and only one panel wants them. The
/// grouping is this app's - MariaDB publishes none - and lives beside the
/// rules that decide it, in glpi::refused.
#[derive(Serialize)]
struct InnodbReport {
    configured: bool,
    /// Whether this backend will accept a change. The panel renders its
    /// controls either way and says why they are disabled - a page that
    /// simply shows no buttons leaves the reader to guess whether the
    /// settings are read-only or the app is.
    writes_enabled: bool,
    /// Where persisted settings are written, if anywhere. Shown rather than
    /// hidden: a reader who wonders what "kept at startup" means should be
    /// able to go and read the file.
    persist_path: Option<String>,
    groups: Vec<refused::InnodbGroup>,
}

/// Every system variable, in four tiers.
#[derive(Serialize)]
struct VariablesReport {
    configured: bool,
    /// Whether this backend will accept a write at all. The page asks so it
    /// can say why a row has no control, rather than rendering one that is
    /// refused on use.
    writes_enabled: bool,
    /// Where a kept value is written, when anywhere. None means a change
    /// cannot outlive a restart, which the page says plainly rather than
    /// offering a Keep control that writes to nothing.
    persist_path: Option<String>,
    variables: Vec<refused::Variable>,
}

/// The whole variable space, each row tagged with what changing it costs and
/// with what the server will accept for it.
///
/// This used to say "no write path, deliberately", on the grounds that a SET
/// GLOBAL reachable for any variable the server has is a different decision
/// from the one made for InnoDB's 137. That was right, and the decision has
/// since been taken rather than drifted into: `set_variable` below is that
/// write, it has a flag of its own rather than inheriting the InnoDB one, and
/// the nine names that can take the server or GLPI down are set only on a
/// second, informed request. The old reasoning is kept here because it is
/// still the argument for every one of those guards.
async fn variables(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let persist_path = state.init.as_ref().map(|f| f.path().display().to_string());

    let Some(db) = state.db.as_ref() else {
        return Ok(Json(VariablesReport {
            configured: false,
            writes_enabled: state.variable_writes,
            persist_path,
            variables: Vec::new(),
        }));
    };

    // What the startup file asks for, folded into the rows it is about, so a
    // row can show "this is what a restart will give you" beside what the
    // server has now. An unreadable file reads as nothing persisted rather
    // than failing the page: the variables are still worth seeing, and the
    // disagreement is what the column is for.
    let persisted = state
        .init
        .as_ref()
        .map(|f| f.read().unwrap_or_default())
        .unwrap_or_default();

    let mut variables = db.variables().await?;
    for v in &mut variables {
        v.persisted = persisted.get(&v.name).cloned();
    }

    Ok(Json(VariablesReport {
        configured: true,
        writes_enabled: state.variable_writes,
        persist_path,
        variables,
    }))
}

/// Set one system variable on the running server.
///
/// The general case of `set_innodb`, behind its own flag, and with one step
/// that write does not have: a variable named in `glpi::hazard` is answered
/// 409 with the reason the first time and only set when the caller comes back
/// with `confirm`. The two-visit shape is the restart links' - preview, then
/// commit - because the cost is invisible in the control that performs it.
///
/// 409 rather than 400 so the page can tell "come back and mean it" from "the
/// server will not take that value", which look identical at 400 and need
/// opposite things from the reader.
async fn set_variable(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<VariableWrite>,
) -> axum::response::Response {
    if !state.variable_writes {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "System variables are read-only in this backend. Start it with \
                          GLPI_INV_VARIABLE_WRITES=1 to allow changes.",
            })),
        )
            .into_response();
    }
    let Some(db) = state.db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No database connection is configured, so there is no server to set."
            })),
        )
            .into_response();
    };

    // Before the write and before any validation: what this asks the reader
    // is whether they want the consequence, which does not depend on whether
    // the value parses.
    if let Some(why) = crate::glpi::hazard::hazard(&name) {
        if !body.confirm {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "confirm_required": true,
                    "name": name,
                    "warning": why,
                })),
            )
                .into_response();
        }
        tracing::warn!("confirmed hazard: SET GLOBAL {name} = {:?}", body.value);
    }

    match db.set_variable(&name, &body.value).await {
        Ok(done) => {
            tracing::info!(
                "SET GLOBAL {} = {:?}, now {:?}",
                done.name,
                body.value,
                done.value
            );
            // Setting keeps, unless there is nowhere to keep it.
            //
            // The two were separate endpoints at first and the split was
            // wrong in practice: a value set and not kept reverts at the next
            // restart with nothing on the row saying it will, so the control
            // that appeared to work was the one that silently did not last.
            // Making the common case the default leaves un-keeping as the
            // deliberate act, which is the right way round - it is the
            // reversible one.
            //
            // A failure to keep does NOT fail the request. The server has
            // already taken the value; reporting an error now would describe
            // a write that happened as one that did not. It comes back as
            // `kept`, and the page says which.
            let kept = match state.init.as_ref() {
                Some(init) => match init.upsert(&done.name, &done.literal) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::warn!("set {} but could not keep it: {e:#}", done.name);
                        false
                    }
                },
                None => false,
            };

            Json(VariableChange {
                name: done.name,
                requested: body.value,
                value: done.value,
                kept,
            })
            .into_response()
        }
        Err(e) => bad_request(e),
    }
}

/// What a write did, and whether it will still be true after a restart.
#[derive(Serialize)]
struct VariableChange {
    name: String,
    /// What was asked for.
    requested: String,
    /// What the server reports now. Not the same thing: a numeric value is
    /// rounded to the setting's block size, and a buffer pool resize is
    /// asynchronous, so this can lag or differ.
    value: String,
    /// Whether the startup file now carries it. False means the change is
    /// real but lasts only until the next restart - either because no
    /// startup file is configured, or because writing it failed.
    kept: bool,
}

/// Keep a variable across restarts, or stop keeping it.
///
/// The general case of `persist_innodb`, writing to the same file and under
/// the same flag as the general write. `Some(body)` adds or replaces the
/// line; no body drops it, which changes the next startup and deliberately
/// leaves the running server exactly as it is - silently reverting a live
/// value would be a second change nobody asked for.
///
/// Hazards are NOT re-confirmed here, and that is deliberate: a line in this
/// file does nothing until the next restart, and it can be removed again
/// before then. What needs the second look is `set_variable`, which takes
/// effect at once. The one thing that cannot be allowed is persisting a value
/// the running server would refuse - that line would fail at startup and, per
/// `glpi::init_file`, silently drop every line below it - so it goes
/// through the same `variable_literal` gate the write does.
async fn persist_variable(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Option<Json<VariableWrite>>,
) -> axum::response::Response {
    if !state.variable_writes {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "System variables are read-only in this backend. Start it with \
                          GLPI_INV_VARIABLE_WRITES=1 to allow changes.",
            })),
        )
            .into_response();
    }
    let Some(init) = state.init.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No startup file is configured, so there is nowhere to keep this. \
                          Set GLPI_INV_INIT_FILE, and point MariaDB's init-file at the same \
                          path - it cannot be under /home, which the mysql unit cannot see.",
            })),
        )
            .into_response();
    };

    let Some(Json(body)) = body else {
        return match init.remove(&name) {
            Ok(()) => {
                tracing::info!("no longer persisting {name}");
                Json(serde_json::json!({ "name": name, "persisted": null })).into_response()
            }
            Err(e) => bad_request(e),
        };
    };

    let Some(db) = state.db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No database connection is configured, so nothing can be checked \
                          against the server.",
            })),
        )
            .into_response();
    };

    match db.variable_literal(&name, &body.value).await {
        Ok((canonical, literal)) => match init.upsert(&canonical, &literal) {
            Ok(()) => {
                tracing::info!("persisting {canonical} = {:?}", body.value);
                Json(serde_json::json!({
                    "name": canonical,
                    "persisted": body.value,
                }))
                .into_response()
            }
            Err(e) => bad_request(e),
        },
        Err(e) => bad_request(e),
    }
}

#[derive(serde::Deserialize)]
struct VariableWrite {
    value: String,
    /// Absent means no. Only consulted for a name `glpi::hazard` knows; for
    /// every other variable the first request is the only one.
    #[serde(default)]
    confirm: bool,
}

/// What a write did, in the server's terms rather than the page's.
#[derive(Serialize)]
struct InnodbChange {
    name: String,
    /// What was asked for.
    requested: String,
    /// What the server reports now. Not the same thing: a numeric value is
    /// rounded to the setting's block size, and a buffer pool resize is
    /// asynchronous, so this can lag or differ.
    value: String,
}

/// How long GLPI takes to answer, measured now.
///
/// Measured per request rather than cached: the page exists to be reloaded
/// after a change, and a cached figure would answer the question the reader
/// asked five minutes ago. It costs ten small HTTP requests to GLPI, which
/// is why nothing else on this dashboard calls it.
async fn perf(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        crate::glpi::perf::measure(&state.glpi_url, state.db.as_ref()).await?,
    ))
}

async fn innodb(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(InnodbReport {
            configured: false,
            writes_enabled: false,
            persist_path: None,
            groups: Vec::new(),
        }));
    };

    // What the startup file asks for, folded into the rows it is about, so
    // the panel can put "this is what a restart will give you" beside "this
    // is what the server has". An unreadable file is reported as nothing
    // persisted rather than failing the whole panel: the settings are still
    // worth seeing, and the mismatch column is where the problem shows.
    let persisted = state
        .init
        .as_ref()
        .map(|f| f.read().unwrap_or_default())
        .unwrap_or_default();

    let mut groups = db.innodb_variables().await?;
    for group in &mut groups {
        for v in &mut group.variables {
            v.persisted = persisted.get(&v.name).cloned();
        }
    }

    Ok(Json(InnodbReport {
        configured: true,
        writes_enabled: state.innodb_writes_allowed(),
        persist_path: state.init.as_ref().map(|f| f.path().display().to_string()),
        groups,
    }))
}

/// Keep one InnoDB setting across restarts, or stop keeping it.
///
/// Writes the file MariaDB replays at startup. Gated exactly as the live
/// write is - this is the same change, made to apply again unattended - and
/// validated through the same path, because a value nobody is watching when
/// it runs is the last place to accept one the server might refuse.
async fn persist_innodb(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Option<Json<InnodbWrite>>,
) -> axum::response::Response {
    if !state.innodb_writes_allowed() {
        return refused_write();
    }
    let Some(init) = state.init.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No startup file is configured, so there is nowhere to keep this.                           Set GLPI_INV_INNODB_INIT_FILE, and point MariaDB's init-file at                           the same path."
            })),
        )
            .into_response();
    };

    // No body means stop persisting it. The setting keeps whatever it is
    // set to now - dropping the line is about the next restart, not this
    // one, and silently reverting a running server would be a second change
    // nobody asked for.
    let Some(Json(body)) = body else {
        return match init.remove(&name) {
            Ok(()) => {
                tracing::info!("no longer persisting {name}");
                Json(serde_json::json!({ "name": name, "persisted": null })).into_response()
            }
            Err(e) => bad_request(e),
        };
    };

    let Some(db) = state.db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No database connection is configured, so nothing can be checked                           against the server."
            })),
        )
            .into_response();
    };

    match db.innodb_literal(&name, &body.value).await {
        Ok((canonical, literal)) => match init.upsert(&canonical, &literal) {
            Ok(()) => {
                tracing::info!("persisting {canonical} = {:?}", body.value);
                Json(serde_json::json!({
                    "name": canonical,
                    "persisted": body.value,
                }))
                .into_response()
            }
            Err(e) => bad_request(e),
        },
        Err(e) => bad_request(e),
    }
}

/// Which restart, and whether to only show what would happen.
#[derive(serde::Deserialize)]
struct RestartRequest {
    /// "plain" or "settings". Anything else is the script's to reject - it
    /// prints its own usage and exits 2 - so this process does not keep a
    /// second list of valid modes that could disagree with the first.
    mode: String,
    #[serde(default)]
    settings: Vec<String>,
    /// True runs it with DRY_RUN=1: the script prints the commands and runs
    /// nothing. The links use this for the preview shown before confirming.
    #[serde(default)]
    preview: bool,
}

/// Restart GLPI's database, or restart it carrying a non-default InnoDB
/// setting.
///
/// THIS PROCESS DECIDES ALMOST NOTHING. It checks its flag, then runs
/// scripts/db-restart.sh and returns what the script said, verbatim and with
/// the script's own exit code. Everything that varies - the operating system,
/// the unit name, whether the config file is writable, which settings are
/// refused outright - is in the script, where it can be read and run by hand
/// by whoever is about to let it restart their database.
///
/// The escalation is the script's too, and it is polkit rather than a sudo
/// rule: a human consents to this specific restart, at the moment it happens.
/// The flag here is a separate question - whether an install offers the
/// control at all.
async fn db_restart(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RestartRequest>,
) -> axum::response::Response {
    // A preview runs nothing, so it is allowed to describe what the control
    // would do even where the control is off: a reader deserves to see why a
    // disabled link exists. The script's settings mode on NixOS is pure
    // instructions anyway, which is the case this matters for.
    if !state.db_restart && !req.preview {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Restarting the database is disabled in this backend. Start it \
                          with GLPI_INV_DB_RESTART=1 to allow it - and note that a restart \
                          takes every in-flight inventory import with it."
            })),
        )
            .into_response();
    }

    match crate::db_restart::run(&req.mode, &req.settings, req.preview).await {
        Ok(run) => {
            if !run.preview {
                tracing::warn!(
                    "db-restart.sh {} {:?} exited {}",
                    req.mode,
                    req.settings,
                    run.code
                );
            }
            Json(run).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

fn refused_write() -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "error": "Engine settings are read-only in this backend. Start it with \
                      GLPI_INV_INNODB_WRITES=1 for these settings alone, or \
                      GLPI_INV_VARIABLE_WRITES=1, which already covers them."
        })),
    )
        .into_response()
}

fn bad_request(e: anyhow::Error) -> axum::response::Response {
    let msg = format!("{e:#}");
    tracing::warn!("refused: {msg}");
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

/// Change one InnoDB setting on the running server.
///
/// The only request this backend makes that is not a read. Three refusals
/// before it reaches SQL, in this order: writes must be enabled, a database
/// must be configured, and the setting must be one the server reports as
/// writable - the last two are decided in glpi::refused, against the server,
/// not against anything the page sent.
///
/// A rejected value answers 400 with the reason in `error`, not 502: the
/// request was understood and refused, and the panel prints that sentence
/// beside the row. 502 stays what it has always been here - the upstream
/// could not be reached or failed.
async fn set_innodb(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<InnodbWrite>,
) -> axum::response::Response {
    if !state.innodb_writes_allowed() {
        return refused_write();
    }
    let Some(db) = state.db.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No database connection is configured, so there is no server to set."
            })),
        )
            .into_response();
    };

    match db.set_innodb_setting(&name, &body.value).await {
        Ok(value) => {
            tracing::info!("SET GLOBAL {name} = {:?}, now {value:?}", body.value);
            Json(InnodbChange {
                name,
                requested: body.value,
                value,
            })
            .into_response()
        }
        Err(e) => bad_request(e),
    }
}

#[derive(serde::Deserialize)]
struct InnodbWrite {
    value: String,
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
