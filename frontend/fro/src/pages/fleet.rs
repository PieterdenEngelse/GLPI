//! The one page that answers the question: which agents have gone quiet?
use crate::api::{self, AgentStatus};
use crate::components::RefreshControl;
use dioxus::prelude::*;

/// How often the table re-reads the backend.
///
/// Five minutes, not five seconds. The agent timer behind this data runs
/// once a day, and the number the page derives from it is in whole hours, so
/// a faster poll could not show anything new - it would only cost GLPI a
/// query plus the COUNT it runs for the total, every tick, forever. What the
/// interval has to beat is a dashboard left open overnight still showing
/// yesterday's hours, and minutes beat that comfortably.
const REFRESH_MS: u32 = 5 * 60 * 1000;
const REFRESH_LABEL: &str = "5m";

#[component]
pub fn Fleet() -> Element {
    let mut agents = use_resource(|| async move { api::fetch_agents().await });

    // use_future, not a spawn per render: this runs once for the life of the
    // component. restart() re-runs the resource in place, so the table keeps
    // showing the previous rows until the new ones land rather than blinking
    // back through the loading state every five minutes.
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(REFRESH_MS).await;
            agents.restart();
        }
    });

    rsx! {
        RefreshControl {
            interval_label: REFRESH_LABEL.to_string(),
            on_refresh: move |_| agents.restart(),
        }
        match &*agents.read_unchecked() {
            // An unreachable backend must not look like an empty fleet. The
            // two mean opposite things and the page says which it is.
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
