//! GLPI's own screens, and what can be put on them.
//!
//! Reference, not telemetry - the same kind of page as DB. It answers
//! questions about configuring GLPI that get asked once per deployment and
//! then re-answered from memory, wrongly.
//!
//! Every claim on all three tabs was read in the GLPI 12.0.1 source or in
//! its install schema, both named inline so they can be re-checked against a
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
//!
//! Rights - a new asset type is invisible to every profile, including the one
//! you are looking at it from, until its own right is granted. The screen
//! looks finished and simply is not in the menu, which sends people back to
//! re-check a definition that was never the problem.
//!
//! Instruct - the other three tabs answer questions. This one is the ordered
//! procedure they are answers about: six steps from an empty Setup page to a
//! working asset type, in the order that avoids rework, because two of the
//! decisions cannot be taken back once data exists. Written from
//! ~/docs/gl-screens.md, which carries the same material at length.
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
            button {
                role: "tab",
                class: if tab() == 2 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(2),
                "Rights"
            }
            button {
                role: "tab",
                class: if tab() == 3 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(3),
                "Instruct"
            }
        }

        if tab() == 3 {
            Instruct {}
        } else if tab() == 2 {
            Rights {}
        } else if tab() == 1 {
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

/// Who can see a screen, and who can change it.
///
/// Three panels, stacked with `mt-5` for the same mechanical reason as `Cf`
/// and `Def`: the stylesheet is generated from the class names found in
/// these files, so only classes already emitted here are safe to use.
#[component]
fn Rights() -> Element {
    rsx! {
        InvisibleUntilGranted {}
        div { class: "mt-5", TwoBitsOfSearchConfig {} }
        div { class: "mt-5", VaryByProfile {} }
    }
}

#[component]
fn InvisibleUntilGranted() -> Element {
    rsx! {
        Panel {
            title: "A new asset type is invisible until its right is granted".to_string(),
            subtitle: "One right per definition, named after it".to_string(),
            info: "Why a finished definition does not appear in the menu.".to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "The name: getCustomObjectRightname at src/Glpi/Asset/AssetDefinition.php \
                     line 98, which returns sprintf('asset_%s', strtolower(system_name)). \
                     Storage: glpi_profilerights in install/mysql/glpi-empty.sql - one row \
                     per profile and right name, unique on (profiles_id, name).",
                ),
                InfoSection::new(
                    "Why this wastes time specifically",
                    "Nothing looks broken. The definition saved, it is active, its fields \
                     are there - and it is absent from the menu, so the natural move is to \
                     go back and re-check the definition, which was never the problem. The \
                     super-admin profile is not exempt: the right did not exist when that \
                     profile was created, so it is not held.",
                ),
            ],
            info_links: vec![InfoLink::new("Profiles", "front/profile.php")],

            p { class: "mb-3 text-sm",
                "Creating a definition creates a "
                span { class: "font-medium", "new right" }
                ", and no profile holds it yet."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "The name is derived, not chosen. " }
                    code { class: "text-xs", "asset_" }
                    " plus the system name, lowercased - so a definition named "
                    code { class: "text-xs", "Vehicle" }
                    " is governed by "
                    code { class: "text-xs", "asset_vehicle" }
                    ". Renaming is not possible, so neither is renaming the right."
                }
                li {
                    span { class: "font-medium", "It is per type, not per feature. " }
                    "Ten definitions mean ten rights to grant. There is no single \
                     \"custom assets\" right that covers them all."
                }
                li {
                    span { class: "font-medium", "Grant it in Profiles, not in the definition. " }
                    "The definition's own Profiles tab selects which profiles the type is \
                     offered to; the right itself is a row in "
                    code { class: "text-xs", "glpi_profilerights" }
                    "."
                }
            }
        }
    }
}

