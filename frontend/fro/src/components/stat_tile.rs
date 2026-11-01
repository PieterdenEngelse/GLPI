use crate::components::InfoButton;
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
    info: String,
    /// Colour the figure only when it means something is wrong. A red zero
    /// trains the reader to ignore the colour, and then the one that matters
    /// does not register.
    #[props(default = false)]
    alarm: bool,
) -> Element {
    rsx! {
        div { class: "stat",
            div { class: "stat-title flex items-center gap-2",
                span { "{title}" }
                InfoButton { title: title.clone(), body: info }
            }
            div {
                class: if alarm { "stat-value text-2xl text-error" } else { "stat-value text-2xl" },
                "{value}"
            }
            div { class: "stat-desc", "{desc}" }
        }
    }
}
