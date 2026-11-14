use crate::app::Route;
use crate::components::nav;
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// The pages, as tabs across the header.
///
/// The destinations themselves are not declared here - they come from
/// components/nav.rs, which the sidebar reads too, so adding a page is one
/// edit rather than two that can disagree. What is local to this strip is how
/// it renders them: every destination marked `in_header`, as a flat row, with
/// no sub-tabs. The sidebar is the one that nests those.
///
/// Active state comes from `use_route`, not a signal the links write to: a
/// signal only knows about clicks, so a pasted URL or the back button would
/// leave the wrong tab lit.
#[component]
pub fn NavTabs() -> Element {
    let current = use_route::<Route>();

    rsx! {
        nav { role: "tablist", class: "tabs tabs-box tabs-sm shrink-0",
            for dest in nav::destinations().into_iter().filter(|d| d.in_header) {
                Link {
                    key: "{dest.label}",
                    class: if nav::is_active(&dest.route, &current) { "tab text-lg tab-active" } else { "tab text-lg" },
                    to: dest.route,
                    "{dest.label}"
                }
            }
        }
    }
}
