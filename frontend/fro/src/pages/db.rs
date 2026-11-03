//! Where GLPI actually keeps the things this app reads - and where it does
//! not.
//!
//! This page is reference, not telemetry. It answers a question that came up
//! repeatedly while scoping the Rights page: "could we just read the database
//! for that?" For rights VALUES the answer is yes and the app already does it
//! through the API; for almost everything else the answer is no, because it is
//! PHP rather than data. Writing it down stops the question being re-opened
//! from memory.
//!
//! Every table and path named here was read in the GLPI 11.0.12 source or
//! observed against the live instance. The counts, though, are computed from
//! the live /api/rights response rather than quoted - a number nobody
//! re-checks rots into a lie, and this app has already shipped one wrong
//! denominator for exactly that reason.
use crate::api::{self, RightsReport};
use crate::components::{InfoLink, InfoSection, Panel, StatTile};
use dioxus::prelude::*;

#[component]
pub fn Db() -> Element {
    let report = use_resource(|| async move { api::fetch_rights().await });

    // The reference panels do not depend on the fetch, so they render either
    // way. Only the counts need it, and a failed count says so rather than
    // taking the whole page down - the same distinction between "absent" and
    // "failed" the rest of this app insists on.
    let counts = match &*report.read_unchecked() {
        Some(Ok(r)) => Some(derive(r)),
        _ => None,
    };

    rsx! {
        match counts {
            Some(c) => rsx! { Counts { counts: c } },
            None => rsx! {
                div { class: "mb-5 text-sm text-base-content/60",
                    "The figures on this page are read live and are not available right now; "
                    "the rest of the page is reference and is unaffected."
                }
            },
        }

        WhereItLives {}
        div { class: "mt-5", NotInTheDatabase {} }
        div { class: "mt-5", CustomObjects {} }
    }
}

/// What the live rights report tells us about the database behind it.
#[derive(Clone, PartialEq)]
struct DbCounts {
    profiles: usize,
    right_names: usize,

    /// Rows that actually exist in `glpi_profilerights`.
    ///
    /// Counted as the entries GLPI records a value for, which is exactly what
    /// a row is. Verified against the raw collection: 899 either way on this
    /// install. The product of the other two figures would be 909 and wrong -
    /// GLPI stores only the rows that exist, and five of the nine profiles
    /// have no row for `rule_change` or `rule_problem`.
    stored_rows: usize,
}

fn derive(r: &RightsReport) -> DbCounts {
    use std::collections::BTreeSet;
    let names: BTreeSet<&str> = r
        .profiles
        .iter()
        .flat_map(|p| p.entries.iter().map(|e| e.name.as_str()))
        .collect();
    DbCounts {
        profiles: r.profiles.len(),
        right_names: names.len(),
        stored_rows: r
            .profiles
            .iter()
            .flat_map(|p| p.entries.iter())
            .filter(|e| e.recorded)
            .count(),
    }
}

#[component]
fn Counts(counts: DbCounts) -> Element {
    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",

            StatTile {
                title: "Profiles".to_string(),
                value: counts.profiles.to_string(),
                desc: "in this GLPI".to_string(),
                body: "Every permission set GLPI holds. Each one is a row in its profiles \
                       table, and each gets its own set of rights."
                    .to_string(),
                links: vec![InfoLink::new("Profiles", "front/profile.php")],
            }

            StatTile {
                title: "Right names".to_string(),
                value: counts.right_names.to_string(),
                desc: "distinct permissions".to_string(),
                body: "The areas of GLPI that permission can be granted over - computers, \
                       tickets, agents, and so on. One name each."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why this is not a fixed number",
                        "It grows. A custom asset type defined in GLPI creates a new right \
                         named after it, and a plugin can add its own. That is why this figure \
                         is counted from what GLPI reports rather than written into this page.",
                    ),
                ],
            }

            StatTile {
                title: "Stored rows".to_string(),
                value: counts.stored_rows.to_string(),
                desc: "profile × right pairs".to_string(),
                body: "The actual size of the one table that holds any of this. Counted, not \
                       multiplied: GLPI stores only the pairs that exist."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why it is not profiles times rights",
                        "Because a few rights have no entry on some profiles at all, so the \
                         product would overstate it. An entry that does not exist denies just \
                         as one set to zero does, but it is a different statement - nobody \
                         chose it. The Rights page marks those as \u{201c}no entry\u{201d}.",
                    ),
                ],
            }
        }
    }
}

