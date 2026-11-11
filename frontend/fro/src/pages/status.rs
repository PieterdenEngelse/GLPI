//! What this app is, as GLPI sees it.
//!
//! Written because this cost hours. A 403 from GLPI's HL API says only "You
//! don't have permission to perform this action" with a null detail - it
//! never names the missing right - and GLPI activates exactly ONE profile per
//! session, so an account holding several authorizations does not get their
//! union. None of that is visible from the dashboard it breaks.
use crate::api::{self, Authorization, SessionInfo};
use crate::components::{InfoButton, InfoLink, InfoSection, Panel};
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
            Some(Ok(s)) => rsx! {
                SessionPanel { session: s.clone() }
                AuthorizationsPanel {}
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
fn SessionPanel(session: SessionInfo) -> Element {
    let agent_ok = session.right_agent.unwrap_or(0) & READ != 0;
    let entity_ok = session.right_entity.unwrap_or(0) & READ != 0;
    let recursive = session.entity_recursive.unwrap_or(false);

    rsx! {
        Panel {
            title: "GLPI session".to_string(),
            subtitle: "The identity and rights this dashboard reads with".to_string(),
            info: "This app signs in to GLPI as an ordinary user and can only see what \
                   that user is allowed to see."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this page exists",
                    "When GLPI refuses a request it answers with a single sentence - you do \
                     not have permission to perform this action - and does not say which \
                     permission was missing. There is no way to tell from the refusal alone \
                     whether the account cannot list agents, cannot list entities, or is \
                     simply pointed at the wrong part of GLPI. This page shows all of it at \
                     once.",
                ),
                InfoSection::new(
                    "If a page is empty",
                    "Check in this order: both rights below are granted; the interface is \
                     Standard; and child entities are included. An empty list with all four \
                     green means GLPI genuinely has nothing to show, which is a different \
                     and much better answer.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Users", "front/user.php"),
                InfoLink::new("Profiles", "front/profile.php"),
            ],

            dl { class: "grid gap-x-6 gap-y-3 sm:grid-cols-2",
                Row {
                    label: "Profile".to_string(),
                    value: session.profile_name.clone().unwrap_or_else(|| "unknown".into()),
                    info: "The set of permissions GLPI applied to this session. An account \
                           can be granted several profiles, but GLPI activates exactly one \
                           per session - the account's default - so holding others does not \
                           add their permissions; granting a second profile and expecting the \
                           union is a common and quiet mistake. The profile an account may \
                           use is granted under its Authorizations in GLPI, and which one is \
                           used by default is a field on the account itself. Changing the \
                           permissions inside a profile affects every account that uses it, \
                           so a dedicated profile is safer than editing a shared one."
                        .to_string(),
                    links: vec![
                        InfoLink::new("Users", "front/user.php"),
                        InfoLink::new("Profiles", "front/profile.php"),
                    ],
                }
                Row {
                    label: "Interface".to_string(),
                    value: session.profile_interface.clone().unwrap_or_else(|| "unknown".into()),
                    info: "GLPI profiles come in two kinds, and only one of them can hold the \
                           permissions this app needs. Standard is the full interface. \
                           Simplified is the helpdesk view, meant for people who only raise \
                           tickets, and the permissions this app needs do not exist on it - \
                           the settings are not merely unticked, the whole section is absent \
                           from the form. If this says Simplified, no amount of looking for \
                           the permissions will help: the profile's interface has to be \
                           changed to Standard first, after which the section appears and the \
                           two rights below can be granted."
                        .to_string(),
                    links: vec![InfoLink::new("Profiles", "front/profile.php")],
                }
                Row {
                    label: "Entity".to_string(),
                    value: session.entity_name.clone().unwrap_or_else(|| "unknown".into()),
                    info: "The part of GLPI this app is working in. GLPI can divide one \
                           installation into entities - separate sites, departments or \
                           customers. It affects everything: GLPI filters every list by \
                           entity before this app sees it, so the entity here decides which \
                           machines exist as far as any page is concerned. A small \
                           installation usually has only the root entity, in which case this \
                           never matters."
                        .to_string(),
                    links: vec![InfoLink::new("Entities", "front/entity.php")],
                }

                div {
                    dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                        span { "Includes child entities" }
                        InfoButton {
                            title: "Includes child entities".to_string(),
                            body: "Whether this app can also see entities nested below the \
                                   one above. When it is wrong it is invisible: GLPI removes \
                                   out-of-scope machines from the results and reports no \
                                   count of what it removed - no total, no marker, nothing - \
                                   so a machine you cannot see is indistinguishable from one \
                                   that was never there, and no amount of checking from \
                                   inside this app can tell the difference. It is a property \
                                   of the account's authorization, not of the profile: the \
                                   same profile can be granted recursively to one account and \
                                   not another. It is set when the authorization is added, \
                                   and GLPI marks a recursive one with an R in the list."
                                .to_string(),
                            links: vec![InfoLink::new("Users", "front/user.php")],
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
                                   page cannot load at all. The permission is called Agent, \
                                   and it is not the one called Inventory - granting \
                                   Inventory does nothing for this, and the two sit near each \
                                   other in the same list, which makes the wrong one easy to \
                                   tick. Grant it in GLPI under Administration, then \
                                   Profiles, on the Administration tab of the profile named \
                                   above; Read is enough, since this app never writes \
                                   anything."
                                .to_string(),
                            links: vec![InfoLink::new("Profiles", "front/profile.php")],
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
                                   state which part of GLPI it is reading: the line under \
                                   each table cannot say what the list was drawn from, so a \
                                   short list and a complete one look the same. On an \
                                   installation with only the root entity nothing is actually \
                                   hidden, but the app can no longer say so. Grant it in the \
                                   same place as the agent permission - Administration, \
                                   Profiles, Administration tab - where it is listed as \
                                   Entities, and Read is enough."
                                .to_string(),
                            links: vec![InfoLink::new("Profiles", "front/profile.php")],
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
fn Row(
    label: String,
    value: String,
    info: String,
    #[props(default = vec![])] sections: Vec<InfoSection>,
    #[props(default = vec![])] links: Vec<InfoLink>,
) -> Element {
    rsx! {
        div {
            dt { class: "flex items-center gap-2 text-xs text-base-content/60",
                span { "{label}" }
                InfoButton { title: label.clone(), body: info, sections, links }
            }
            dd { class: "mt-0.5 font-medium", "{value}" }
        }
    }
}

/// Every authorization the account holds - not just the one in force.
///
/// This is the question the v2.3 API cannot answer at all: `Profile_User` has
/// zero occurrences anywhere in it, and its `Profile` schema carries neither
/// `interface` nor any rights. So the panel above can say "something is
/// missing" and this one can say which profile, on which entity, to fix.
#[component]
fn AuthorizationsPanel() -> Element {
    let rows = use_resource(|| async move { api::fetch_authorizations().await });

    rsx! {
        div { class: "mt-5",
            match &*rows.read_unchecked() {
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-warning",
                        div {
                            div { class: "font-semibold", "Could not read this account's authorizations." }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },
                Some(Ok(list)) => rsx! { AuthTable { rows: list.clone() } },
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Loading…"
                    }
                },
            }
        }
    }
}

#[component]
fn AuthTable(rows: Vec<Authorization>) -> Element {
    rsx! {
        Panel {
            title: "Authorizations".to_string(),
            subtitle: "Every profile this account may use, the one in force first".to_string(),
            info: "What this account is allowed to be in GLPI, and what each of those \
                   options would grant."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Only one is in force",
                    "GLPI activates exactly one profile per session - the account's default - \
                     so the others are available but inactive. Holding a second authorization \
                     does not add its permissions to the first; it only means the account \
                     could be switched to it.",
                ),
                InfoSection::new(
                    "Reading the rights",
                    "Agent and Entity are the two permissions this app needs, shown as GLPI \
                     stores them. Anything other than none means read access is present. A \
                     helpdesk profile always shows none, because those permissions cannot \
                     exist on one.",
                ),
                InfoSection::new(
                    "Where this comes from",
                    "GLPI's current API does not expose authorizations at all - not for this \
                     account, not for any other. This panel reads GLPI's older interface, \
                     which still does. It is the only way to see this without opening GLPI \
                     itself.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Users", "front/user.php"),
                InfoLink::new("Profiles", "front/profile.php"),
            ],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "" }
                            th { "Profile" }
                            th { "Entity" }
                            th { "Interface" }
                            th { "Child entities" }
                            th { "Agent" }
                            th { "Entity right" }
                        }
                    }
                    tbody {
                        for a in rows.iter() {
                            tr { class: if a.is_active { "bg-primary/5" } else { "" },
                                td {
                                    if a.is_active {
                                        span { class: "badge badge-primary badge-sm", "in force" }
                                    }
                                }
                                td { class: "font-medium", "{a.profile}" }
                                td { class: "text-base-content/80", "{a.entity}" }
                                td {
                                    if a.interface == "central" {
                                        span { class: "badge badge-ghost badge-sm", "Standard" }
                                    } else {
                                        span { class: "badge badge-warning badge-sm", "Simplified" }
                                    }
                                }
                                td {
                                    if a.is_recursive {
                                        span { class: "badge badge-success badge-sm", "included" }
                                    } else {
                                        span { class: "badge badge-warning badge-sm", "this entity only" }
                                    }
                                }
                                RightCell { value: a.right_agent }
                                RightCell { value: a.right_entity }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A right as GLPI stores it: a bitmask where bit 1 is read.
#[component]
fn RightCell(value: i64) -> Element {
    rsx! {
        td {
            if value & READ != 0 {
                span { class: "badge badge-success badge-sm", "read" }
            } else {
                span { class: "badge badge-ghost badge-sm text-base-content/50", "none" }
            }
        }
    }
}