#[component]
fn TwoBitsOfSearchConfig() -> Element {
    rsx! {
        Panel {
            title: "Personal columns and everyone's columns are one right".to_string(),
            subtitle: "Two bits of search_config: 1024 and 2048".to_string(),
            info: "What those two numbers are, and why they are not unique to this right."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "The constants: src/DisplayPreference.php lines 55 and 56, PERSONAL = \
                     1024 and GENERAL = 2048, with the right name search_config at line 53 \
                     and the two checks at lines 80 to 85. The bit vocabulary: \
                     src/autoload/constants.php lines 67 to 79. The defaults: \
                     install/empty_data.php, the six search_config rows.",
                ),
                InfoSection::new(
                    "Reuse is wholesale",
                    "In GLPI 12.0.1 bit 10 carries 15 distinct meanings and bit 11 \
                     carries 10 - counting distinct constant NAMES plus the global \
                     one, not declarations: READALL and PARENT are each declared by \
                     two classes and mean the same thing twice. Under search_config \
                     they are PERSONAL \
                     and GENERAL; under ticket, 1024 is READALL; under planning it is \
                     READGROUP; under reservation it is RESERVEANITEM; under knowbase it is \
                     KNOWBASEADMIN. Harmless, because glpi_profilerights is unique on \
                     (profiles_id, name), so the integer is scoped to one right name.",
                ),
                InfoSection::new(
                    "Nothing carries across",
                    "READALL and READGROUP are SWAPPED between two rights: under ticket \
                     READALL is 1024 and READGROUP is 2048, and under planning it is the \
                     other way round. So 1024 is not \"owned-ish\" or \"all-ish\" - it is \
                     only bit 10 of whichever row you are reading, which is why a rights \
                     value cannot be interpreted without its name.",
                ),
            ],
            info_links: vec![InfoLink::new("Display preferences", "front/displaypreference.form.php")],

            p { class: "mb-3 text-sm",
                "\"Set these columns for everyone\" is not a different screen. It is the \
                 same mechanism with a second bit set."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "1024 is 2 to the 10th - PERSONAL. " }
                    "My own column layout for this list."
                }
                li {
                    span { class: "font-medium", "2048 is 2 to the 11th - GENERAL. " }
                    "The default layout everyone without a personal one gets."
                }
                li {
                    span { class: "font-medium", "3072 is both. " }
                    "The two are independent bits of one integer, so a profile can hold \
                     either, both, or neither."
                }
                li {
                    span { class: "font-medium", "Which you hold decides what the UI offers. " }
                    "If the global option is not shown, the profile lacks GENERAL. There is \
                     no other screen to go looking for - it is a rights question wearing the \
                     costume of a missing feature."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "The shipped defaults are worth knowing because they are not what people \
                 assume: Super-Admin and Admin get 3072, Observer gets 1024, and \
                 Self-Service, Hotliner and Technician get 0 - so a technician cannot \
                 rearrange even their own columns until the right is granted."
            }
        }
    }
}

#[component]
fn VaryByProfile() -> Element {
    rsx! {
        Panel {
            title: "Vary one screen by profile - do not clone the definition".to_string(),
            subtitle: "readonly and hidden take profiles, not true or false".to_string(),
            info: "The option that makes a second definition unnecessary.".to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "src/Glpi/Asset/CustomFieldType/AbstractType.php lines 75 to 83: every \
                     field gets full_width, required, readonly and hidden. The last two are \
                     ProfileRestrictOption, not BooleanOption - see \
                     src/Glpi/Asset/CustomFieldOption/ProfileRestrictOption.php, which \
                     carries an \"All\" label at line 52.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "Two of the four options every custom field has are "
                span { class: "font-medium", "sets of profiles" }
                ", which is easy to miss when they read like checkboxes."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "readonly - visible, not editable. " }
                    "For the profiles named. Everyone else edits it normally."
                }
                li {
                    span { class: "font-medium", "hidden - not rendered at all. " }
                    "For the profiles named. The stored value is untouched, so this is \
                     reversible in a way that disabling a capacity is not."
                }
                li {
                    span { class: "font-medium", "This is the answer to two audiences. " }
                    "Technicians see the field, the helpdesk does not, one definition. \
                     Cloning the definition instead gives two types that mean the same \
                     thing, two sets of data that cannot be searched together, and two \
                     places to add every future field."
                }
                li {
                    span { class: "font-medium", "required is a plain boolean. " }
                    "It cannot be varied by profile - a mandatory field is mandatory for \
                     everyone who can see the form."
                }
            }
        }
    }
}

/// The ordered procedure: an empty Setup page to a working asset type.
///
/// Seven panels rather than one long list, because the steps are not equally
/// reversible and the panels are where that gets said. Steps 2 and 3 contain
/// the two decisions that cannot be taken back once a definition holds data,
/// and each carries its own warning rather than relying on a reader having
/// met it in the Def tab.
#[component]
fn Instruct() -> Element {
    rsx! {
        StepZero {}
        div { class: "mt-5", StepOneName {} }
        div { class: "mt-5", StepTwoCapacities {} }
        div { class: "mt-5", StepThreeFields {} }
        div { class: "mt-5", StepFourRight {} }
        div { class: "mt-5", StepFiveActivate {} }
        div { class: "mt-5", StepSixProve {} }
    }
}

