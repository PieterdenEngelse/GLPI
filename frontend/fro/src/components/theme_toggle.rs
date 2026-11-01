use crate::theme;
use dioxus::prelude::*;

/// Switches between the two themes and remembers the choice.
///
/// Shows the theme you would switch TO, not the one you are in: a button is a
/// verb, and an icon of the current state reads as a status light that
/// mysteriously does something when pressed.
#[component]
pub fn ThemeToggle() -> Element {
    let mut current = theme::use_theme();

    let is_dark = current() == theme::DARK;
    let (next, label) = if is_dark {
        (theme::LIGHT, "Switch to light theme")
    } else {
        (theme::DARK, "Switch to dark theme")
    };

    rsx! {
        button {
            // btn-sm, not btn-xs: this box is driven by a touchpad, where a
            // small target costs a second attempt.
            class: "btn btn-ghost btn-sm btn-square shrink-0",
            title: "{label}",
            aria_label: "{label}",
            onclick: move |_| {
                theme::apply(next);
                current.set(next.to_string());
            },
            if is_dark {
                // Sun: pressing this gives you light.
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    class: "h-4 w-4",
                    circle { cx: "12", cy: "12", r: "4" }
                    path { d: "M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4" }
                }
            } else {
                // Moon: pressing this gives you dark.
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "h-4 w-4",
                    path { d: "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" }
                }
            }
        }
    }
}
