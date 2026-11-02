use crate::app::Route;
use crate::components::{HealthDot, NavTabs, ThemeToggle};
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// The app bar: a thin sticky strip, not a page title block.
///
/// Note the deliberate mismatch: `text-lg` on `-sm` chrome. The links were
/// asked to be ~30% larger, and taking the chrome up a size with them added
/// 16px to the bar's height - which defeats the point of a thin bar. Larger
/// text in smaller padding keeps both.
///
/// Shaped after RERAG's header - mark on the left, small centred title with a
/// live status dot, navigation on the right, `sticky top-0` with a raised
/// z-index so it survives scrolling and overlays. The difference from a
/// document heading is the point: this frames an application, so it stays put
/// and stays out of the way rather than consuming the top of every page.
///
/// The active tab comes from `use_route`, not a signal the links write to. A
/// signal only knows about clicks, so a pasted link or the back button would
/// leave the wrong tab lit.
#[component]
pub fn Header() -> Element {
    let current = use_route::<Route>();

    rsx! {
        header {
            class: "sticky top-0 z-50 flex items-center gap-3 border-b border-base-300 bg-base-100/95 px-3 py-1 shadow-sm backdrop-blur",

            // Mark. An inline SVG rather than an asset: one file fewer to
            // hash, bundle and cache-bust for a 20px square.
            div { class: "ml-1 flex h-7 w-7 shrink-0 items-center justify-center rounded bg-primary text-primary-content",
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    class: "h-4 w-4",
                    rect { x: "3", y: "4", width: "18", height: "12", rx: "1" }
                    path { d: "M8 20h8M12 16v4" }
                }
            }

            NavTabs {}

            // Title and live status after the navigation. Allowed to truncate,
            // so a long title shortens itself rather than pushing anything
            // else out of the bar on a narrow window.
            div { class: "flex min-w-0 items-center gap-2",
                h1 { class: "truncate text-sm font-medium text-primary",
                    "GLPI inventory — which agents have stopped reporting"
                }
                HealthDot {}
            }

            // ml-auto, not a spacer element: the toggle sits against the right
            // edge and everything before it keeps its natural width.
            div { class: "ml-auto flex shrink-0 items-center gap-1",
                // Styled as a quiet action rather than a fifth tab: it is not
                // a view of the data, and giving it tab chrome over here would
                // read as a stray member of the group on the left.
                Link {
                    to: Route::About {},
                    class: if matches!(current, Route::About { .. }) {
                        "btn btn-ghost btn-sm text-lg btn-active"
                    } else {
                        "btn btn-ghost btn-sm text-lg text-base-content/70"
                    },
                    "About"
                }
                ThemeToggle {}
            }

        }
    }
}
