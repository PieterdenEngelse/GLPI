//! What this app is, as GLPI sees it.
//!
//! Written because this cost hours. A 403 from GLPI's HL API says only "You
//! don't have permission to perform this action" with a null detail - it
//! never names the missing right - and GLPI activates exactly ONE profile per
//! session, so an account holding several authorizations does not get their
//! union. None of that is visible from the dashboard it breaks.
use crate::api::{self, SessionInfo};
use crate::components::Panel;
use dioxus::prelude::*;

/// GLPI's rights are a bitmask; READ is bit 1.
const READ: i64 = 1;

#[component]
pub fn Status() -> Element {
    let session = use_resource(|| async move { api::fetch_session().await });

    rsx! {
        match &*session.read_unchecked() {
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not read the GLPI session." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },
            Some(Ok(s)) => rsx! { SessionPanel { session: s.clone() } },
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
fn SessionPanel(session: SessionInfo) -> Element {
    let agent_ok = session.right_agent.unwrap_or(0) & READ != 0;
    let entity_ok = session.right_entity.unwrap_or(0) & READ != 0;
    let recursive = session.entity_recursive.unwrap_or(false);

    rsx! {
        Panel {
            title: "GLPI session".to_string(),
            subtitle: "The identity and rights this dashboard reads with".to_string(),

            dl { class: "grid gap-x-6 gap-y-3 sm:grid-cols-2",
                Row { label: "Profile".to_string(),
                    value: session.profile_name.clone().unwrap_or_else(|| "unknown".into()) }
                Row { label: "Interface".to_string(),
                    value: session.profile_interface.clone().unwrap_or_else(|| "unknown".into()) }
                Row { label: "Entity".to_string(),
                    value: session.entity_name.clone().unwrap_or_else(|| "unknown".into()) }

                div {
                    dt { class: "text-xs text-base-content/60", "Includes child entities" }
                    dd { class: "mt-0.5",
                        if recursive {
                            span { class: "badge badge-success badge-sm", "recursive" }
                        } else {
                            span { class: "badge badge-warning badge-sm", "this entity only" }
                        }
                    }
                }

                div {
                    dt { class: "text-xs text-base-content/60", "Right: agent (read)" }
                    dd { class: "mt-0.5",
                        if agent_ok {
                            span { class: "badge badge-success badge-sm", "granted" }
                        } else {
                            span { class: "badge badge-error badge-sm", "missing" }
                        }
                    }
                }

                div {
                    dt { class: "text-xs text-base-content/60", "Right: entity (read)" }
                    dd { class: "mt-0.5",
                        if entity_ok {
                            span { class: "badge badge-success badge-sm", "granted" }
                        } else {
                            span { class: "badge badge-error badge-sm", "missing" }
                        }
                    }
                }
            }

            if !recursive {
                div { class: "alert alert-warning mt-5",
                    div {
                        div { class: "font-semibold", "Agents in child entities are invisible here." }
                        div { class: "text-sm",
                            "GLPI filters every query by entity and the excluded rows leave no "
                            "trace in the response - no count, no header. A machine outside this "
                            "scope is simply absent, which looks identical to one that does not "
                            "exist. Set the authorization recursive in GLPI to widen it."
                        }
                    }
                }
            }

            if !agent_ok || !entity_ok {
                div { class: "alert alert-error mt-3",
                    div {
                        div { class: "font-semibold", "A required right is missing." }
                        div { class: "text-sm",
                            "GLPI answers 403 without naming the right, so check both: "
                            "Administration → Profiles → this profile → Administration tab, "
                            "Read on Agent and on Entities."
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Row(label: String, value: String) -> Element {
    rsx! {
        div {
            dt { class: "text-xs text-base-content/60", "{label}" }
            dd { class: "mt-0.5 font-medium", "{value}" }
        }
    }
}
