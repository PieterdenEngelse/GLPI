//! The one page that answers the question: which agents have gone quiet?
use crate::api::{self, AgentStatus};
use crate::components::{InfoLink, InfoSection, Panel, RefreshControl, ScopeNote, StatRow};
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
pub fn Agents() -> Element {
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
                    title: "Registered agents".to_string(),
                    subtitle: "Most overdue first".to_string(),
                    info: "Every inventory agent registered in GLPI, with the one judgement \
                           this app adds: whether it is still reporting."
                        .to_string(),
                    info_sections: vec![
                        InfoSection::new(
                            "The columns",
                            "Status is the summary. Agent shows the name GLPI holds and, beneath \
                             it, the identifier the agent reports itself under - GLPI matches on \
                             that identifier between runs, not on the hostname - it is only shown when it \
                             differs from the name. Age is how long since \
                             the last check-in; hover it for the exact timestamp. Version is \
                             the agent's own version, useful for \
                             spotting a fleet drifting apart. From is the address the last \
                             submission arrived from.",
                        ),
                        InfoSection::new(
                            "The order",
                            "Most overdue first, always. The reason to open this page is to find \
                             what has stopped reporting, so the answer is on the first row rather \
                             than somewhere down an alphabetical list.",
                        ),
                        InfoSection::new(
                            "How often it updates",
                            "The table re-reads every five minutes, and Refresh now does it \
                             immediately. Five minutes rather than five seconds because the \
                             underlying agents report once a day - a faster poll could not show \
                             anything new and would only add load to GLPI.",
                        ),
                    ],
                    info_links: vec![InfoLink::new("Agents", "front/agent.php")],
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
    // GLPI sets tag only if services.glpi-agent.tag was configured, so on most
    // installs the column is empty for every row. Dropping it when nothing has
    // one keeps the table's width meaning something.
    let any_tag = agents
        .iter()
        .any(|a| a.tag.as_deref().is_some_and(|t| !t.is_empty()));

    rsx! {
        // The table scrolls inside its own box; the page never scrolls sideways.
        div { class: "overflow-x-auto",
            table { class: "table table-sm table-pin-rows",
                thead {
                    tr {
                        th { "Status" }
                        th { "Agent" }
                        th { "Age" }
                        th { "Version" }
                        if any_tag {
                            th { "Tag" }
                        }
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
                                // GLPI names an agent after its deviceid unless
                                // something else was set, so on most rows these
                                // two strings are identical. Printing it twice
                                // taught the reader nothing and cost a line.
                                if a.deviceid.is_some() && a.deviceid != a.name {
                                    div { class: "font-mono text-xs text-base-content/60",
                                        "{a.deviceid.clone().unwrap_or_default()}"
                                    }
                                }
                            }
                            // The exact timestamp moves to the hover title. Age
                            // is the question - "is this machine overdue" - and
                            // the raw value was the widest column on the page
                            // while answering it worst.
                            td {
                                class: if a.stale { "whitespace-nowrap font-semibold text-error" } else { "whitespace-nowrap" },
                                title: "Last contact: {a.last_contact.clone().unwrap_or_else(|| \"never\".into())}",
                                match a.hours_since_contact {
                                    Some(h) if h < 1 => "just now".to_string(),
                                    Some(h) if h < 48 => format!("{h}h ago"),
                                    Some(h) => format!("{}d ago", h / 24),
                                    None => "never".to_string(),
                                }
                            }
                            td { span { class: "badge badge-ghost badge-sm font-mono", "{a.version.clone().unwrap_or_default()}" } }
                            if any_tag {
                                td { class: "text-base-content/70", "{a.tag.clone().unwrap_or_default()}" }
                            }
                            td { class: "font-mono text-xs text-base-content/70", "{a.remote_address.clone().unwrap_or_default()}" }
                        }
                    }
                }
            }
        }
    }
}