#[component]
fn StepZero() -> Element {
    rsx! {
        Panel {
            title: "Before step 1: decide what you are building".to_string(),
            subtitle: "Four subsystems, plus one request that is not a screen at all".to_string(),
            info: "Picking the wrong one is the expensive mistake - far more so \
                   than mis-configuring the right one."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "Menu placement comes from each class's getSectorizedDetails: \
                     src/Glpi/Asset/AssetDefinition.php line 70 returns \
                     ['config', self::class], so asset definitions sit under \
                     Setup; src/Glpi/Dropdown/DropdownDefinition.php line 53 puts \
                     custom dropdowns under Setup then Dropdowns.",
                ),
                InfoSection::new(
                    "Asset or form",
                    "The line that is most often crossed. An asset records facts \
                     ABOUT A THING you will look up later; a form collects answers \
                     FROM A PERSON at request time. Intake questions on an asset \
                     screen give you a form you cannot sequence; asset properties \
                     in a form give you data you cannot search across the fleet.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Asset definitions", "front/asset/assetdefinition.php"),
                InfoLink::new("Custom dropdowns", "front/dropdown/dropdowndefinition.php"),
                InfoLink::new("Forms", "front/form/form.php"),
                InfoLink::new("Asset dashboard", "front/dashboard_assets.php"),
            ],

            p { class: "mb-3 text-sm",
                "The rest of this tab builds a "
                span { class: "font-medium", "custom asset definition" }
                ". Check first that it is what you want."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "A kind of thing to record - vehicles, IoT devices. " }
                    "A custom asset definition, with its own menu entry, list and form. Steps 1 to 6 \
                     below build one."
                }
                li {
                    span { class: "font-medium", "A list of values to pick from. " }
                    "A custom dropdown definition - Setup then Dropdowns - managed like Locations or \
                     Manufacturers."
                }
                li {
                    span { class: "font-medium", "Questions put to a person. " }
                    "Forms, under Service catalogue. They sequence and condition questions and keep answers \
                     relationally."
                }
                li {
                    span { class: "font-medium", "Figures to look at. " }
                    "A dashboard - a grid of widgets, read-only by nature, edited in place from the dashboard \
                     itself."
                }
                li {
                    span { class: "font-medium", "Different columns on a list that already exists. " }
                    "Not a new screen at all. That is a display preference, and whether you can set it \
                     for everyone is a rights question - see the Rights tab."
                }
            }
        }
    }
}

#[component]
fn StepOneName() -> Element {
    rsx! {
        Panel {
            title: "Step 1 - choose the system name".to_string(),
            subtitle: "Everything else derives from it, and it cannot be changed".to_string(),
            info: "The name is not a label. A class name, a right name and every \
                   stored reference are built from it."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "The pattern is SYSTEM_NAME_PATTERN at \
                     src/Glpi/CustomObject/AbstractDefinition.php line 69, applied \
                     at line 407; uniqueness is checked lowercased at line 429. The \
                     reserved list is getReservedSystemNamesPattern at \
                     src/Glpi/Asset/AssetDefinitionManager.php line 160, matched \
                     case-insensitively.",
                ),
                InfoSection::new(
                    "Why first",
                    "A definition's system name has no rename. The generated class, \
                     the right asset_<name> and every profile row that references it \
                     all derive from the string typed here, so changing your mind \
                     later means deleting the definition and starting again - with \
                     whatever it held.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Asset definitions", "front/asset/assetdefinition.php"),
                InfoLink::new("New definition", "front/asset/assetdefinition.form.php"),
            ],

            p { class: "mb-3 text-sm",
                "Open Setup, add a definition, and take your time over this one field."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "The pattern: " }
                    code { class: "text-xs", "[A-Za-z][A-Za-z0-9_]*[A-Za-z0-9]" }
                    " - start with a letter, end alphanumeric, so at least two characters and never a \
                     trailing underscore. "
                    code { class: "text-xs", "Vehicle" }
                    ", "
                    code { class: "text-xs", "IoT_Device" }
                    " pass; "
                    code { class: "text-xs", "_Vehicle" }
                    " and "
                    code { class: "text-xs", "V" }
                    " do not."
                }
                li {
                    span { class: "font-medium", "Fifteen names are reserved, case-insensitively. " }
                    "Computer, Monitor, Software, NetworkEquipment, Peripheral, Printer, Cartridge, \
                     Consumable, Phone, Rack, Enclosure, PDU, PassiveDCEquipment, Unmanaged, Cable - \
                     and so are computer and COMPUTER."
                }
                li {
                    span { class: "font-medium", "It must be unique, compared lowercased. " }
                    code { class: "text-xs", "Vehicle" }
                    " and "
                    code { class: "text-xs", "vehicle" }
                    " cannot both exist."
                }
                li {
                    span { class: "font-medium", "What it becomes. " }
                    code { class: "text-xs", "Vehicle" }
                    " gives you the class "
                    code { class: "text-xs", "VehicleAsset" }
                    " and the right "
                    code { class: "text-xs", "asset_vehicle" }
                    ". No new database table is created - every custom asset of every type shares "
                    code { class: "text-xs", "glpi_assets_assets" }
                    "."
                }
            }
        }
    }
}

