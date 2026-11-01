//! Info buttons, to RERAG's spec.
//!
//! Constants, geometry and colour are copied from RERAG's CLAUDE.md rather
//! than redesigned, so the two apps' info buttons are the same object:
//!
//!   PARAM_ICON_BUTTON_CLASS  w-6 h-6 min-w-6 min-h-6 shrink-0 rounded ...
//!   PARAM_ICON_BUTTON_STYLE  background-color:#7C2A02;border:1px solid #7C2A02;
//!   INFO_ICON_SVG_CLASS      w-5 h-5 text-white
//!
//! #7C2A02 is the Rust brand colour and is used flat in both themes on
//! purpose: it is a fixed brand mark, not semantic colour, so it must not
//! follow the light/dark tokens the rest of the app uses.
//!
//! VOICE - also RERAG's rule. These are written for someone using the app,
//! who has no idea it is split into pieces. Say "the app", never "the
//! backend", "the frontend" or "the server". User-visible technical terms
//! (GLPI, entities, profiles, ports) are fine; words from the source code
//! are not.
use dioxus::prelude::*;

pub const PARAM_ICON_BUTTON_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80";
pub const PARAM_ICON_BUTTON_STYLE: &str = "background-color:#7C2A02;border:1px solid #7C2A02;";
pub const INFO_ICON_SVG_CLASS: &str = "w-5 h-5 text-white";

/// The icon itself. Geometry is RERAG's: circle r=9, stem from y=8 to y=14,
/// and a separate filled dot at cy=6.3 - an "i" drawn rather than typeset, so
/// it stays centred at any size without depending on a font.
#[component]
pub fn InfoIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.5",
            class: "{INFO_ICON_SVG_CLASS}",
            circle { cx: "12", cy: "12", r: "9" }
            line { x1: "12", y1: "8", x2: "12", y2: "14", stroke_width: "1.5" }
            circle { cx: "12", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}

/// A button that explains the thing it sits next to.
///
/// Owns its own open state, so adding one is a single element with no signal
/// to thread through the caller. The overlay closes on a click outside, and
/// the panel stops propagation so a click inside does not close it.
#[component]
pub fn InfoButton(title: String, body: String) -> Element {
    let mut open = use_signal(|| false);

    rsx! {
        button {
            class: "{PARAM_ICON_BUTTON_CLASS}",
            style: "{PARAM_ICON_BUTTON_STYLE}",
            title: "What this is",
            aria_label: "What \"{title}\" means",
            onclick: move |_| open.set(true),
            InfoIcon {}
        }

        if open() {
            div {
                class: "fixed inset-0 z-[100] flex items-start justify-center bg-black/60 p-4 pt-24 backdrop-blur-sm",
                onclick: move |_| open.set(false),
                div {
                    class: "w-full max-w-md rounded-box border border-base-300 bg-base-100 p-5 shadow-2xl",
                    onclick: move |e: Event<MouseData>| e.stop_propagation(),
                    div { class: "mb-3 flex items-center justify-between gap-3",
                        h3 { class: "text-base font-semibold", "{title}" }
                        button {
                            class: "btn btn-ghost btn-sm",
                            onclick: move |_| open.set(false),
                            "Close"
                        }
                    }
                    p { class: "text-sm leading-relaxed text-base-content/80", "{body}" }
                }
            }
        }
    }
}
