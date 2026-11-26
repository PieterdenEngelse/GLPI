//! Report definitions: where the builder will live.
//!
//! A placeholder, deliberately. The nav entry exists now so the destination is
//! settled before the UI is written; the builder itself is step 4 of the plan
//! in `~/docs/gl_report_builder.md`.
//!
//! What already works, and is what this page will eventually compose: GLPI
//! reports are rows in `glpi_plugin_swcat_reports`, served by one
//! parameterised renderer in the `swcat` plugin. The plugin reads the table on
//! every request, so a definition saved here appears under GLPI's
//! Tools > Reports with no deployment at all - no plugin file, and on this
//! host no `nixos-rebuild`, because nothing new lands in /nix/store.
use dioxus::prelude::*;

#[component]
pub fn Reports() -> Element {
    // None until the first health response lands. Rendering no link beats
    // rendering a broken one, which is the rule the rest of the app follows.
    let reports_page = crate::glpi_link::link("front/report.php");

    rsx! {
        section { class: "max-w-prose",
            h2 { class: "mb-1 text-base font-semibold", "What this will do" }
            p { class: "mb-3 text-base-content/80",
                "Compose a GLPI report from dropdowns - pick the item types, the "
                "field to group by, whether a location tree rolls up to its root - "
                "and save it. GLPI has no UI for authoring a report; this is it."
            }

            h2 { class: "mt-7 mb-1 text-base font-semibold", "Not built yet" }
            p { class: "mb-3 text-base-content/80",
                "The plugin side is done: definitions are rows, one renderer "
                "serves them all, and each row becomes its own entry under "
                "Tools > Reports. What is missing is this page - the builder, and "
                "the write path behind it."
            }
            p { class: "mb-3 text-base-content/80",
                "Until then a definition is an INSERT, and the reports already "
                "defined are visible in GLPI under Tools > Reports"
                if let Some(href) = reports_page {
                    ": "
                    a {
                        class: "link link-primary",
                        href: "{href}",
                        target: "_blank",
                        rel: "noopener",
                        "open it"
                    }
                }
                "."
            }

            h2 { class: "mt-7 mb-1 text-base font-semibold", "One thing it will have to admit" }
            p { class: "mb-3 text-base-content/80",
                "A preview shown here is read over SQL, which bypasses GLPI's "
                "entity permissions; the rendered report runs inside GLPI and "
                "does not. So a preview can legitimately show more than the "
                "report it is previewing, and will have to say so rather than "
                "quietly disagree."
            }
        }
    }
}
