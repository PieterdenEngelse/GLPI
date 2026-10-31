use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// Site chrome: the question this app answers, and the navigation.
///
/// The active tab comes from `use_route` rather than a signal the links
/// write to. A signal would only know about clicks, so a pasted link or a
/// back button would leave the wrong tab lit.
#[component]
pub fn Header() -> Element {
    let current = use_route::<Route>();

    rsx! {
        header { class: "site",
            div { class: "bar",
                div { class: "brand",
                    h1 { "GLPI inventory" }
                    p { class: "sub", "Which agents have stopped reporting" }
                }
                nav {
                    Link {
                        to: Route::Fleet {},
                        class: if matches!(current, Route::Fleet { .. }) { "tab on" } else { "tab" },
                        "Fleet"
                    }
                    Link {
                        to: Route::About {},
                        class: if matches!(current, Route::About { .. }) { "tab on" } else { "tab" },
                        "About"
                    }
                }
            }
        }
    }
}
