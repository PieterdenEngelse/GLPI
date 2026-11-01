use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn PageNotFound(#[props(default = vec![])] segments: Vec<String>) -> Element {
    rsx! {
        // Outside the Layout by design - a 404 should not pretend to be a
        // page of the app - so it carries the surface itself rather than
        // rendering on whatever the browser defaults to.
        div { class: "flex min-h-screen flex-col bg-base-200 p-4 text-base-content",
            div { class: "rounded-box border border-base-300 bg-base-100 p-5 shadow-sm",
                div { class: "font-semibold", "No such page." }
                p { class: "mt-1 font-mono text-xs text-base-content/60", "/{segments.join(\"/\")}" }
                Link { to: Route::Fleet {}, class: "link link-primary mt-3 inline-block",
                    "Back to the fleet"
                }
            }
        }
    }
}
