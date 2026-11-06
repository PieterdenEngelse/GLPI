//! GLPI's own screens, and what can be put on them.
//!
//! Reference, not telemetry - the same kind of page as DB. It answers
//! questions about configuring GLPI that get asked once per deployment and
//! then re-answered from memory, wrongly.
//!
//! Every claim on the CF tab was read in the GLPI 12.0.1 source or in its
//! install schema, both named inline so they can be re-checked against a
//! later release. That matters more here than usual: the widely repeated
//! answer to "how many custom fields can I have" is built on a mechanism
//! GLPI 12 does not use, and it is wrong in a direction that makes people
//! plan around a ceiling that is not there.
use crate::components::{InfoButton, InfoSection, Panel};
use dioxus::prelude::*;

#[component]
pub fn Screens() -> Element {
    // One tab so far. The machinery is db.rs's, so a second is one button
    // and one branch - which is the whole reason it is here rather than the
    // panels being rendered directly.
    let mut tab = use_signal(|| 0usize);

    rsx! {
        div { class: "mb-3",
            InfoButton {
                label: "What is this".to_string(),
                title: "What this page is".to_string(),
                body: "What GLPI's own configuration screens can be made to hold - read out of \
                       GLPI's source, not from its documentation."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why it is here",
                        "These are questions about configuring GLPI rather than about this \
                         app's own data, so nothing on this page is live. They are written \
                         down because they are asked once per deployment and then answered \
                         from memory the second time, and because the commonly circulated \
                         answer to the first of them is wrong.",
                    ),
                    InfoSection::new(
                        "Checked against the source",
                        "Each claim names the file, line or schema it came from in GLPI \
                         12.0.1. A later GLPI release can make any of them false, and naming \
                         the source is what makes that cheap to find out instead of \
                         something this page quietly lies about.",
                    ),
                ],
            }
        }

        div { role: "tablist", class: "tabs tabs-border mb-5",
            button {
                role: "tab",
                class: if tab() == 0 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(0),
                "CF"
            }
        }

        Cf {}
    }
}

/// Custom fields: how many, and what actually constrains the number.
///
/// Three panels as three components, stacked with `mt-5` rather than a
/// `space-y-*` class - db.rs's idiom, and the reason is mechanical: the
/// stylesheet is generated from the class names found in these files, and
/// `space-y-5` is not among the ones already emitted.
#[component]
fn Cf() -> Element {
    rsx! {
        TheMechanism {}
        div { class: "mt-5", ThePracticalLimit {} }
        div { class: "mt-5", WhereToPutItInstead {} }
    }
}

#[component]
fn TheMechanism() -> Element {
    rsx! {
        Panel {
            title: "How many custom fields can an asset have?".to_string(),
            subtitle: "No limit in the interface, and not the one usually quoted".to_string(),
            info: "The mechanism GLPI 12 uses, and why the column-count answer does not apply \
                   to it."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "Values: src/Glpi/Asset/Asset.php line 517 encodes them and line 526 \
                     decodes them. Storage: install/mysql/glpi-11.0.12-empty.sql line 10035, \
                     the last column of glpi_assets_assets. Definitions: the table \
                     glpi_assets_customfielddefinitions in the same file.",
                ),
                InfoSection::new(
                    "The answer this corrects",
                    "The widely repeated answer is that GLPI adds a database column per \
                     custom field, so InnoDB's 1,017-column ceiling leaves roughly 950 to 980 \
                     of them per asset type. The ceiling is real as a property of InnoDB; it \
                     simply is not what GLPI 12 runs into, because it does not add columns. \
                     Anyone planning around ~950 is planning around the wrong structure, and \
                     will also expect a per-type table that does not exist.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "There is "
                span { class: "font-medium", "no limit in the interface" }
                ", and the limit underneath it is not a column count. GLPI 12 does not add a "
                "database column when you add a custom field."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Definitions are rows. " }
                    "Each custom field is a row in "
                    code { class: "text-xs", "glpi_assets_customfielddefinitions" }
                    ", belonging to one asset definition. Adding one inserts a row and "
                    "changes no table's shape."
                }
                li {
                    span { class: "font-medium", "Values are one JSON document. " }
                    "Every custom value for an asset is written into a single "
                    code { class: "text-xs", "custom_fields json" }
                    " column, keyed by the definition's id - "
                    code { class: "text-xs", "json_encode" }
                    " on save, "
                    code { class: "text-xs", "json_decode" }
                    " on read. Fifty custom fields and one custom field occupy the same one "
                    "column."
                }
                li {
                    span { class: "font-medium", "All custom assets share one table. " }
                    code { class: "text-xs", "glpi_assets_assets" }
                    " holds every custom asset type at once, told apart by "
                    code { class: "text-xs", "assets_assetdefinitions_id" }
                    ". It has 27 columns and does not grow. Creating an asset definition does "
                    "not create a table."
                }
                li {
                    span { class: "font-medium", "Tickets have no native custom fields. " }
                    "This is an asset-definition feature. Extra data on a ticket comes from "
                    "Forms or a plugin, not from this mechanism - so none of the above "
                    "transfers to the ticket screen."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "What does bind, then: the size of one JSON document - MariaDB implements "
                code { class: "text-xs", "json" }
                " as "
                code { class: "text-xs", "longtext" }
                ", so the practical ceiling is a server setting rather than a field count - "
                "and, long before that, the interface itself. The next panel is the limit "
                "people actually meet."
            }
        }
    }
}

