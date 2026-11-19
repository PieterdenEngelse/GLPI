use crate::components::{InfoButton, InfoLink, InfoSection};
use dioxus::prelude::*;

/// A titled card. RERAG's monitor pages are built almost entirely from these,
/// which is most of why they read as a dashboard rather than a document: the
/// content is identical, but a bordered surface with a header gives the eye
/// somewhere to start.
#[component]
pub fn Panel(
    title: String,
    #[props(default = None)] subtitle: Option<String>,
    #[props(default = None)] actions: Option<Element>,
    /// Rendered in the title row, immediately after the info button.
    ///
    /// Distinct from `actions`, which justify-between parks at the far end
    /// of the header and which long content therefore wraps onto a line of
    /// its own. This is for content that belongs TO the title rather than
    /// beside it: the tiers panel is the case it exists for, because its
    /// four destinations ARE what that panel is, so they read as part of the
    /// heading instead of as controls parked on the right.
    #[props(default = None)]
    beside_title: Option<Element>,
    /// Explains what this panel shows. RERAG's rule: every control or figure
    /// a reader could misread gets its own modal, not a tooltip.
    #[props(default = None)]
    info: Option<String>,
    /// Longer explanation, rendered as tabs inside the info panel.
    #[props(default = vec![])]
    info_sections: Vec<InfoSection>,
    /// Links into GLPI's own screens, as paths relative to its base URL.
    #[props(default = vec![])]
    info_links: Vec<InfoLink>,
    children: Element,
) -> Element {
    rsx! {
        section { class: "rounded-box border border-base-300 bg-base-100 shadow-sm",
            div { class: "flex flex-wrap items-center justify-between gap-3 border-b border-base-300 px-4 py-3",
                div {
                    // flex-wrap because beside_title can be long: it wraps
                    // under the heading rather than forcing the header wide.
                    div { class: "flex flex-wrap items-center gap-2",
                        h2 { class: "text-sm font-semibold tracking-tight", "{title}" }
                        if let Some(i) = info.clone() {
                            InfoButton { title: title.clone(), body: i, sections: info_sections.clone(), links: info_links.clone() }
                        }
                        if let Some(b) = beside_title {
                            {b}
                        }
                    }
                    if let Some(s) = subtitle {
                        p { class: "text-xs text-base-content/60", "{s}" }
                    }
                }
                if let Some(a) = actions {
                    div { class: "flex items-center gap-2", {a} }
                }
            }
            div { class: "p-4", {children} }
        }
    }
}