#[component]
fn StepTwoCapacities() -> Element {
    rsx! {
        Panel {
            title: "Step 2 - enable capacities, before any data exists".to_string(),
            subtitle: "Enabling is free; disabling destroys what it held".to_string(),
            info: "The step to get right while the definition is still empty.".to_string(),
            info_sections: vec![
                InfoSection::new(
                    "What a disable runs",
                    "HasVolumesCapacity::onCapacityDisabled, lines 95 to 110, is the \
                     shape of all of them: deleteByCriteria with force: true and \
                     history: false, then the relation logs, then the display \
                     preferences. force bypasses the trash; history off means \
                     nothing records that it happened.",
                ),
                InfoSection::new(
                    "27 purge, 26 add a tab",
                    "Two different splits of the 28, easily confused. On DELETION: \
                     27 purge, and only AllowedInGlobalSearch does not, because its \
                     whole disable path is one unregisterFromTypeConfig call and it \
                     owns no data. On TABS: 26 register one, and AllowedInGlobalSearch \
                     and IsRackable do not. Not a reassuring ratio either way - the \
                     safe one is the one you would never have worried about.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "A capacity bolts a whole subsystem onto the type - volumes, software, network ports, \
                 financial information, inventory. There are 28, and 26 of them register a tab \
                 on the asset at boot - AllowedInGlobalSearch and IsRackable are the two that \
                 do not."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Decide them now. " }
                    "Enabling is additive and safe at any time. Disabling is a purge, and it is silent."
                }
                li {
                    span { class: "font-medium", "Never toggle one to see what it does. " }
                    "Not on a definition holding real records. Try it on an empty one."
                }
                li {
                    span { class: "font-medium", "Off and on again restores nothing. " }
                    "You get the tab back, empty."
                }
                li {
                    span { class: "font-medium", "Take a database dump first. " }
                    "Before disabling anything on a populated type. There is no undo in the interface \
                     and the history will not even show that it occurred."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Disabling also deletes the display preferences for that capacity's columns, so any \
                 list layouts people had configured go with the data."
            }
        }
    }
}

#[component]
fn StepThreeFields() -> Element {
    rsx! {
        Panel {
            title: "Step 3 - add the fields".to_string(),
            subtitle: "A field's type and name are fixed once saved".to_string(),
            info: "Nine types, four options on every one of them, and two \
                   properties you cannot revise."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Two name rules",
                    "The definition and its fields are validated differently, and \
                     the inner one is stricter. A field name must match /^[a-z0-9_]+$/ \
                     - lowercase only - where a definition name may be mixed case. \
                     So IoTSensor is a valid definition name and an invalid field \
                     name. Read at src/Glpi/Asset/CustomFieldDefinition.php line 172.",
                ),
                InfoSection::new(
                    "Vary, do not clone",
                    "readonly and hidden are ProfileRestrictOptions, not booleans: \
                     each takes a SET OF PROFILES. One definition can therefore show \
                     a field to technicians and hide it from the helpdesk. A second \
                     definition for the same real thing gives you two types that mean \
                     one thing and two places to add every future field.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "Nine types: Boolean, Date, DateTime, Dropdown, Number, Raw, String, Text, URL."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Get the type right first time. " }
                    "Type and system name cannot be changed on a saved field. A String that should have \
                     been a Dropdown must be created anew under another name."
                }
                li {
                    span { class: "font-medium", "Four options on every field: " }
                    code { class: "text-xs", "full_width" }
                    ", "
                    code { class: "text-xs", "required" }
                    ", "
                    code { class: "text-xs", "readonly" }
                    " and "
                    code { class: "text-xs", "hidden" }
                    ". The last two take profiles; required is a plain boolean and applies to everyone."
                }
                li {
                    span { class: "font-medium", "Per type: " }
                    "Number adds min and max, Dropdown adds multiple, Text adds rich text - which is "
                    span { class: "font-medium", "on by default" }
                    ", worth turning off if the value will be exported or matched."
                }
                li {
                    span { class: "font-medium", "The name becomes a search key. " }
                    code { class: "text-xs", "custom_<name>" }
                    ", permanently. Name it as you would a column, not as a caption - the label is \
                     separate and freely editable."
                }
                li {
                    span { class: "font-medium", "Order is the layout. " }
                    "The sequence on the Fields tab is the sequence on the form. There is nothing else \
                     to position."
                }
                li {
                    span { class: "font-medium", "Structured values belong in a dropdown. " }
                    "A location or category hierarchy in one nested dropdown, not spread across twenty \
                     text fields nothing can validate."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "There is no limit in the interface and the limit underneath is not a column count - \
                 see the CF tab. What you meet first is rendering cost and legibility; 15 to 30 fields \
                 is the usual point to stop and reorganise."
            }
        }
    }
}

