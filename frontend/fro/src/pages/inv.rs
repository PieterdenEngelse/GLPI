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
    rsx! {
        InvNav {}
        Panel {
            title: "Refused".to_string(),
            subtitle: "GLPI does not expose this".to_string(),
            info: "Equipment GLPI rejected when an agent tried to submit it. This page is \
                   empty because the data is not available through the API at all - not \
                   because it has not been built."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this one is different",
                    "The other three views are waiting on a page to be written against data \
                     GLPI already serves. This one is not. There is no address for refused \
                     equipment anywhere in GLPI's current API, and asking for the obvious one \
                     is answered with a flat not-found. No amount of work in this app changes \
                     that.",
                ),
                InfoSection::new(
                    "Why it is worth a page anyway",
                    "Because it is the failure this dashboard would most want to show. The \
                     agent runs, reports success and exits cleanly; GLPI rejects what it sent; \
                     the machine never appears. Nothing in the agent's own log says so, and \
                     nothing on any other page here will either - an asset that was refused \
                     looks exactly like one that was never installed.",
                ),
                InfoSection::new(
                    "What would be needed",
                    "The records exist in GLPI's database, so reading them means going outside \
                     the API - which would step around the permission and entity checks every \
                     request in this app currently passes through. That is a deliberate \
                     decision nobody has taken, and this page exists to keep the question \
                     visible rather than let it be quietly forgotten.",
                ),
            ],
            info_links: vec![InfoLink::new("Agents", "front/agent.php")],

            div { class: "alert alert-warning",
                div {
                    div { class: "font-semibold", "Not reachable through GLPI's API." }
                    div { class: "text-sm",
                        "Refused equipment appears nowhere in the current API, and the address \
                        it would occupy answers not-found. This is a gap in GLPI, not an \
                        unfinished page here - which is why it says so instead of showing an \
                        empty table."
                    }
                }
            }
        }
    }
}
