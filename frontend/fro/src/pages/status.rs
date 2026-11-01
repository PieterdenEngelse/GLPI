//! What this app is, as GLPI sees it.
//!
//! Written because this cost hours. A 403 from GLPI's HL API says only "You
//! don't have permission to perform this action" with a null detail - it
//! never names the missing right - and GLPI activates exactly ONE profile per
//! session, so an account holding several authorizations does not get their
//! union. None of that is visible from the dashboard it breaks.
use crate::api::{self, SessionInfo};
use crate::components::{InfoButton, Panel};
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
            info: "This app signs in to GLPI as an ordinary user and can only see what that \
                   user is allowed to see. When a page is empty or refuses to load, this is \
                   where to look first - GLPI's refusals do not say which permission was \
                   missing.".to_string(),

            dl { class: "grid gap-x-6 gap-y-3 sm:grid-cols-2",
                Row {
                    label: "Profile".to_string(),
                    value: session.profile_name.clone().unwrap_or_else(|| "unknown".into()),
                    info: "The set of permissions GLPI applied to this session. An account \
                           can be granted several profiles, but GLPI activates exactly one \
                           at a time - the account's default - so holding others does not \
                           add their permissions.".to_string(),
                }
                Row {
                    label: "Interface".to_string(),
                    value: session.profile_interface.clone().unwrap_or_else(|| "unknown".into()),
                    info: "GLPI profiles come in two kinds. Standard can hold the permissions \
                           this app needs; Simplified is the helpdesk view and cannot hold \
                           them at all - the settings simply do not appear on it.".to_string(),
                }
                Row {
                    label: "Entity".to_string(),
                    value: session.entity_name.clone().unwrap_or_else(|| "unknown".into()),
                    info: "GLPI can divide one installation into entities - separate sites, \
                           departments or customers - and every list is filtered to the ones \
                           this account may reach. This is the entity it is working in.".to_string(),
                }

                div {
                    dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                        span { "Includes child entities" }
                        InfoButton {
                            title: "Includes child entities".to_string(),
                            body: "Whether this app can also see entities nested below the one \
                                   above. When it cannot, machines in those entities are \
                                   missing from every page with nothing to indicate it - GLPI \
                                   removes them from the results and reports no count, so an \
                                   absent machine is indistinguishable from one that was never \
                                   there.".to_string(),
                        }
                    }
                    dd { class: "mt-0.5",
                        if recursive {
                            span { class: "badge badge-success badge-sm", "recursive" }
                        } else {
                            span { class: "badge badge-warning badge-sm", "this entity only" }
                        }
                    }
                }

                div {
                    dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                        span { "Right: agent (read)" }
                        InfoButton {
                            title: "Right: agent (read)".to_string(),
                            body: "Permission to list inventory agents. Without it the Agents \
                                   page cannot load at all, and GLPI's refusal does not say \
                                   which permission is missing. It is granted in GLPI under \
                                   Administration, Profiles, on the Administration tab.".to_string(),
                        }
                    }
                    dd { class: "mt-0.5",
                        if agent_ok {
                            span { class: "badge badge-success badge-sm", "granted" }
                        } else {
                            span { class: "badge badge-error badge-sm", "missing" }
                        }
                    }
                }

                div {
                    dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                        span { "Right: entity (read)" }
                        InfoButton {
                            title: "Right: entity (read)".to_string(),
                            body: "Permission to list entities. Without it this app cannot \
                                   state which part of GLPI it is reading, so it cannot tell \
                                   you whether a page is showing everything or only a \
                                   slice.".to_string(),
                        }
                    }
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
fn Row(label: String, value: String, info: String) -> Element {
    rsx! {
        div {
            dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                span { "{label}" }
                InfoButton { title: label.clone(), body: info }
            }
            dd { class: "mt-0.5 font-medium", "{value}" }
        }
    }
}