#[component]
fn StepFourRight() -> Element {
    rsx! {
        Panel {
            title: "Step 4 - grant the right".to_string(),
            subtitle: "Until you do, the type is invisible to everyone".to_string(),
            info: "The step most often missed, because nothing looks broken when \
                   it is skipped."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why it wastes time",
                    "The definition saved, it is active, the fields are there - and \
                     it is absent from the menu. The natural move is to go back and \
                     re-check the definition, which was never the problem. The \
                     super-admin profile is not exempt: the right did not exist when \
                     that profile was created, so nobody holds it.",
                ),
            ],
            info_links: vec![InfoLink::new("Profiles", "front/profile.php")],

            p { class: "mb-3 text-sm",
                "Creating the definition created a new right, "
                code { class: "text-xs", "asset_<lowercase system name>" }
                ", and no profile holds it yet."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "It is per type. " }
                    "Ten definitions mean ten rights to grant. There is no single \"custom assets\" right."
                }
                li {
                    span { class: "font-medium", "Two places, one meaning. " }
                    "The definition's own Profiles tab writes the same numbers into glpi_profilerights \
                     that the profile form edits. Either is fine; they are synchronised."
                }
                li {
                    span { class: "font-medium", "Helpdesk-interface profiles are forced to 0. " }
                    "Custom assets are central-interface only, by construction."
                }
            }
        }
    }
}

#[component]
fn StepFiveActivate() -> Element {
    rsx! {
        Panel {
            title: "Step 5 - translate, then activate".to_string(),
            subtitle: "Deactivating later deletes the rights rows".to_string(),
            info: "Two small steps. The second one can be undone, but not for \
                   free - deactivating costs the grants."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Deactivation is not a pause",
                    "syncProfilesRights in \
                     src/Glpi/CustomObject/AbstractDefinition.php deletes every \
                     profile right for the type when the definition is not active, \
                     rather than setting them to zero. Re-activating does not bring \
                     the grants back - they must be made again.",
                ),
            ],

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Translations tab. " }
                    "Set the displayed name and its plural, per language. The system name is not shown \
                     to anyone, so this is where the type gets the name people read."
                }
                li {
                    span { class: "font-medium", "Then activate. " }
                    "The type appears in the menu for every profile holding its right."
                }
                li {
                    span { class: "font-medium", "Deactivating is heavier than it sounds. " }
                    "It removes the rights rows outright. The data stays; the grants do not."
                }
            }
        }
    }
}

#[component]
fn StepSixProve() -> Element {
    rsx! {
        Panel {
            title: "Step 6 - prove it works".to_string(),
            subtitle: "Six checks, in the order that isolates a failure".to_string(),
            info: "Each check tells you which earlier step went wrong, which a \
                   single end-to-end look does not."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Reading a failure",
                    "Missing from the menu is step 4, not step 1 - the right. \
                     Present but missing a tab is step 2, a capacity. Present with a \
                     field absent is step 3, either the field order or a hidden \
                     option set for your profile. Checking in this order means the \
                     first thing that fails names the step to go back to.",
                ),
            ],

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li { "The type appears in the menu for your own profile." }
                li { "Create one record and save it." }
                li { "Find that record through search, by one of its custom fields." }
                li { "Every tab you enabled in step 2 is present on it." }
                li { "Set the list columns - globally if everyone should get them, which needs the \
                      GENERAL bit of search_config." }
                li { "Open it as a lesser profile and confirm readonly and hidden behave as set." }
            }
        }
    }
}
