//! The one page that answers the question: which agents have gone quiet?
use crate::api::{self, AgentStatus};
use dioxus::prelude::*;

#[component]
pub fn Fleet() -> Element {
    let agents = use_resource(|| async move { api::fetch_agents().await });

    rsx! {
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