#[component]
fn WhereItLives() -> Element {
    rsx! {
        Panel {
            title: "Where GLPI keeps this".to_string(),
            subtitle: "What is data, and what is program".to_string(),
            info: "Which parts of GLPI's behaviour live in its database, and which are fixed \
                   in its code."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why it is worth knowing",
                    "It decides what can be changed, by whom, and what a database account \
                     would be good for. Anything in the database can be edited by an \
                     administrator and differs between installations. Anything in the code is \
                     the same everywhere and changes only when GLPI is upgraded.",
                ),
                InfoSection::new(
                    "This app does not read the database",
                    "Everything it shows comes through GLPI's API, as the account on the \
                     Status page. That is deliberate: the API applies the account's own \
                     permissions and its entity scope on every request, and reading the \
                     database directly would bypass both - as well as tying this app to \
                     GLPI's internal table layout across upgrades.",
                ),
            ],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "What" }
                            th { "Where it lives" }
                        }
                    }
                    tbody {
                        Row {
                            what: "The value of every right, per profile".to_string(),
                            where_: "Database".to_string(),
                            detail: "glpi_profilerights".to_string(),
                            in_db: true,
                        }
                        Row {
                            what: "Whether the API is switched on at all".to_string(),
                            where_: "Database".to_string(),
                            detail: "glpi_configs".to_string(),
                            in_db: true,
                        }
                        Row {
                            what: "Who may sign in to the current API".to_string(),
                            where_: "Database".to_string(),
                            detail: "glpi_oauthclients and its token tables".to_string(),
                            in_db: true,
                        }
                        Row {
                            what: "Who may sign in to the older API".to_string(),
                            where_: "Database".to_string(),
                            detail: "glpi_apiclients - token, address range, entity".to_string(),
                            in_db: true,
                        }
                        Row {
                            what: "Which addresses the API offers".to_string(),
                            where_: "Code".to_string(),
                            detail: "declared on each handler in PHP".to_string(),
                            in_db: false,
                        }
                        Row {
                            what: "Which fields each address returns".to_string(),
                            where_: "Code".to_string(),
                            detail: "written out in PHP beside the handler".to_string(),
                            in_db: false,
                        }
                        Row {
                            what: "Which right an address requires".to_string(),
                            where_: "Code".to_string(),
                            detail: "a fixed property on the item's class".to_string(),
                            in_db: false,
                        }
                        Row {
                            what: "Which tab and block a right appears under".to_string(),
                            where_: "Code".to_string(),
                            detail: "the profile form's own definition".to_string(),
                            in_db: false,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Row(what: String, where_: String, detail: String, in_db: bool) -> Element {
    rsx! {
        tr {
            td { class: "text-sm", "{what}" }
            td {
                div { class: "flex flex-wrap items-center gap-2",
                    span {
                        class: if in_db {
                            "badge badge-success badge-sm"
                        } else {
                            "badge badge-ghost badge-sm"
                        },
                        "{where_}"
                    }
                    span { class: "font-mono text-xs text-base-content/60", "{detail}" }
                }
            }
        }
    }
}

#[component]
fn NotInTheDatabase() -> Element {
    rsx! {
        Panel {
            title: "What a database account would not give you".to_string(),
            subtitle: "The question this page exists to settle".to_string(),
            info: "Four things that are commonly assumed to be in the database and are not."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Checked, not assumed",
                    "Each of these was looked for in both of GLPI's APIs, in the machine-\
                     readable description the API publishes of itself, and in GLPI's own \
                     command-line tool. None of the four is available by any of those routes, \
                     and none is in the database either - they are all fixed in the program.",
                ),
                InfoSection::new(
                    "What it means in practice",
                    "A database account is the wrong tool for discovering or changing what the \
                     API does. It would help only for reading data the API declines to expose - \
                     a different question, with its own cost, since it steps around the \
                     permission and entity checks every API request applies.",
                ),
            ],

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "The list of addresses the API serves. " }
                    "Each one is declared on its handler in GLPI's code. Nothing in the "
                    "database decides that an address exists."
                }
                li {
                    span { class: "font-medium", "The fields each address returns. " }
                    "Written out by hand beside the handler, which is why two addresses over "
                    "the same kind of record can return different fields."
                }
                li {
                    span { class: "font-medium", "Which right an address requires. " }
                    "A fixed property on the class behind it. Worth knowing because the name "
                    "is often not the one you would guess from the address, and because a "
                    "refusal never tells you which right was missing - that is what the "
                    "Rights page is for."
                }
                li {
                    span { class: "font-medium", "Which tab a right appears under. " }
                    "This one exists only inside the function that draws the profile form. "
                    "The Rights page groups by it anyway, using a copy taken from GLPI "
                    "itself - and says so when that copy and GLPI disagree, rather than "
                    "filing a right under the wrong heading."
                }
            }
        }
    }
}

#[component]
fn CustomObjects() -> Element {
    rsx! {
        Panel {
            title: "The one exception".to_string(),
            subtitle: "Custom asset and dropdown types".to_string(),
            info: "Custom types are the one case where defining something in GLPI's database \
                   creates both a new API address and a new right."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "How it works",
                    "A custom type is a row describing it. GLPI builds the class for that type \
                     when it starts, taking the right's name from the row - so a type whose \
                     internal name is kiosk gets a right called asset_kiosk - and keeps the \
                     rights table in step as the definition's profiles are edited. No code \
                     changes.",
                ),
                InfoSection::new(
                    "Why it is worth flagging",
                    "It means the API's address list is not fixed, and neither is the set of \
                     rights. Anything that treats either as a constant - including a page like \
                     this one - is wrong in principle. It is also why the figures above are \
                     counted from GLPI rather than written down here.",
                ),
                InfoSection::new(
                    "Not observed here",
                    "This installation defines no custom types, so the addresses exist as \
                     templates with nothing behind them. The behaviour described above was \
                     read in GLPI's source and confirmed only as far as the empty result; it \
                     has never been watched happen on this box.",
                ),
            ],

            p { class: "max-w-prose text-sm text-base-content/80",
                "Define a custom asset type called Kiosk in GLPI and three things appear "
                "together, with no change to this app or to GLPI's code: an API address for "
                "it, a right called "
                span { class: "font-mono", "asset_kiosk" }
                ", and that right's entries for every profile. It would show up on the "
                "Rights page by itself - and, until the grouping copy is refreshed, under "
                "\u{201c}Not on the profile form\u{201d}, which is the honest place for a "
                "right this app cannot yet place."
            }
        }
    }
}
