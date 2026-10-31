//! GLPI inventory telemetry - the view.
//!
//! One page, one question: which agents have stopped reporting?
use dioxus::prelude::*;
use serde::Deserialize;

/// Mirrors the backend's AgentStatus.
///
/// Deliberately duplicated rather than shared through a fourth crate: the
/// layout here follows RERAG, which has no shared crate. The cost is that
/// this struct and backend/src/glpi/agents.rs must be changed together -
/// every field is Option so a backend that adds or drops one degrades to a
/// blank cell instead of a failed parse.
// PartialEq is required, not cosmetic: Dioxus 0.7 memoizes components by
// comparing props, so a prop type without it fails to compile with
// "binary operation `==` cannot be applied" pointing at the component
// rather than at the struct.
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct AgentStatus {
    name: Option<String>,
    deviceid: Option<String>,
    last_contact: Option<String>,
    version: Option<String>,
    tag: Option<String>,
    remote_address: Option<String>,
    hours_since_contact: Option<i64>,
    stale: bool,
}

fn main() {
    dioxus::launch(App);
}

/// Absolute URL for a backend path, resolved against the page's own origin.
///
/// reqwest compiled to wasm32 goes through the browser's fetch, but it still
/// parses the URL with `url::Url` first, and that REJECTS a relative path -
/// there is no base. The failure surfaces as the single phrase "builder
/// error", with no mention of the URL, so it reads like a misconfigured
/// client rather than a malformed address.
///
/// The backend serves this bundle, so its origin is always the right base and
/// nothing needs configuring. Same origin also means no CORS.
fn api_url(path: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    format!("{origin}{path}")
}

#[component]
fn App() -> Element {
    let agents = use_resource(|| async move {
        reqwest::get(api_url("/api/agents"))
            .await
            .map_err(|e| e.to_string())?
            .json::<Vec<AgentStatus>>()
            .await
            .map_err(|e| e.to_string())
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/styling/main.css") }
        main {
            h1 { "GLPI inventory" }
            p { class: "sub", "Which agents have stopped reporting" }

            match &*agents.read_unchecked() {
                // An unreachable backend must not look like an empty fleet.
                Some(Err(e)) => rsx! {
                    div { class: "error",
                        strong { "Could not reach the backend." }
                        pre { "{e}" }
                    }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div { class: "empty",
                        "No agents registered yet. GLPI has nothing to report, "
                        "which is different from an error."
                    }
                },
                Some(Ok(list)) => rsx! { AgentTable { agents: list.clone() } },
                None => rsx! { div { class: "loading", "Loading…" } },
            }
        }
    }
}

#[component]
fn AgentTable(agents: Vec<AgentStatus>) -> Element {
    rsx! {
        table {
            thead {
                tr {
                    th { "Agent" }
                    th { "Last contact" }
                    th { "Age" }
                    th { "Version" }
                    th { "Tag" }
                    th { "From" }
                }
            }
            tbody {
                for a in agents.iter() {
                    tr { class: if a.stale { "stale" } else { "fresh" },
                        td {
                            div { class: "name", "{a.name.clone().unwrap_or_else(|| \"(unnamed)\".into())}" }
                            div { class: "deviceid", "{a.deviceid.clone().unwrap_or_default()}" }
                        }
                        td { "{a.last_contact.clone().unwrap_or_else(|| \"never\".into())}" }
                        td {
                            match a.hours_since_contact {
                                Some(h) if h < 1 => "just now".to_string(),
                                Some(h) if h < 48 => format!("{h}h ago"),
                                Some(h) => format!("{}d ago", h / 24),
                                None => "—".to_string(),
                            }
                        }
                        td { "{a.version.clone().unwrap_or_default()}" }
                        td { "{a.tag.clone().unwrap_or_default()}" }
                        td { "{a.remote_address.clone().unwrap_or_default()}" }
                    }
                }
            }
        }
    }
}
