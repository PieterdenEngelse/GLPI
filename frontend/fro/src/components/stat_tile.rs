use crate::components::{InfoButton, InfoLink, InfoSection};
use dioxus::prelude::*;

/// One figure, with the explanation attached to it rather than to a legend
/// somewhere else.
///
/// A dashboard number is exactly the kind of thing a reader guesses at - is
/// "stale" hours or days, does it count machines or agents - and a guess that
/// happens to be wrong is worse than no number.
#[component]
pub fn StatTile(
    title: String,
    value: String,
    desc: String,
    body: String,
    #[props(default = vec![])] sections: Vec<InfoSection>,
    #[props(default = vec![])] links: Vec<InfoLink>,
    /// Colour the figure only when it means something is wrong. A red zero
    /// trains the reader to ignore the colour, and then the one that matters
    /// does not register.
    #[props(default = false)]
    alarm: bool,
    /// A control belonging to this figure, rendered under the description.
    ///
    /// Distinct from `links`, which live inside the info modal and leave the
    /// app for GLPI's own screens. This is for showing what the figure counts
    /// without going anywhere - the rules tab unfolds its rule list with it.
    /// Under the description rather than beside the title, so the figure
    /// stays the thing the eye lands on.
    #[props(default = None)]
    action: Option<Element>,
) -> Element {
    rsx! {
        div { class: "stat",
            div { class: "stat-title flex items-center gap-2",
                span { "{title}" }
                InfoButton { title: title.clone(), body, sections, links }
            }
            div {
                class: if alarm { "stat-value text-2xl text-error" } else { "stat-value text-2xl" },
                "{value}"
            }
            div { class: "stat-desc", "{desc}" }
            if let Some(a) = action {
                // whitespace-normal: .stat-desc is nowrap, which would push a
                // link of any length out past the tile instead of wrapping.
                div { class: "stat-desc mt-1 whitespace-normal", {a} }
            }
        }
    }
}
