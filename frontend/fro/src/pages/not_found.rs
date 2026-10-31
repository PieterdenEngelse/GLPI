use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn PageNotFound(#[props(default = vec![])] segments: Vec<String>) -> Element {
    rsx! {
        main {
            div { class: "empty",
                strong { "No such page." }
                p { class: "path", "/{segments.join(\"/\")}" }
                Link { to: Route::Fleet {}, "Back to the fleet" }
            }
        }
    }
}
