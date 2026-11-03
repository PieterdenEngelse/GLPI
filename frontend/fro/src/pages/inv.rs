//! The inventory section: a landing page and four views beneath it.
//!
//! One module rather than five files. The four pages share a sub-header and
//! are at the same stage, so keeping them together makes the family obvious
//! and the shared nav one definition instead of four copies.
//!
//! None of the four carries data yet, and each says why in its own terms -
//! because the reasons differ. Three are waiting on a page to be written
//! against data GLPI already serves; the fourth is waiting on GLPI, which does
//! not expose refused equipment at all. Collapsing that into one "coming soon"
//! would hide the only one of the four that no amount of work here will fix.
use crate::app::Route;
use crate::components::{InfoLink, InfoSection, Panel};
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// The sub-header. Four links, the active one lit.
///
/// Active state comes from `use_route`, not a signal the links write to: a
/// signal only knows about clicks, so a pasted URL or the back button would
/// leave the wrong link lit - the same reasoning as the main NavTabs.
#[component]
fn InvNav() -> Element {
    let current = use_route::<Route>();
    let is = |r: &Route| std::mem::discriminant(&current) == std::mem::discriminant(r);

    let links = [
        (Route::InvHardware {}, "hardware"),
        (Route::InvSoftware {}, "software"),
        (Route::InvNetwork {}, "network"),
        (Route::InvRefused {}, "refused"),
    ];

    rsx! {
        nav {
            role: "tablist",
            class: "tabs tabs-box tabs-sm mb-5 w-full justify-start",
            for (route, label) in links {
                Link {
                    key: "{label}",
                    class: if is(&route) { "tab tab-active" } else { "tab" },
                    to: route,
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn Inv() -> Element {
    rsx! {
        InvNav {}
        Panel {
            title: "Inventory".to_string(),
            subtitle: "Four views, none of them written yet".to_string(),
            info: "What the inventory agents collect, split by the kind of thing it is."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is separate from Agents and Computers",
                    "Those two pages answer whether the collecting is working - which agents \
                     report, and which machines no agent speaks for. These four are about what \
                     was collected. A fleet can be perfectly healthy by the first measure and \
                     still hold nothing you can act on.",
                ),
                InfoSection::new(
                    "Three of these are waiting on us; one is not",
                    "GLPI already serves the hardware, software and network data through its \
                     API - those pages simply have not been written. Refused equipment is \
                     different: GLPI does not expose it at all, so that page cannot be written \
                     from the API however much time is spent on it. Each page says which of \
                     the two it is.",
                ),
            ],

            p { class: "max-w-prose text-sm text-base-content/80",
                "Pick a view above. Each one states what it will show and what currently \
                 stands in the way, rather than presenting an empty table that reads as a \
                 fleet with nothing in it."
            }
        }
    }
}

/// A view that has data waiting and no page yet.
#[component]
fn Pending(title: String, collection: String, shows: String, extra: Option<Element>) -> Element {
    rsx! {
        InvNav {}
        Panel {
            title: title.clone(),
            subtitle: "Not written yet - the data is there".to_string(),
            info: "This page is empty because it has not been built, not because something \
                   failed to load or because GLPI holds nothing."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where it will come from",
                    &format!(
                        "GLPI serves this as {collection} through the API this app already \
                         speaks, as the same account the Status page names. Nothing new has to \
                         be granted or configured for it."
                    ),
                ),
            ],
            info_links: vec![InfoLink::new("Assets", "front/computer.php")],

            p { class: "max-w-prose text-sm text-base-content/80", "{shows}" }
            if let Some(e) = extra {
                div { class: "mt-4", {e} }
            }
        }
    }
}

#[component]
pub fn InvHardware() -> Element {
    rsx! {
        Pending {
            title: "Hardware".to_string(),
            collection: "its computer, monitor and peripheral collections".to_string(),
            shows: "What the agents found inside each machine - processors, memory, disks and \
                    the devices attached to it. Confirmed reachable: the computer, monitor and \
                    peripheral collections all answer for this account."
                .to_string(),
        }
    }
}

#[component]
pub fn InvNetwork() -> Element {
    rsx! {
        Pending {
            title: "Network".to_string(),
            collection: "its network equipment collection".to_string(),
            shows: "Switches, routers and the rest of the network estate, with what the agents \
                    discovered about them. Confirmed reachable for this account."
                .to_string(),
        }
    }
}

#[component]
pub fn InvSoftware() -> Element {
    rsx! {
        Pending {
            title: "Software".to_string(),
            collection: "its software collection".to_string(),
            shows: "What is installed across the fleet, and on how many machines. Reachable, \
                    but the only one of these four large enough to hit a limit worth knowing \
                    about before the page is written."
                .to_string(),
            extra: rsx! {
                div { class: "alert alert-warning",
                    div {
                        div { class: "font-semibold", "This one needs paging first." }
                        div { class: "text-sm",
                            "GLPI returns at most 100 entries per request and reports that it "
                            "has withheld the rest. This install holds 1330 software entries, "
                            "so a page that asks once and shows what comes back would show "
                            "under a tenth of them. Whatever is built here has to ask "
                            "repeatedly until GLPI says it is done."
                        }
                    }
                }
            },
        }
    }
}

/// The one that is not waiting on us.
#[component]
pub fn InvRefused() -> Element {
    let report = use_resource(|| async move { crate::api::fetch_refused().await });

    rsx! {
        InvNav {}
        Panel {
            title: "Refused".to_string(),
            subtitle: "Submissions GLPI threw away".to_string(),
            info: "Equipment GLPI rejected when an agent tried to submit it. The agent \
                   reports success and exits cleanly; nothing appears; nothing says why."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is the failure worth catching",
                    "The agent runs, reports success and exits cleanly; GLPI rejects what it \
                     sent; the machine never appears, or its record silently stops being \
                     updated. Nothing in the agent's own log says so. Worse, the agent still \
                     counts as reporting on the Agents page, because GLPI records the moment \
                     of contact before it inspects what was sent - so this failure looks \
                     healthy from every other angle.",
                ),
                InfoSection::new(
                    "Where this comes from",
                    "GLPI's database, directly. There is no address for refused equipment \
                     anywhere in its API - not the current one, not the older one - so this \
                     is the single thing in the app that is not read through GLPI's own \
                     interface.",
                ),
                InfoSection::new(
                    "Does that step around GLPI's permissions?",
                    "It would, left alone, which is why it is not left alone. The rows shown \
                     are restricted to the same entities the API reports as visible, so this \
                     page means the same thing by \"everything\" as every other page. The \
                     account used holds read access to two tables and nothing else - less \
                     than the account the rest of the app already signs in with.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Import rules", "front/ruleimportasset.php"),
                InfoLink::new("Agents", "front/agent.php"),
            ],

            match &*report.read_unchecked() {
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Could not read refusals." }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },

                // Not configured is not the same as nothing to report, and
                // must never render as the reassuring one.
                Some(Ok(r)) if !r.configured => rsx! {
                    div { class: "alert alert-warning",
                        div {
                            div { class: "font-semibold", "No database access configured." }
                            div { class: "text-sm",
                                "Refused submissions exist only in GLPI's database, so this \
                                page cannot say whether there are any. That is different \
                                from saying there are none."
                            }
                        }
                    }
                },

                Some(Ok(r)) if r.rows.is_empty() => rsx! {
                    div { class: "rounded-box border border-base-300 p-4 text-base-content/70",
                        "No refused submissions recorded. Every inventory GLPI received was "
                        "accepted."
                    }
                },

                Some(Ok(r)) => rsx! { RefusedTable { rows: r.rows.clone() } },

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
fn RefusedTable(rows: Vec<crate::api::Refused>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "When" }
                        th { "Name" }
                        th { "Type" }
                        th { "Serial" }
                        th { "From" }
                        th { "Rule" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        tr {
                            td { class: "whitespace-nowrap text-base-content/80",
                                "{r.date_creation.clone().unwrap_or_default()}"
                            }
                            td { class: "font-medium", "{r.name.clone().unwrap_or_default()}" }
                            td { class: "text-base-content/80", "{r.itemtype.clone().unwrap_or_default()}" }
                            td { class: "font-mono text-xs", "{r.serial.clone().unwrap_or_default()}" }
                            td { class: "font-mono text-xs text-base-content/70", "{r.ip.clone().unwrap_or_default()}" }
                            td {
                                // GLPI replays the rules against this exact
                                // submission, which answers "why" far better
                                // than a rule name would.
                                if let Some(id) = r.id.into() {
                                    RuleLink { refused_id: id, rules_id: r.rules_id }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RuleLink(refused_id: i64, rules_id: Option<i64>) -> Element {
    let href = crate::glpi_link::link(&format!(
        "front/rulesengine.test.php?sub_type=RuleImportAsset&refusedequipments_id={refused_id}"
    ));
    let label = match rules_id {
        Some(id) => format!("rule {id}"),
        None => "replay".to_string(),
    };

    rsx! {
        if let Some(h) = href {
            a {
                class: "link link-primary text-xs",
                href: "{h}",
                target: "_blank",
                rel: "noopener noreferrer",
                "{label} ↗"
            }
        } else {
            span { class: "text-xs text-base-content/60", "{label}" }
        }
    }
}
