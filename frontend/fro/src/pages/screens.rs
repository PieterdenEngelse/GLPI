//! GLPI's own screens, and what can be put on them.
//!
//! Reference, not telemetry - the same kind of page as DB. It answers
//! questions about configuring GLPI that get asked once per deployment and
//! then re-answered from memory, wrongly.
//!
//! Every claim on all four tabs was read in the GLPI 12.0.1 source or in
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
use crate::app::Route;
use crate::components::{InfoButton, InfoLink, InfoSection, Panel};
use dioxus::prelude::*;
use dioxus_router::{use_navigator, Link};

#[component]
pub fn Screens(tab: String) -> Element {
    // Four tabs. The machinery is db.rs's - one button each and one branch -
    // which is the whole reason the panels are behind components rather than
    // rendered directly here.
    let nav = use_navigator();

    // Names, not indices: a reordered strip must not silently change what an
    // existing link points at.
    const TABS: [&str; 4] = ["cf", "def", "rights", "instruct"];

    // The strip's position comes from the URL rather than a signal, so a
    // pasted link, a reload and the Back button all land on the tab the
    // reader was looking at. Unknown or absent means the first tab, never an
    // error - the same fallback /logs uses.
    let at = TABS.iter().position(|t| *t == tab).unwrap_or(0);

    rsx! {
        div { class: "mb-3",
            InfoButton {
                label: "What is this".to_string(),
                title: "What this page is".to_string(),
                body: "What GLPI's own configuration screens can be made to hold - read out \
                       of GLPI's source, not from its documentation. These are questions \
                       about configuring GLPI rather than about this app's own data, so \
                       nothing on this page is live. They are written down because they are \
                       asked once per deployment and then answered from memory the second \
                       time, and because the commonly circulated answer to the first of them \
                       is wrong. Each claim names the file, line or schema it came from in \
                       GLPI 12.0.1: a later GLPI release can make any of them false, and \
                       naming the source is what makes that cheap to find out instead of \
                       something this page quietly lies about."
                    .to_string(),
            }
        }

        div { role: "tablist", class: "tabs tabs-border mb-5",
            button {
                role: "tab",
                class: if at == 0 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| {
                    nav.push(Route::Screens { tab: TABS[0].to_string() });
                },
                "CF"
            }
            button {
                role: "tab",
                class: if at == 1 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| {
                    nav.push(Route::Screens { tab: TABS[1].to_string() });
                },
                "Def"
            }
            button {
                role: "tab",
                class: if at == 2 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| {
                    nav.push(Route::Screens { tab: TABS[2].to_string() });
                },
                "Rights"
            }
            button {
                role: "tab",
                class: if at == 3 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| {
                    nav.push(Route::Screens { tab: TABS[3].to_string() });
                },
                "Instruct"
            }
        }

        if at == 3 {
            Instruct {}
        } else if at == 2 {
            Rights {}
        } else if at == 1 {
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
        // Two columns from lg up - see Instruct for why row-major and
        // items-start. Three panels leave the last cell empty, which is
        // the ordinary result of an odd count and still beats a column of
        // full-width panels with the right half of the page unused.
        div { class: "grid gap-4 items-start lg:grid-cols-2",
            TheMechanism {}
            ThePracticalLimit {}
            WhereToPutItInstead {}
        }
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
                     decodes them. Storage: install/mysql/glpi-12.0.0-empty.sql line 10358, \
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
            info: "Why the absence of a hard cap is not an invitation. These are the failure \
                   modes the mechanism above makes likely, and the figure below is a \
                   convention rather than a measurement: nothing in this app has benchmarked \
                   a GLPI install with hundreds of custom fields, and the panel does not \
                   pretend otherwise."
                .to_string(),

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
                   that does not exist. A separate asset definition is worth making for the \
                   reasons below - a clean screen and a type that means one thing. It is \
                   often recommended for a different reason: that it yields a fresh table \
                   with its own column budget. It does not. Every custom asset lives in \
                   glpi_assets_assets regardless, so the gain is organisational, not a larger \
                   allowance."
                .to_string(),

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
/// Five panels, so the grid has a trailing empty cell as `Cf` and `Rights` do.
/// `mt-5` wrappers for the same mechanical reason: the stylesheet is
/// generated from the class names found in these files, and `space-y-5` is
/// not among the ones already emitted.
#[component]
fn Def() -> Element {
    rsx! {
        // Two columns from lg up - see Instruct for why row-major and
        // items-start. Five panels, so the last cell is empty again - the
        // ordinary result of an odd count, and still better than a column
        // of full-width panels with the right half of the page unused.
        // Deleting follows disabling deliberately: it is the same purge,
        // for every capacity at once.
        div { class: "grid gap-4 items-start lg:grid-cols-2",
            WhatADefinitionCreates {}
            DisablingACapacityDeletes {}
            DeletingADefinition {}
            HidingVersusRemoving {}
            WhichViewYouLandOn {}
        }
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
                    span { class: "font-medium", "And nothing in the database protects it. " }
                    "A JSON column takes no foreign keys, so the profile ids inside "
                    code { class: "text-xs", "profiles" }
                    " are just numbers - deleting a profile cannot cascade into the \
                     definitions that name it. GLPI does that work in PHP instead: \
                     Profile.php lines 369 and 372 call removeProfileFromField on each \
                     definition, and cleanRights at AssetDefinition.php line 479 strips a \
                     withdrawn right out of the stored bitmasks. No schema change is the \
                     benefit; hand-written cascades are the bill."
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
            info: "The two ways to simplify a screen, and why only one of them is safe. Read \
                   in src/Glpi/Asset/AssetDefinition.php: field order at lines 340 to 350, \
                   where the array index IS the order, so there is no separate layout model; \
                   the fallback when the JSON is empty or unparseable in \
                   getDefaultFieldsDisplay at line 805; and the placeable core fields in \
                   getAllFields at line 710."
                .to_string(),

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

/// Deleting a definition: the same purge as disabling, for all of it at once.
///
/// Sits after `DisablingACapacityDeletes` because it is that panel's warning
/// multiplied, and the page carried the smaller one only.
#[component]
fn DeletingADefinition() -> Element {
    rsx! {
        Panel {
            title: "Deleting a definition takes more than the type".to_string(),
            subtitle: "No bin, three classes, every capacity at once".to_string(),
            info: "Read in src/Glpi/Asset/AssetDefinition.php and \
                   src/Glpi/CustomObject/AbstractDefinition.php. cleanDBonPurge at \
                   AssetDefinition.php 436 loops onCapacityDisabled over the decoded \
                   capacities, then calls purgeConcreteClassFromDb for the asset, model \
                   and type classes; that helper is at AbstractDefinition.php 626 and \
                   uses deleteByCriteria with force true and history false, then deletes \
                   the DisplayPreference rows for the itemtype. The definitions table has \
                   no is_deleted column, which is what makes delete and purge the same \
                   action here. Not verified: what the UI asks before running it."
                .to_string(),

            p { class: "mb-3 text-sm",
                "The panel above is about one capacity. This is the same destruction with \
                 nothing left behind, and it is the larger of the two."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "There is no bin. " }
                    code { class: "text-xs", "glpi_assets_assetdefinitions" }
                    " has no "
                    code { class: "text-xs", "is_deleted" }
                    " column, so unlike the assets it defines, a definition cannot be \
                     trashed and restored. Deleting runs the purge immediately."
                }
                li {
                    span { class: "font-medium", "Every capacity fires first. " }
                    "cleanDBonPurge walks the enabled capacities and disables each one, so \
                     whatever the panel above would destroy for a single capacity is \
                     destroyed for all of them in one action."
                }
                li {
                    span { class: "font-medium", "Three classes, not one. " }
                    "The asset class is purged, and so are its model and type classes - the \
                     two dropdowns the definition created. Their contents go with the type \
                     rather than surviving as orphaned dropdown values."
                }
                li {
                    span { class: "font-medium", "Forced, and no history. " }
                    "The purge passes force and turns history off, and it also deletes \
                     every "
                    code { class: "text-xs", "DisplayPreference" }
                    " row for the itemtype, so each user's column choices go too. Nothing \
                     about it is recorded in the log that would show a field change."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Deactivating is the reversible one. Clearing is_active drops the right's \
                 rows through ProfileRight::deleteProfileRights and leaves every asset \
                 where it is - the profiles JSON stays on the definition row, so \
                 reactivating writes the same rights back."
            }
        }
    }
}

/// Which saved view a list opens on, and why it is not DisplayPreference.
///
/// The fourth Def panel. Columns and filters are separate mechanisms with
/// separate rights, and the page covered only the first until now.
#[component]
fn WhichViewYouLandOn() -> Element {
    rsx! {
        Panel {
            title: "Which view a list opens on".to_string(),
            subtitle: "Columns are one mechanism, filters are another".to_string(),
            info: "Read in src/SavedSearch.php and install/mysql/glpi-12.0.0-empty.sql. \
                   The right is \
                   bookmark_public at line 56; markDefault at 594 writes \
                   glpi_savedsearches_users at 606, unmarkDefault at 637 and \
                   unmarkDefaults at 672 remove it. glpi_savedsearches holds the search \
                   itself - query is one text column, read back with parse_str at line \
                   581, so the criteria are a serialised URL rather than a modelled set. \
                   Not verified: whether a stored query can also pin the columns, which \
                   would overlap DisplayPreference. SavedSearch.php names no forcedisplay."
                .to_string(),

            p { class: "mb-3 text-sm",
                "A screen that should open already filtered is a saved search, not a \
                 display preference. The two are often confused because both change what \
                 you see on arrival."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Columns versus filters. " }
                    code { class: "text-xs", "DisplayPreference" }
                    " decides which columns a list shows and nothing else - see the \
                     Rights tab for its two bits. A saved search carries the criteria, \
                     and marking one default decides which of them you land on."
                }
                li {
                    span { class: "font-medium", "The default is per user. " }
                    code { class: "text-xs", "glpi_savedsearches_users" }
                    " is unique on "
                    code { class: "text-xs", "(users_id, itemtype)" }
                    ", so there is exactly one default per person per type. A search can \
                     be shared while the decision to land on it stays personal, which is \
                     why one user's list opens filtered and a colleague's does not."
                }
                li {
                    span { class: "font-medium", "Sharing is a right. " }
                    "A search defaults to private - "
                    code { class: "text-xs", "is_private" }
                    " is 1 - and the public ones are governed by "
                    code { class: "text-xs", "bookmark_public" }
                    ", the class rightname. Without it a shared landing view cannot be \
                     created, only a personal one."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "None of this is part of a custom asset definition. It applies to any \
                 itemtype with a list, built-in or custom, which is why an \"empty\" or \
                 \"wrong\" screen is sometimes neither the definition nor the rights but \
                 a default view someone marked."
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
        // Two columns from lg up - see Instruct for why row-major and
        // items-start. Three panels leave the last cell empty, which is
        // the ordinary result of an odd count and still beats a column of
        // full-width panels with the right half of the page unused.
        div { class: "grid gap-4 items-start lg:grid-cols-2",
            InvisibleUntilGranted {}
            TwoBitsOfSearchConfig {}
            VaryByProfile {}
        }
    }
}

#[component]
fn InvisibleUntilGranted() -> Element {
    rsx! {
        Panel {
            title: "A new asset type is invisible until its right is granted".to_string(),
            subtitle: "One right per definition, named after it".to_string(),
            info: "Why a finished definition does not appear in the menu. Nothing looks \
                   broken: the definition saved, it is active, its fields are there - and it \
                   is absent from the menu, so the natural move is to go back and re-check \
                   the definition, which was never the problem. The super-admin profile is \
                   not exempt either - the right did not exist when that profile was created, \
                   so it is not held. Read in getCustomObjectRightname at \
                   src/Glpi/Asset/AssetDefinition.php line 98, which returns \
                   sprintf('asset_%s', strtolower(system_name)), with storage in \
                   glpi_profilerights in install/mysql/glpi-12.0.0-empty.sql - one row per \
                     profile \
                   and right name, unique on (profiles_id, name)."
                .to_string(),
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
                    InfoButton {
                        label: "How".to_string(),
                        title: "Granting the right".to_string(),
                        body: "Two routes write the same rows and stay in step, so use whichever you have open."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The steps",
                                "Administration, then Profiles, then the profile you want. Find the right by name - asset_ plus the lowercased system name - set what it should hold, and save. Or open the definition and use its own Profiles tab, which reaches the same rows.",
                            ),
                            InfoSection::new(
                                "Both directions are synchronised",
                                "The definition's Profiles tab writes glpi_profilerights through syncProfilesRights; the profile form writes back into the definition through setProfileRights at Profile.php line 270. Neither is the master copy - editing either updates the other.",
                            ),
                            InfoSection::new(
                                "Where this was read",
                                "src/Glpi/CustomObject/AbstractDefinition.php for syncProfilesRights and setProfileRights, src/Profile.php line 270 for the write-back, and AssetDefinition.php line 98 for how the right's name is built.",
                            ),
                        ],
                        links: vec![InfoLink::new("Profiles", "front/profile.php")],
                    }
                }
                li {
                    span { class: "font-medium", "A helpdesk profile is always zero. " }
                    "syncProfilesRights reads each profile's interface and writes 0 for every \
                     one on the helpdesk interface, whatever the definition's Profiles tab \
                     holds - AbstractDefinition.php line 717, where the ternary chooses \
                     between 0 and the stored value. The grant saves without complaint and \
                     the right is still absent, so a type that must reach helpdesk users is \
                     not made reachable this way."
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
            info: "The option that makes a second definition unnecessary. Read in \
                   src/Glpi/Asset/CustomFieldType/AbstractType.php lines 75 to 83: every \
                   field gets full_width, required, readonly and hidden, and the last two are \
                   ProfileRestrictOption rather than BooleanOption - see \
                   src/Glpi/Asset/CustomFieldOption/ProfileRestrictOption.php, which carries \
                   an \"All\" label at line 52."
                .to_string(),

            p { class: "mb-3 text-sm",
                "Two of the four options every custom field has are "
                span { class: "font-medium", "sets of profiles" }
                ", which is easy to miss when they read like checkboxes."
                InfoButton {
                    label: "How".to_string(),
                    title: "Setting readonly or hidden".to_string(),
                    body: "These live on the FIELD, not on the profile - which is why people look for them in the wrong place and conclude GLPI cannot do it."
                        .to_string(),
                    sections: vec![
                        InfoSection::new(
                            "The steps",
                            "Open the definition, go to its Fields tab, open the field, and set its readonly-for-these-profiles or hidden-for-these-profiles option. Each takes a SET of profiles rather than a yes or no, so one field can be editable for technicians, visible but locked for one group, and absent for another.",
                        ),
                        InfoSection::new(
                            "Hiding keeps the data",
                            "A hidden field is not rendered, but its stored values are untouched and return the moment it is shown again. That is the difference between this and disabling a capacity, which deletes what it held.",
                        ),
                        InfoSection::new(
                            "Where this was read",
                            "src/Glpi/Asset/CustomFieldType/AbstractType.php lines 75 to 83 for the four options every field has, and CustomFieldOption/ProfileRestrictOption.php for the profile-set type, whose All label is at line 52.",
                        ),
                    ],
                }
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

/// The Instruct tab's sub-tab index, shared through context.
#[derive(Clone, Copy)]
struct InstructTab(Signal<usize>);

/// A "step N" reference that takes the reader to the steps themselves.
///
/// Rendered as a button rather than a Link: General is a sub-tab, not a
/// route, so there is no URL to point at. It reads as a link and behaves as
/// one within the page.
#[component]
fn StepRef(n: usize) -> Element {
    let InstructTab(mut sub) = use_context::<InstructTab>();
    rsx! {
        button {
            class: "link link-primary",
            onclick: move |_| sub.set(0),
            "step {n}"
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
    // A second strip inside a tab. Deliberately styled smaller than the page
    // tabs above it - no text-base - so the hierarchy reads at a glance
    // rather than looking like two peer rows of tabs.
    // Shared so a panel on the Template tab can send the reader to the steps.
    // A newtype rather than a bare Signal<usize>: context is keyed by type,
    // and a second usize signal anywhere above would silently bind instead.
    let InstructTab(mut sub) = use_context_provider(|| InstructTab(Signal::new(0usize)));

    rsx! {
        div { role: "tablist", class: "tabs tabs-border mb-3",
            button {
                role: "tab",
                class: if sub() == 0 { "tab tab-active" } else { "tab" },
                onclick: move |_| sub.set(0),
                "General"
            }
            button {
                role: "tab",
                class: if sub() == 1 { "tab tab-active" } else { "tab" },
                onclick: move |_| sub.set(1),
                "Template"
            }
        }

        if sub() == 1 {
            InstructTemplate {}
        } else {
            InstructGeneral {}
        }
    }
}

/// The six-step procedure for building the type itself.
#[component]
fn InstructGeneral() -> Element {
    rsx! {
        StepZero {}

        // Two columns filled COLUMN-MAJOR: steps 1-3 down the left, 4-6 down
        // the right, read like a printed page rather than left-to-right by
        // rows.
        //
        // Two explicit column containers rather than columns-2 or
        // grid-flow-col. Multi-column can break a panel across the column
        // boundary and would need break-inside-avoid; grid-flow-col needs an
        // explicit row count kept in step with the number of steps. Two divs
        // need neither, and below lg the outer grid collapses to one column
        // so the reading order stays 1-2-3-4-5-6.
        //
        // items-start on both levels: without it the columns stretch to equal
        // height and a short panel renders as a mostly empty box.
        div { class: "mt-5 grid gap-4 items-start lg:grid-cols-2",
            div { class: "grid gap-4 items-start",
                StepOneName {}
                StepTwoCapacities {}
                StepThreeFields {}
            }
            div { class: "grid gap-4 items-start",
                StepFourRight {}
                StepFiveActivate {}
                StepSixProve {}
            }
        }
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

            // Two columns from lg up: this panel stays full width while the
            // steps below it are half, so one bullet per row left the right
            // half of it empty. gap-2 replaces space-y-2 because the gap
            // belongs to the grid once the children are grid items.
            //
            // The grid goes on the ul and the li stay list-items, so the
            // disc markers survive - blockification keeps `list-item`
            // rather than flattening it to `block`.
            ul { class: "ml-5 grid list-disc gap-2 text-sm lg:grid-cols-2",
                li {
                    span { class: "font-medium", "A kind of thing to record - vehicles, IoT devices. " }
                    "A custom asset definition, with its own menu entry, list and form. Steps 1 to 6 \
                     below build one."
                }
                li {
                    span { class: "font-medium", "A list of values to pick from. " }
                    "A custom dropdown definition - Setup then Dropdowns - managed like Locations or \
                     Manufacturers. "
                    InfoButton {
                        label: "How".to_string(),
                        title: "Making a custom dropdown".to_string(),
                        body: "Far less work than an asset definition: a dropdown has no \
                               capacities and no fields, because the values ARE the content. \
                               Its definition form carries two tabs, Profiles and Translations, \
                               and nothing else."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The steps",
                                "Setup, then Dropdowns, then add a definition. Give it a system \
                                 name. Grant its right in Profiles - nobody holds it yet, \
                                 exactly as with an asset type. Set the displayed name and \
                                 plural on Translations, then activate it. The values \
                                 themselves are added afterwards from the dropdown's own \
                                 screen, like any core dropdown.",
                            ),
                            InfoSection::new(
                                "What the name becomes",
                                "The same pattern rule as an asset definition - start with a \
                                 letter, end alphanumeric - but a different suffix and right. \
                                 Colour gives you the class ColourDropdown and the right \
                                 dropdown_colour, not asset_colour. The reserved names differ \
                                 too: every standard GLPI dropdown class is taken, computed at \
                                 runtime rather than listed, so the set grows with GLPI.",
                            ),
                            InfoSection::new(
                                "Where this was read",
                                "src/Glpi/Dropdown/DropdownDefinition.php - the right at line \
                                 93, the class suffix at line 68, the two tabs at line 108. \
                                 The reserved set is getReservedSystemNamesPattern in \
                                 DropdownDefinitionManager.php line 83, which walks \
                                 Dropdown::getStandardDropdownItemTypes. Capacity and custom \
                                 field appear zero times in the definition.",
                            ),
                        ],
                        links: vec![InfoLink::new(
                            "Custom dropdowns",
                            "front/dropdown/dropdowndefinition.php",
                        )],
                    }
                }
                li {
                    span { class: "font-medium", "Questions put to a person. " }
                    "Forms, under Service catalogue. They sequence and condition questions and keep answers \
                     relationally. "
                    InfoButton {
                        label: "How".to_string(),
                        title: "Making a form".to_string(),
                        body: "Built from questions rather than fields, and it ends by creating something. Three decisions: what it asks, who may open it, and what it produces."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The steps",
                                "Service catalogue, then Forms, then add one. Build it from sections and questions - 19 question types ship. Set access control: an allow list of who may open it, or direct access for a shareable link. Add a destination, which is what the answers become: a Ticket, a Change or a Problem. Then activate it.",
                            ),
                            InfoSection::new(
                                "Why not an asset",
                                "Questions can be sequenced and made conditional on earlier answers, and the answers are kept relationally. An asset's flat field list can do neither, which is the whole reason to reach for this instead.",
                            ),
                            InfoSection::new(
                                "Where this was read",
                                "src/Glpi/Form/ - the right is 'form' at Form.php line 108; Destination/ holds FormDestinationTicket, FormDestinationChange and FormDestinationProblem; AccessControl/ControlType/ holds AllowList and DirectAccess; QuestionType/ has 19 concrete types.",
                            ),
                        ],
                        links: vec![InfoLink::new("Forms", "front/form/form.php")],
                    }
                }
                li {
                    span { class: "font-medium", "Figures to look at. " }
                    "A dashboard - a grid of widgets, read-only by nature, edited in place from the dashboard \
                     itself. "
                    InfoButton {
                        label: "How".to_string(),
                        title: "Making a dashboard".to_string(),
                        body: "A grid of widgets fed by providers. Unlike the others it is not created from Setup - you edit a dashboard from the dashboard itself."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The steps",
                                "Open a dashboard, put it into edit mode, add widgets and arrange them on the grid, then save. A dashboard carries its own access rows rather than a profile right, so who may see it is set per dashboard.",
                            ),
                            InfoSection::new(
                                "Not verified in the UI",
                                "The classes and the access model were read in the source; the exact control that enters edit mode was not. Expect that button's wording to differ from this description - the structure is right, the label may not be.",
                            ),
                            InfoSection::new(
                                "Where this was read",
                                "src/Glpi/Dashboard/ - Grid.php for the layout, Widget.php for what can go on it, Provider.php for where the figures come from, and Right.php keyed on dashboards_dashboards_id for the per-dashboard access.",
                            ),
                        ],
                        links: vec![InfoLink::new("Asset dashboard", "front/dashboard_assets.php")],
                    }
                }
                li {
                    span { class: "font-medium", "Different columns on a list that already exists. " }
                    "Not a new screen at all. That is a display preference, and whether you can set it \
                     for everyone is a rights question - see the Rights tab. "
                    InfoButton {
                        label: "How".to_string(),
                        title: "Changing a list's columns".to_string(),
                        body: "Nothing is created. It is a setting on a list that already exists, and the only real question is whether you may set it for everyone or only for yourself."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The steps",
                                "Open the list, use its column selector, choose the columns. Saving them for yourself needs the PERSONAL bit of search_config; saving them as everyone's default needs GENERAL. If the global option is not offered to you, the profile lacks that bit - there is no second screen to go looking for.",
                            ),
                            InfoSection::new(
                                "Personal is seeded from global",
                                "The first time a user needs a personal set, GLPI builds it from the global one and then leaves it alone. So someone who has never chosen sees the global columns, and someone who chose once stops tracking later global changes.",
                            ),
                            InfoSection::new(
                                "Where this was read",
                                "src/DisplayPreference.php - the right search_config at line 53, PERSONAL 1024 and GENERAL 2048 at lines 55 and 56, the global-versus-personal split around line 511, and the seeding at the comment on line 281 with its guard at line 295.",
                            ),
                        ],
                        links: vec![InfoLink::new("Display preferences", "front/displaypreference.form.php")],
                    }
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
                     and so are computer and COMPUTER. "
                    InfoButton {
                        label: "Can I add my own?".to_string(),
                        title: "Reserving your own names".to_string(),
                        body: "No - the fifteen are fixed in GLPI's source. But they are the \
                               only names taken, so everything outside them is yours, which is \
                               more freedom than the list suggests."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "Not configurable",
                                "The list is a PHP array literal in \
                                 AssetDefinitionManager::getReservedSystemNamesPattern, joined \
                                 into one case-insensitive regex. It is not config, not a \
                                 database table and not exposed anywhere in the interface, so \
                                 adding to it means patching GLPI - which the next upgrade \
                                 overwrites.",
                            ),
                            InfoSection::new(
                                "The only three checks",
                                "A name is accepted if it matches the pattern, does not match \
                                 those fifteen case-insensitively, and is unique among existing \
                                 definitions compared lowercased. That is the whole validation - \
                                 no namespace, no prefix requirement, no allow-list.",
                            ),
                            InfoSection::new(
                                "Dropdowns differ",
                                "Custom dropdowns are stricter, and not by a fixed list: \
                                 DropdownDefinitionManager computes its reserved set at runtime \
                                 by walking every standard dropdown class GLPI ships, so it is \
                                 large and grows with each release. Asset names stay at fifteen \
                                 until GLPI's own source changes.",
                            ),
                            InfoSection::new(
                                "Upgrades are safe",
                                "A later GLPI reserving a name you already used cannot strand \
                                 your definition. prepareInputForUpdate refuses any change to \
                                 system_name outright - the system name cannot be changed - so \
                                 the name is never resubmitted and never re-validated against a \
                                 longer list.",
                            ),
                        ],
                    }
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
                    ". "
                    InfoButton {
                        label: "In detail".to_string(),
                        title: "What the name becomes".to_string(),
                        body: "One row in one table, and from it GLPI conjures seven PHP classes, \
                               a permission and a place to put the data - none of which exists \
                               until the name is saved."
                            .to_string(),
                        sections: vec![
                            InfoSection::new(
                                "The class is eval'd",
                                "VehicleAsset is not a file. AssetDefinitionManager registers an \
                                 spl_autoload hook, and the first time anything mentions \
                                 Glpi\\CustomAsset\\VehicleAsset it is built by eval() on the spot: \
                                 a final class extending Glpi\\Asset\\Asset with two static \
                                 properties, the system name and the rightname. Nothing is ever \
                                 written to disk - file_put_contents appears zero times in that \
                                 manager.",
                            ),
                            InfoSection::new(
                                "Seven, not one",
                                "The autoloader matches seven patterns per definition: the asset \
                                 itself, its Model and Type, the two rule-dictionary classes for \
                                 those, and a collection class for each dictionary. So Vehicle \
                                 also yields VehicleAssetModel, VehicleAssetType, \
                                 RuleDictionaryVehicleAssetModel and three more - all in the \
                                 Glpi\\CustomAsset namespace, all conjured the same way.",
                            ),
                            InfoSection::new(
                                "Why statics repeat",
                                "The generated class restates $definition_system_name and \
                                 $rightname rather than inheriting them, and the source says why: \
                                 a static property declared only on the parent is SHARED by every \
                                 concrete class, so the last one loaded would overwrite the rest \
                                 and every custom asset would answer to one definition.",
                            ),
                            InfoSection::new(
                                "No table, ever",
                                "Every custom asset of every type lives in glpi_assets_assets - 27 \
                                 columns, told apart by assets_assetdefinitions_id. Field \
                                 definitions are rows in glpi_assets_customfielddefinitions and \
                                 their values go into one custom_fields json column. So a second \
                                 definition buys organisation, never storage, and the ~950-column \
                                 folklore has no table to apply to.",
                            ),
                        ],
                    }
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
                 do not. "
                InfoButton {
                    label: "Can I write my own?".to_string(),
                    title: "Adding a capacity".to_string(),
                    body: "Yes, but only from PHP - there is no screen for it. And the answer \
                           is narrower for custom FIELD types, which can be added the \
                           same way but have no plugin-facing door."
                        .to_string(),
                    sections: vec![
                        InfoSection::new(
                            "The door",
                            "AssetDefinitionManager::registerCapacity takes any \
                             CapacityInterface, at line 339. Nothing in GLPI's own source \
                             calls it, which is about as clear a signal as you get that it is \
                             provided for plugins rather than used by core. Registering at \
                             plugin boot is the durable route.",
                        ),
                        InfoSection::new(
                            "Just a directory",
                            "They are not a list anywhere. The manager's constructor runs a \
                             DirectoryIterator over src/Glpi/Asset/Capacity and takes every \
                             non-abstract class implementing CapacityInterface. GLPI's own \
                             comment calls this a stopgap for missing dependency-injection \
                             auto-discovery. Dropping a file in there does work - and the \
                             next upgrade deletes it.",
                        ),
                        InfoSection::new(
                            "Three methods",
                            "CapacityInterface declares fourteen, but AbstractCapacity \
                             implements eleven. Extend it and you must write only getLabel, \
                             getIcon and getCapacityUsageDescription; the lifecycle hooks and \
                             search options have defaults you override only if the capacity \
                             really does something. For scale, the smallest shipped one is 77 \
                             lines.",
                        ),
                        InfoSection::new(
                            "Types differ",
                            "The same manager scans CustomFieldType the same way, so a \
                             tenth type is also just a class in a directory, with the \
                             same upgrade problem. What is missing is the door: there \
                             is no registerCustomFieldType to match registerCapacity, \
                             so a plugin cannot add one cleanly. Narrower, not closed. \
                             Note the third filter too - a class must also return true \
                             from isAllowedForCustomFields, which is how RawType exists \
                             and is still never offered.",
                        ),
                        InfoSection::new(
                            "Writing a type",
                            "Two methods. TypeInterface declares eleven instance methods \
                             plus two statics, and AbstractType implements all but getName \
                             - the label in the type picker - and getFormInput, which \
                             renders the input. URLType is the smallest shipped example: 60 \
                             lines declaring exactly those two, and it extends StringType \
                             rather than AbstractType, so building on an existing type is \
                             the cheaper start. Leave isAllowedForCustomFields at its \
                             inherited true, or the class is scanned and skipped.",
                        ),
                    ],
                }
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
            info: "Eight selectable types, four options on every one of them, and \
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
                "Eight types are offered: Boolean, Date, DateTime, Dropdown, Number, String, Text \
                 and URL. "
                InfoButton {
                    label: "Where from, and can I add one?".to_string(),
                    title: "Where the types come from".to_string(),
                    body: "One class per type in one directory, discovered at runtime. The \
                           database never lists them - it only records which one each field \
                           chose."
                        .to_string(),
                    sections: vec![
                        InfoSection::new(
                    "Where they live",
                    "src/Glpi/Asset/CustomFieldType, one class each. \
                     AssetDefinitionManager's constructor runs a DirectoryIterator \
                     over it and keeps a class on three conditions: it implements \
                     TypeInterface, it is not abstract, and its static \
                     isAllowedForCustomFields returns true. The menu is therefore \
                     built from the filesystem every boot.",
                        ),
                        InfoSection::new(
                    "Why eight, not nine",
                    "Nine concrete classes sit in that directory, but RawType \
                     returns false from isAllowedForCustomFields and is skipped. \
                     Counting the directory gives nine; counting what you can pick \
                     gives eight. The third condition is invisible from a file \
                     listing, which is exactly how the wrong number gets quoted.",
                        ),
                        InfoSection::new(
                    "Not in the database",
                    "There is no glpi_assets_customfieldtypes table - zero \
                     occurrences in the schema. What is stored is the other \
                     direction: customfielddefinitions.type is a varchar(255) \
                     holding the chosen class's fully-qualified name, and \
                     getFieldType validates that string against the runtime list, \
                     throwing Invalid field type if it is not in it.",
                        ),
                        InfoSection::new(
                    "Adding a tenth",
                    "A class in that directory, not a row: two methods, getName \
                     for the type picker and getFormInput to render the input - \
                     AbstractType provides the other eleven. URLType is 60 lines \
                     and extends StringType. But there is no \
                     registerCustomFieldType to match registerCapacity, so a \
                     plugin cannot add one cleanly, and a file dropped into GLPI's \
                     own tree is removed by the next upgrade.",
                        ),
                    ],
                }
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
            info: "The step most often missed, because nothing looks broken when it is \
                   skipped. The definition saved, it is active, the fields are there - and it \
                   is absent from the menu. The natural move is to go back and re-check the \
                   definition, which was never the problem. The super-admin profile is not \
                   exempt: the right did not exist when that profile was created, so nobody \
                   holds it."
                .to_string(),
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
            info: "Two small steps. The second one can be undone, but not for free - \
                   deactivating costs the grants. syncProfilesRights in \
                   src/Glpi/CustomObject/AbstractDefinition.php deletes every profile right \
                   for the type when the definition is not active, rather than setting them \
                   to zero, so re-activating does not bring the grants back - they must be \
                   made again."
                .to_string(),

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
            info: "Each check tells you which earlier step went wrong, which a single \
                   end-to-end look does not. Missing from the menu is step 4, not step 1 - \
                   the right. Present but missing a tab is step 2, a capacity. Present with a \
                   field absent is step 3, either the field order or a hidden option set for \
                   your profile. Checking in this order means the first thing that fails \
                   names the step to go back to."
                .to_string(),

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

/// Templates for the type once it exists.
///
/// Separate from the six steps because a template is not part of building the
/// screen - it is something you make with the screen afterwards, and it needs
/// nothing enabled.
///
/// This is NOT a custom-asset feature. The is_template flag is on twenty
/// tables, core types included, and custom assets simply inherit the same
/// CommonDBTM behaviour. The tab says so because the opposite is easy to
/// assume from where it sits.
#[component]
fn InstructTemplate() -> Element {
    rsx! {
        TemplateWhat {}
    }
}

#[component]
fn TemplateWhat() -> Element {
    rsx! {
        Panel {
            title: "\"Template\" means four different things".to_string(),
            info: "Which four, where each lives, and why telling them apart \
                   matters."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "glpi_assets_assets carries is_template tinyint and \
                     template_name varchar(255), plus two indexes - is_template \
                     on its own and active_assets on (is_deleted, is_template). \
                     CommonDBTM::maybeTemplate at line 3427 simply reports whether \
                     the is_template field exists, so the feature switches itself \
                     on for any type stored in that table.",
                ),
                InfoSection::new(
                    "Four kinds",
                    "One word, four shapes - and the base class does not tell them \
                     apart, since nine of the ten template classes descend from \
                     CommonDropdown. What the table CARRIES does. A FLAGGED ROW, \
                     this one: the template is a record of the very type it \
                     templates - a computer template IS a computer - on the \
                     twenty tables carrying is_template. A DROPDOWN OF VALUES: \
                     ProjectTaskTemplate extends CommonDropdown and its table \
                     mirrors the task's own fields - projects_id, \
                     plan_start_date, planned_duration, percent_done. A FORM \
                     DESCRIPTOR: glpi_tickettemplates holds only id, name, entity \
                     and allowed_statuses, with the actual behaviour in four \
                     satellite tables - predefined, mandatory, hidden and readonly \
                     fields, each a (template, field number, value) row.",
                ),
                InfoSection::new(
                    "Defaults",
                    "And the form-descriptor kind really does define a screen, \
                     which is why the title above is scoped. tickettemplates_id \
                     sits on glpi_entities, glpi_profiles and glpi_itilcategories \
                     - with separate _demand and _incident columns - so GLPI \
                     resolves a DEFAULT ticket template from the context you are \
                     in, and it decides which fields are mandatory, hidden, \
                     readonly or pre-filled. Nothing on this tab does that.",
                ),
                InfoSection::new(
                    "Tasks have both",
                    "glpi_projecttasks carries is_template AND there is a \
                     glpi_projecttasktemplates table, so that one type has both \
                     mechanisms at once: a flagged task you copy, and a dropdown \
                     of pre-filled values you pick. Worth knowing before \
                     concluding that a project task template is not working - \
                     there are two places to look.",
                ),
                InfoSection::new(
                    "No capacity",
                    "There is no template capacity in src/Glpi/Asset/Capacity, and \
                     Asset.php mentions templates once. None of step 2 applies: \
                     this is CommonDBTM behaviour inherited by every custom asset \
                     type, not a subsystem you bolt on and can purge.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "GLPI calls four unrelated mechanisms a template. They share nothing but the \
                 name, so advice about one is wrong about the other three. Each has its own page."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "A flagged row. " }
                    "One of your records with "
                    code { class: "text-xs", "is_template = 1" }
                    ", used to pre-fill the next one. It has no say in what the form looks like. "
                    Link { class: "link link-primary text-sm", to: Route::TemplateRow {}, "The flagged row →" }
                }
                li {
                    span { class: "font-medium", "A carrier of values. " }
                    "A row whose columns mirror the item's own fields - you pick one and it fills \
                     them in. Six of these: ProjectTaskTemplate and PlanningExternalEventTemplate, \
                     plus the four ITIL child templates for tasks, solutions, followups and \
                     validations. "
                    Link { class: "link link-primary text-sm", to: Route::TemplateValues {}, "Carriers of values →" }
                }
                li {
                    span { class: "font-medium", "A form definition - and it does define the screen. " }
                    "ITILTemplate and its three subclasses, for tickets, changes and problems. The \
                     table itself holds almost nothing; the behaviour is in four satellite tables \
                     for predefined, mandatory, hidden and readonly fields. GLPI resolves a DEFAULT \
                     one from the entity, the profile and the ITIL category you are in. Nothing on \
                     this tab behaves like that. "
                    Link { class: "link link-primary text-sm", to: Route::TemplateForm {}, "Form definitions →" }
                }
                li {
                    span { class: "font-medium", "A message. " }
                    "NotificationTemplate is a fourth thing again - name, itemtype and css, with \
                     the subject and body per language in \
                     glpi_notificationtemplatetranslations. No form and no instance anywhere near \
                     it. "
                    Link { class: "link link-primary text-sm", to: Route::TemplateMessage {}, "Message templates →" }
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Why bother with the flagged-row kind: "
                StepRef { n: 3 }
                " warns that 15 to 30 fields is where a screen stops being readable. A template is \
                 how the field count stays useful and a new record still takes three edits rather \
                 than twenty."
            }
        }
    }
}
