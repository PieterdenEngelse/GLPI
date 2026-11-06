//! GLPI's own screens, and what can be put on them.
//!
//! Reference, not telemetry - the same kind of page as DB. It answers
//! questions about configuring GLPI that get asked once per deployment and
//! then re-answered from memory, wrongly.
//!
//! Every claim on both tabs was read in the GLPI 12.0.1 source or in its
//! install schema, both named inline so they can be re-checked against a
//! later release. That matters more here than usual, and for a different
//! reason on each tab:
//!
//! CF - the widely repeated answer to "how many custom fields can I have" is
//! built on a mechanism GLPI 12 does not use, and it is wrong in a direction
//! that makes people plan around a ceiling that is not there.
//!
//! Def - disabling a capacity PURGES the data it managed, with history off,
//! for 27 of the 28. Nothing about the word "capacity" suggests that, and
//! the mistake is not recoverable, so it is the one thing on this page worth
//! reading before acting rather than after.
use crate::components::{InfoButton, InfoLink, InfoSection, Panel};
use dioxus::prelude::*;

#[component]
pub fn Screens() -> Element {
    // Two tabs. The machinery is db.rs's - one button each and one branch -
    // which is the whole reason the panels are behind components rather than
    // rendered directly here.
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
            button {
                role: "tab",
                class: if tab() == 1 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(1),
                "Def"
            }
        }

        if tab() == 1 {
            Def {}
        } else {
            Cf {}
        }
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

/// Making a screen, and the one adjustment that destroys data.
///
/// Same three-panel shape as `Cf`, and `mt-5` wrappers for the same
/// mechanical reason: the stylesheet is generated from the class names found
/// in these files, and `space-y-5` is not among the ones already emitted.
#[component]
fn Def() -> Element {
    rsx! {
        WhatADefinitionCreates {}
        div { class: "mt-5", DisablingACapacityDeletes {} }
        div { class: "mt-5", HidingVersusRemoving {} }
    }
}

#[component]
fn WhatADefinitionCreates() -> Element {
    rsx! {
        Panel {
            title: "What a custom asset definition creates".to_string(),
            subtitle: "A class, not a table".to_string(),
            info: "What is actually created when you define a new asset type, and what \
                   therefore does not get created."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "Generated class name: src/Glpi/Asset/AssetDefinition.php line 100. The \
                     four JSON columns that hold the whole configuration: line 278. The four \
                     tabs: getTabNameForItem at line 113, dispatched by \
                     displayTabContentForItem at line 157.",
                ),
                InfoSection::new(
                    "The answer this corrects",
                    "Splitting a type into its own definition is often recommended on the \
                     grounds that it yields a fresh table with its own column budget. It does \
                     not. Every custom asset of every definition lives in glpi_assets_assets, \
                     told apart by assets_assetdefinitions_id, so the gain from splitting is \
                     organisational - each type means one thing - and never storage.",
                ),
            ],
            info_links: vec![InfoLink::new("Asset definitions", "front/asset/assetdefinition.php")],

            p { class: "mb-3 text-sm",
                "A definition creates a "
                span { class: "font-medium", "class" }
                ", not a table. Its system_name becomes "
                code { class: "text-xs", "asset_<system_name>" }
                " and the records go in the one shared assets table."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Nothing is migrated. " }
                    "The entire shape of the screen is four JSON columns on one row - "
                    code { class: "text-xs", "capacities" }
                    ", "
                    code { class: "text-xs", "fields_display" }
                    ", "
                    code { class: "text-xs", "profiles" }
                    ", "
                    code { class: "text-xs", "translations" }
                    ". That is why defining a type needs no schema change, and also why none \
                     of it is visible to a query that does not know to look inside JSON."
                }
                li {
                    span { class: "font-medium", "Four tabs, and that is the whole surface. " }
                    "Capacities decide which sections the screen has, Fields which fields and \
                     in what order, Profiles who may see it, Translations what it is called. \
                     If what you want is none of those four, it is not an asset-definition \
                     adjustment."
                }
                li {
                    span { class: "font-medium", "Tickets are out of scope. " }
                    "Custom fields are an asset-definition feature, so none of this reaches \
                     the ticket screen - extra data on a ticket comes from Forms or a plugin."
                }
            }
        }
    }
}

