//! Placeholder. The route and the nav entry exist; the content does not yet.
//!
//! Deliberately says so on screen rather than showing an empty panel that
//! looks like a page whose data failed to load - the distinction this app
//! insists on everywhere else.
use crate::components::Panel;
use dioxus::prelude::*;

#[component]
pub fn Sec() -> Element {
    rsx! {
        Panel {
            title: "Security".to_string(),
            subtitle: "Nothing here yet".to_string(),
            info: "This page is reserved. It is empty because no checks have been \
                   defined for it, not because something failed to load."
                .to_string(),

            p { class: "max-w-prose text-base-content/80",
                "This page has no content yet. It is a placeholder, not a failure: "
                "nothing is being fetched and nothing has gone wrong."
            }
        }
    }
}