#[component]
fn ThePracticalLimit() -> Element {
    rsx! {
        Panel {
            title: "The limit you actually meet".to_string(),
            subtitle: "Performance and legibility, not storage".to_string(),
            info: "Why the absence of a hard cap is not an invitation.".to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Not measured here",
                    "These are the failure modes the mechanism above makes likely, and the \
                     figure below is a convention rather than a measurement. Nothing in this \
                     app has benchmarked a GLPI install with hundreds of custom fields, and \
                     the panel does not pretend otherwise.",
                ),
            ],

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Rendering and search. " }
                    "Every field on an asset is rendered each time someone opens it, and "
                    "search and reporting widen with the field list. This is the cost that "
                    "arrives first and is felt on every page load."
                }
                li {
                    span { class: "font-medium", "Imports. " }
                    "CSV import becomes error-prone and slow to the point of timing out once "
                    "there is a large field list to map."
                }
                li {
                    span { class: "font-medium", "A working figure: 15 to 30. " }
                    "The usual rule of thumb for fields per asset type before a screen stops "
                    "being readable. A convention, not a threshold - treat it as the point to "
                    "stop and reorganise rather than a number to reach."
                }
            }
        }
    }
}

#[component]
fn WhereToPutItInstead() -> Element {
    rsx! {
        Panel {
            title: "Where to put data instead".to_string(),
            subtitle: "Three structures, in the order worth trying".to_string(),
            info: "The organising advice holds; one common version of it rests on a table \
                   that does not exist."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "One correction",
                    "A separate asset definition is worth making for the reasons below - a \
                     clean screen and a type that means one thing. It is often recommended \
                     for a different reason: that it yields a fresh table with its own column \
                     budget. It does not. Every custom asset lives in glpi_assets_assets \
                     regardless, so the gain is organisational, not a larger allowance.",
                ),
            ],

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "A dedicated asset definition. " }
                    "Rather than fifty fields on Computer, define the thing you are actually "
                    "tracking - vehicles, IoT devices - and give it its own fields and its "
                    "own screen. The benefit is that each type means one thing; it is not "
                    "extra storage."
                }
                li {
                    span { class: "font-medium", "Forms. " }
                    "For collecting a lot from a person rather than recording a lot about a "
                    "thing. Forms break a long request into steps and keep the answers "
                    "relationally, which is a different shape from an asset's field list and "
                    "the right one for intake."
                }
                li {
                    span { class: "font-medium", "Dropdown trees. " }
                    "Structured values - a location hierarchy, a category tree - belong in a "
                    "nested dropdown, which keeps the structure in one field instead of "
                    "spreading it across twenty text fields that nothing can validate."
                }
            }
        }
    }
}
