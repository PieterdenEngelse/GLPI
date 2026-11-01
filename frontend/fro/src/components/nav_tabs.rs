use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// The application's top-level navigation.
///
/// One list in one place, so adding a page is a single line rather than an
/// edit in the header plus a matching edit wherever else tabs were pasted.
/// RERAG keeps its routes in a `Route` enum but rebuilds the markup per
/// dropdown; with a handful of pages a single list is clearer.
///
/// Active state comes from `use_route`, not a signal the links write to: a
/// signal only knows about clicks, so a pasted URL or the back button would
/// leave the wrong tab lit.
#[component]
pub fn NavTabs() -> Element {
    let current = use_route::<Route>();

    // Discriminant comparison, not equality: Route derives PartialEq, but a
    // variant carrying fields (the 404 carries its segments) would not match
    // a freshly constructed one.
    let is = |r: &Route| std::mem::discriminant(&current) == std::mem::discriminant(r);

    let tabs = [
        (Route::Agents {}, "Agents"),
        (Route::Computers {}, "Computers"),
        (Route::Status {}, "Status"),
        (Route::About {}, "About"),
    ];

    rsx! {
        nav { role: "tablist", class: "tabs tabs-box tabs-sm shrink-0",
            for (route, label) in tabs {
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
