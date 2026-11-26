//! The front door: where everything is, and the one setting people arrive
//! asking about.
//!
//! `/` used to be the Agents table. That made the landing page the answer to
//! one question - which agents stopped reporting - at a point where the app
//! had grown twelve more, and a reader who did not already know the tab strip
//! had no map. Agents is now `/agents` and is still the first entry in the
//! navigation, so the one-click route to it is unchanged.
//!
//! THE TILES ARE BUILT FROM `nav::destinations()`, not from a list here. The
//! navigation module owns which pages exist precisely so the header, the
//! sidebar and now this page cannot disagree about it - a page added to that
//! list appears here without anyone remembering to. Only the sentence under
//! each tile lives here, because it belongs to this page and nothing else
//! renders it.
use crate::app::Route;
use crate::components::{nav, LogoutTile};
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "flex flex-col gap-4",

            section {
                h1 { class: "text-base font-semibold", "GLPI inventory" }
                p { class: "mt-1 max-w-prose text-sm text-base-content/70",
                    "GLPI shows the computer an agent created and goes on showing it, looking "
                    "healthy, long after the agent stopped reporting. Nothing in GLPI marks an "
                    "agent as overdue. This does - and reports on the install underneath it."
                }
            }

            // The map. A grid rather than a list: thirteen destinations read
            // as a wall in one column, and the sentence under each is what
            // makes a tile worth more than the tab of the same name.
            section {
                class: "grid gap-3 sm:grid-cols-2 lg:grid-cols-3",
                for dest in nav::destinations() {
                    Tile {
                        key: "{dest.label}",
                        route: dest.route.clone(),
                        label: dest.label,
                        blurb: blurb(&dest.route),
                    }
                }
                Tile {
                    key: "About",
                    route: Route::About {},
                    label: "About",
                    // About is absent from `destinations()` on purpose - it
                    // describes the app rather than showing data, and sits
                    // apart in the header. On a map of where things are,
                    // leaving it out would be a gap rather than a decision.
                    blurb: blurb(&Route::About {}),
                }
            }

            // The logout settings. On the home page and not on a page of
            // their own: five rows, three of them read-only, answering a
            // question people arrive with rather than go looking for.
            LogoutTile {}
        }
    }
}

/// One destination, as a card.
#[component]
fn Tile(route: Route, label: &'static str, blurb: &'static str) -> Element {
    rsx! {
        Link {
            to: route,
            class: "rounded-box border border-base-300 bg-base-100 p-3 shadow-sm transition-colors hover:border-primary hover:bg-base-200",
            div { class: "text-sm font-semibold", "{label}" }
            p { class: "mt-1 text-xs text-base-content/70", "{blurb}" }
        }
    }
}

/// What each destination is for, in one line.
///
/// An exhaustive match, for the same reason `Route::title` is one: a new page
/// does not compile until it has been given a sentence here, which is the
/// only way "every tile says what it is" stays true without anyone
/// remembering to check. The arms that cannot be reached from
/// `destinations()` still have to exist for that exhaustiveness, and they
/// answer honestly rather than with a placeholder - a child route linked from
/// its parent is a legitimate thing to describe.
fn blurb(route: &Route) -> &'static str {
    match route {
        Route::Home {} => "This page.",
        Route::Agents {} => "Which inventory agents have stopped reporting. The reason this app exists.",
        Route::Computers {} => "Every machine GLPI holds, and which of them no agent speaks for.",
        Route::Status {} => "The GLPI identity this app reads with, and what it is allowed to see.",
        Route::Screens { .. } => "GLPI's own screens: custom fields, definitions, rights, and how to change them.",
        Route::TemplateRow {} | Route::TemplateValues {} | Route::TemplateForm {}
        | Route::TemplateMessage {} => "The four different things GLPI calls a template.",
        Route::Inv {} | Route::InvHardware {} | Route::InvSoftware {} | Route::InvNetwork {}
        | Route::InvRefused {} => "What the agents found inside the machines - hardware, software, network, and the submissions GLPI refused.",
        Route::Rights { .. } => "Who may do what: profiles, the API's own rights, and the database grant.",
        Route::Db { .. } => "MariaDB under GLPI: settings, what can be changed, and what a change costs.",
        Route::Netw {} | Route::NetwProxy {} => "This machine's networking, and the proxy in front of GLPI.",
        Route::Sec {} => "What is exposed, what is not checked, and what this app cannot see.",
        Route::Perf {} | Route::PerfHistory { .. } | Route::PerfPhp { .. }
        | Route::PerfNginx {} => "How fast GLPI answers, now and over time.",
        Route::Reports {} => "Saved report definitions, served by GLPI's own Tools > Reports.",
        Route::Logs { .. } => "GLPI's own log files, as far as they are readable.",
        Route::About {} => "What this app judges, and what it is blind to.",
        Route::PageNotFound { .. } => "Nothing is here.",
    }
}
