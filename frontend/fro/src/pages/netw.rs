//! A page with nothing on it yet.
//!
//! The route, the nav entry and the layout are what exist so far. What goes
//! here is a separate question from where it lives, and guessing at content
//! would be harder to undo than an empty page is to fill.
//!
//! Note what this is NOT: the fleet network view already written at
//! `/inv/network`, which reads v1 collections and is reachable from the Inv
//! sub-header. Pointing this tab there instead was the other reading of the
//! request, and is a one-line change in `nav_tabs.rs` if that is what is
//! wanted.
use dioxus::prelude::*;

#[component]
pub fn Netw() -> Element {
    rsx! {
        // One quiet line rather than zero elements. A page that renders
        // nothing at all is indistinguishable from one that failed to render,
        // and this app draws that distinction everywhere else.
        div { class: "text-sm text-base-content/60", "Nothing here yet." }
    }
}
