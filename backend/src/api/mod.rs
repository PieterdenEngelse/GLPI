//! This backend's own HTTP surface, consumed by the Dioxus frontend.
//!
//! Loopback only by default. It holds GLPI OAuth credentials, so exposing it
//! to a network would hand anyone who can reach it the ability to read GLPI
//! through this process.
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;

use crate::glpi::{agents::Agent, Client};

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
        .with_state(state.clone());

    match &state.static_dir {
        // fallback_service, not nest("/"): the frontend is a single-page app,
        // so a path it owns client-side must still return index.html rather
        // than 404. ServeDir's own fallback does that; routing it as a nested
        // service would shadow /api instead.
        Some(dir) => api.fallback_service(
            tower_http::services::ServeDir::new(dir)
                .fallback(tower_http::services::ServeFile::new(dir.join("index.html"))),
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

    // GLPI returns a bare array for a collection. Tolerate an object with a
    // data/items key too, since the HL API's envelope has changed across its
    // own versions and pinning to one shape would break on an upgrade.
    let list = raw
        .as_array()
        .cloned()
        .or_else(|| raw.get("data").and_then(|d| d.as_array()).cloned())
        .or_else(|| raw.get("items").and_then(|d| d.as_array()).cloned())
        .unwrap_or_default();

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