#[component]
fn DisablingACapacityDeletes() -> Element {
    rsx! {
        Panel {
            title: "Turning a capacity off deletes its data".to_string(),
            subtitle: "27 of the 28, purged and unlogged".to_string(),
            info: "The one action here that cannot be undone, and the word for it does not \
                   warn you."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "The save-time diff that triggers it: src/Glpi/Asset/AssetDefinition.php \
                     lines 405 to 428, and the same path on purge at line 436. The deletion \
                     itself is per capacity - AbstractCapacity::onCapacityDisabled at line \
                     193 is an empty default, and the pattern every other capacity follows is \
                     HasVolumesCapacity.php lines 95 to 109.",
                ),
                InfoSection::new(
                    "How the 27 was counted",
                    "Every class in src/Glpi/Asset/Capacity/ was checked for an \
                     onCapacityDisabled override and for a delete call inside it. Twenty-seven \
                     of the twenty-eight delete. The exception is AllowedInGlobalSearch, which \
                     overrides the method and deletes nothing, because it owns no data - it \
                     only toggles whether the type appears in header-bar search.",
                ),
                InfoSection::new(
                    "Before you do it",
                    "Take a database dump. There is no GLPI console command for one - the \
                     12.0.1 command list has no database:dump - so it is mariadb-dump, as in \
                     the setup manual's backup step. A capacity you only want hidden is a \
                     Fields question, which the next panel covers, and destroys nothing.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "Disabling a capacity does not hide a section. It "
                span { class: "font-medium", "purges the data that section held" }
                ", on every asset of that type."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "force: true. " }
                    "A purge, not a move to trash. There is nothing to restore from."
                }
                li {
                    span { class: "font-medium", "history: false. " }
                    "The deletion is not written to the history either, so afterwards there \
                     is no record that the data ever existed."
                }
                li {
                    span { class: "font-medium", "Display preferences go too. " }
                    "The column layouts built on that capacity's search options are deleted \
                     in the same call, so a global list layout can disappear as collateral."
                }
                li {
                    span { class: "font-medium", "The heaviest two. " }
                    "HasSoftwares and IsInventoriable carry the most deletion calls of the \
                     twenty-seven - seven and five - which is a rough measure of how much \
                     they tear out."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Whether GLPI shows a confirmation first is "
                span { class: "font-medium", "not known here" }
                " - this was read in the source, and the interface was never driven. Assume \
                 it does not."
            }
        }
    }
}

#[component]
fn HidingVersusRemoving() -> Element {
    rsx! {
        Panel {
            title: "Hiding a field is not removing a section".to_string(),
            subtitle: "One keeps the data, the other does not".to_string(),
            info: "The two ways to simplify a screen, and why only one of them is safe."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "Field order: src/Glpi/Asset/AssetDefinition.php lines 340 to 350 - the \
                     array index IS the order, so there is no separate layout model. The \
                     fallback when the JSON is empty or unparseable: getDefaultFieldsDisplay \
                     at line 805. The placeable core fields: getAllFields at line 710.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "Both of these shorten a screen. They differ in what survives."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Remove a field from Fields - data kept. " }
                    "The field leaves the form and the stored values are untouched. This is \
                     the safe way to simplify a screen and is almost always what was wanted."
                }
                li {
                    span { class: "font-medium", "Disable a capacity - data destroyed. " }
                    "See the panel above. Reach for this only when you mean the data to go."
                }
                li {
                    span { class: "font-medium", "Order is the layout. " }
                    "The sequence of the field list is the sequence on the form - "
                    code { class: "text-xs", "fields_display" }
                    " is a list of keys whose index is their order. Reordering the list \
                     reorders the screen; there is nothing else to position."
                }
                li {
                    span { class: "font-medium", "Empty means everything. " }
                    "An empty or unreadable fields_display falls back to the full field list, \
                     not to none. A definition that suddenly shows every field has probably \
                     had that JSON invalidated rather than reset."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Seventeen core fields are placeable, plus the type and model dropdowns and \
                 every custom field. None of this applies to GLPI's built-in types: a \
                 Computer has no fields_display, and what is adjustable there is which \
                 columns a list shows - a search_config right, personal or global."
            }
        }
    }
}
