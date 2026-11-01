//! The one page that answers the question: which agents have gone quiet?
use crate::api::{self, AgentStatus};
use crate::components::{Panel, RefreshControl, ScopeNote, StatRow};
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
        match &*agents.read_unchecked() {
            // An unreachable backend must not look like an empty fleet. The
            // two mean opposite things and the page says which it is.
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not reach the backend." }
                        pre { class: "mt-1 text-xs whitespace-pre-wrap break-all", "{e}" }
                    }
                }
            },

            Some(Ok(list)) => rsx! {
                StatRow { agents: list.clone() }

                Panel {
                    title: "Agents".to_string(),
                    subtitle: "Most overdue first".to_string(),
                    actions: rsx! {
                        RefreshControl {
                            interval_label: REFRESH_LABEL.to_string(),
                            on_refresh: move |_| agents.restart(),
                        }
                    },

                    // Above the table: the scope qualifies every row in it, so
                    // reading it afterwards is too late.
                    ScopeNote {}

                    if list.is_empty() {
                        div { class: "text-base-content/60",
                            "No agents registered yet. GLPI has nothing to report, "
                            "which is different from an error."
                        }
                    } else {
                        AgentTable { agents: list.clone() }
                    }
                }
            },

            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Loading…"
                }
            },
        }
    }
}

#[component]
fn AgentTable(agents: Vec<AgentStatus>) -> Element {
    rsx! {
        // The table scrolls inside its own box; the page never scrolls sideways.
        div { class: "overflow-x-auto",
            table { class: "table table-sm table-pin-rows",
                thead {
                    tr {
                        th { "Status" }
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
                        tr { class: if a.stale { "bg-error/5" } else { "" },
                            td {
                                if a.stale {
                                    span { class: "badge badge-error badge-sm", "stale" }
                                } else {
                                    span { class: "badge badge-success badge-sm", "ok" }
                                }
                            }
                            td {
                                div { class: "font-medium", "{a.name.clone().unwrap_or_else(|| \"(unnamed)\".into())}" }
                                div { class: "font-mono text-xs text-base-content/60", "{a.deviceid.clone().unwrap_or_default()}" }
                            }
                            td { class: "whitespace-nowrap text-base-content/80", "{a.last_contact.clone().unwrap_or_else(|| \"never\".into())}" }
                            td { class: if a.stale { "whitespace-nowrap font-semibold text-error" } else { "whitespace-nowrap" },
                                match a.hours_since_contact {
                                    Some(h) if h < 1 => "just now".to_string(),
                                    Some(h) if h < 48 => format!("{h}h ago"),
                                    Some(h) => format!("{}d ago", h / 24),
                                    None => "never".to_string(),
                                }
                            }
                            td { span { class: "badge badge-ghost badge-sm font-mono", "{a.version.clone().unwrap_or_default()}" } }
                            td { class: "text-base-content/70", "{a.tag.clone().unwrap_or_default()}" }
                            td { class: "font-mono text-xs text-base-content/70", "{a.remote_address.clone().unwrap_or_default()}" }
                        }
                    }
                }
            }
        }
    }
}
