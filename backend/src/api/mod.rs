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
    agents::Agent, computers::Computer, entities::Entity, session::SessionInfo, Client,
};

pub struct AppState {
    pub glpi: Client,
    /// Where the dx-built web bundle lives, if it is being served.
    pub static_dir: Option<std::path::PathBuf>,
    /// Hours without contact before an agent is called stale. 26 rather than
    /// 24: the agent timer runs daily with a RandomizedDelaySec, so a strict
    /// 24 would flag healthy machines whose run drifted by minutes.
    pub stale_after_hours: i64,
}

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/agents", get(agents))
        .route("/api/entities", get(entities))
        .route("/api/computers", get(computers))
        .route("/api/session", get(session))
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
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
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

    let list = collection(&raw);

    let mut out = Vec::with_capacity(list.len());
    for item in list {
        // Skip rather than fail the whole request: one agent row GLPI
        // reports in an unexpected shape should not blank the dashboard.
        match serde_json::from_value::<Agent>(item.clone()) {
            Ok(a) => out.push(a.status(state.stale_after_hours)),
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

    let raw = state.glpi.get("Assets/Computer").await?;
    let mut out = Vec::new();
    for item in collection(&raw) {
        match serde_json::from_value::<Computer>(item.clone()) {
            Ok(c) => out.push(c.status(&covered)),
            Err(e) => tracing::warn!("skipping unparseable computer row: {e}; raw={item}"),
        }
    }

    // Orphans first, then oldest inventory: both are reasons to look, and an
    // orphan is the worse one.
    out.sort_by_key(|c| {
        (
            std::cmp::Reverse(c.orphaned),
            std::cmp::Reverse(c.hours_since_inventory.unwrap_or(i64::MAX)),
        )
    });
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
