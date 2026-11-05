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
//! Every table and path named here was read in the GLPI 12.0.1 source or
//! observed against the live instance. The counts, though, are computed from
//! the live /api/rights response rather than quoted - a number nobody
//! re-checks rots into a lie, and this app has already shipped one wrong
//! denominator for exactly that reason.
use crate::api::{self, RightsReport};
use crate::components::{InfoButton, InfoLink, InfoSection, Panel, StatTile};
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

    let mut tab = use_signal(|| 0usize);

    rsx! {
        // Above the tabs, not inside either one: what this page covers is the
        // same question whichever tab is open. It is a named link rather than
        // the usual icon because the mistake it heads off - reading this as a
        // map of GLPI's database, or as the API's schemas - is made before a
        // reader has anything to hover.
        div { class: "mb-3",
            InfoButton {
                label: "What is this".to_string(),
                title: "What this page is".to_string(),
                body: "The part of GLPI's database this app is allowed to read - and the two \
                       much larger things this page is not."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "What is on it",
                        "Two tables of GLPI's database, column by column, with each column \
                         marked according to whether anything in this app actually reads it. \
                         The rest of the page is the reasoning around them: what GLPI keeps \
                         as data, what it keeps as program, and what a database account \
                         therefore cannot answer however wide you make it.",
                    ),
                    InfoSection::new(
                        "Why only two tables",
                        "Because two is all that is granted. This app may run SELECT on \
                         glpi_refusedequipments and on glpi_logs, and on nothing else. A \
                         GLPI 12.0.1 install creates 447 tables; the other 445 are reached \
                         through GLPI's API, which applies the entity and profile rules that \
                         direct database access would step around, or they are not reached at \
                         all. So this is the whole of the granted surface rather than a \
                         sample of a larger one, and it is complete when both tables are here \
                         with every column listed.",
                    ),
                    InfoSection::new(
                        "Not a map of GLPI",
                        "Nothing here describes GLPI's database as a whole, and it is not \
                         meant to grow into that. A table this app cannot read is a table it \
                         cannot be broken by, which is exactly why it is left out: the list is \
                         the app's own coupling to GLPI's schema, written down so it can be \
                         compared against a GLPI release, not documentation of GLPI.",
                    ),
                    InfoSection::new(
                        "\u{201c}Schema\u{201d} also means something else",
                        "GLPI's API publishes a description of itself, and that description \
                         declares around 330 schemas - Computer, User, Agent, and so on. \
                         Those are shapes of API responses, not tables, and none of them \
                         appear on this page. The app compares five of them against what it \
                         is really sent, which is how it catches a field GLPI's permissions \
                         quietly drop instead of refusing; that comparison is a diagnostic \
                         run on demand and has no page of its own. If you came here expecting \
                         330 of anything, that is the list you were thinking of.",
                    ),
                ],
            }
        }

        div { role: "tablist", class: "tabs tabs-border mb-5",
            button {
                role: "tab",
                class: if tab() == 0 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(0),
                "Overview"
            }
            button {
                role: "tab",
                class: if tab() == 1 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(1),
                "db-read"
            }
            button {
                role: "tab",
                class: if tab() == 2 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(2),
                "api-read"
            }
        }

        if tab() == 1 {
            DbRead {}
        } else if tab() == 2 {
            // Fetches inside the tab, never with the page: this one pulls
            // GLPI's whole OpenAPI document, about 4.6 MB, and the Overview
            // must not pay for a tab nobody opened.
            ApiRead {}
        } else {
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
}

/// Every database column this app reads, and every one it does not.
///
/// Worth a tab of its own because it is the whole cost of reading the database
/// rather than the API. The API names a field it no longer has; SQL does not -
/// a renamed column returns nothing and the page shows a blank. So the columns
/// depended on are written down where someone can compare them against a GLPI
/// release, rather than left to be discovered from an empty table.
///
/// Read from GLPI's own install SQL, not queried from the live database. The
/// grant is two tables of SELECT and gives no way to ask what the columns are.
#[component]
fn DbRead() -> Element {
    rsx! {
        Panel {
            title: "glpi_refusedequipments".to_string(),
            subtitle: "Granted, and read - 12 of its 14 columns".to_string(),
            info: "The submissions GLPI threw away. This is the table the refused view reads, \
                   and the only source for it anywhere."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why the columns are listed at all",
                    "Because this is the cost of reading a database instead of an API. An API \
                     that drops a field says which field; a query for a column that no longer \
                     exists just fails, and a column quietly renamed returns nothing. Every \
                     column here is treated as optional and renders blank rather than failing \
                     the page, so the failure mode is a gap on screen - which is only \
                     diagnosable if the expected columns are written down somewhere.",
                ),
                InfoSection::new(
                    "The name is plural, and that cost a GRANT",
                    "The table is glpi_refusedequipments. GLPI pluralises its table names, so \
                     searching the source for the singular matches every occurrence of the \
                     plural as a prefix and reads back as confirmation that the singular is \
                     right. That mistake reached a real GRANT statement, which failed with \
                     \u{201c}Table glpi.glpi_refusedequipment doesn't exist\u{201d}.",
                ),
                InfoSection::new(
                    "Where this comes from",
                    "GLPI's own install SQL for 12.0.1, not the live database. The grant is \
                     SELECT on two tables and carries no way to ask what columns they have, so \
                     this list is as current as the GLPI release it was read from - which is \
                     the honest limit of it.",
                ),
            ],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "Column" }
                            th { "Type" }
                            th { "Used" }
                        }
                    }
                    tbody {
                        Col { name: "id".to_string(), ty: "int unsigned".to_string(), used: Some("row identity".to_string()) }
                        Col { name: "name".to_string(), ty: "varchar(255)".to_string(), used: Some("what was refused".to_string()) }
                        Col { name: "itemtype".to_string(), ty: "varchar(255)".to_string(), used: Some("what kind of thing".to_string()) }
                        Col { name: "entities_id".to_string(), ty: "int unsigned".to_string(), used: Some("the entity restriction".to_string()) }
                        Col { name: "ip".to_string(), ty: "text".to_string(), used: Some("where it came from".to_string()) }
                        Col { name: "mac".to_string(), ty: "text".to_string(), used: Some("where it came from".to_string()) }
                        Col { name: "rules_id".to_string(), ty: "int unsigned".to_string(), used: Some("which rule refused it".to_string()) }
                        Col { name: "method".to_string(), ty: "varchar(255)".to_string(), used: Some("how it was submitted".to_string()) }
                        Col { name: "serial".to_string(), ty: "varchar(255)".to_string(), used: Some("identifying the machine".to_string()) }
                        Col { name: "uuid".to_string(), ty: "varchar(255)".to_string(), used: Some("identifying the machine".to_string()) }
                        Col { name: "agents_id".to_string(), ty: "int unsigned".to_string(), used: Some("which agent sent it".to_string()) }
                        Col { name: "autoupdatesystems_id".to_string(), ty: "int unsigned".to_string(), used: None }
                        Col { name: "date_creation".to_string(), ty: "timestamp".to_string(), used: Some("when, and the sort order".to_string()) }
                        Col { name: "date_mod".to_string(), ty: "timestamp".to_string(), used: None }
                    }
                }
            }
        }

        div { class: "mt-5",
            Panel {
                title: "glpi_logs".to_string(),
                subtitle: "Granted, and not read by anything yet".to_string(),
                info: "GLPI's own change history. The grant covers it; no query in this app \
                       touches it."
                    .to_string(),
                info_sections: vec![
                    InfoSection::new(
                        "Why grant a table nothing reads",
                        "It was granted alongside the refused table because it is the other \
                         thing neither API exposes, and because widening a grant later means \
                         another round of deciding. That is a defensible trade and also a real \
                         one: a grant wider than the code that uses it is access nobody is \
                         checking the need for. It is listed here rather than left implicit so \
                         the gap is visible - either something comes to use it, or it should \
                         come out of the grant.",
                    ),
                    InfoSection::new(
                        "What it would answer",
                        "What changed on an asset between two inventory runs. The API exposes \
                         no field-level history at all, so \u{201c}this machine's memory \
                         halved last Tuesday\u{201d} is unanswerable without this table.",
                    ),
                ],

                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                th { "Column" }
                                th { "Type" }
                                th { "Used" }
                            }
                        }
                        tbody {
                            Col { name: "id".to_string(), ty: "int unsigned".to_string(), used: None }
                            Col { name: "itemtype".to_string(), ty: "varchar(255)".to_string(), used: None }
                            Col { name: "items_id".to_string(), ty: "int unsigned".to_string(), used: None }
                            Col { name: "itemtype_link".to_string(), ty: "varchar(255)".to_string(), used: None }
                            Col { name: "linked_action".to_string(), ty: "int".to_string(), used: None }
                            Col { name: "user_name".to_string(), ty: "varchar(255)".to_string(), used: None }
                            Col { name: "date_mod".to_string(), ty: "timestamp".to_string(), used: None }
                            Col { name: "id_search_option".to_string(), ty: "int".to_string(), used: None }
                            Col { name: "old_value".to_string(), ty: "varchar(255)".to_string(), used: None }
                            Col { name: "new_value".to_string(), ty: "varchar(255)".to_string(), used: None }
                            Col { name: "old_id".to_string(), ty: "int unsigned".to_string(), used: None }
                            Col { name: "new_id".to_string(), ty: "int unsigned".to_string(), used: None }
                        }
                    }
                }
            }
        }

        div { class: "mt-5 max-w-prose text-sm text-base-content/70",
            "No other table is granted, so no other table has a schema this app can be broken              by. The rights values on the Rights page come from GLPI's API, not from "
            span { class: "font-mono", "glpi_profilerights" }
            " - reading them over SQL would have skipped the entity filtering every other page              passes through."
        }
    }
}

/// Why a withheld field is still a rights question, said on screen.
///
/// The table moved off the Rights page; the reason it was there has not gone
/// anywhere. Without this line a reader who finds a suppressed field here has
/// no idea the remedy is a profile right two pages away.
const RIGHTS_NOTE: &str = "A withheld field is a permissions fact, not a schema one: GLPI \
declares the field and this account may not read it. Which right it wants, and which profile \
would grant it, is on the Rights page - this tab only establishes that something promised did \
not arrive.";

/// What this app reads out of GLPI's API - the other half of `DbRead`.
///
/// Two things, because the tab answers two questions. What the API declares
/// is the catalogue: hundreds of schemas, which is the figure that makes the
/// audited slice legible as a slice. What actually arrives is the audit, over
/// the five paths this app really reads.
///
/// It moved here from the Rights page, and the comment it arrived with made
/// the opposite case - that it belonged beside rights because a withheld
/// field is a permissions phenomenon and not a schema one. That reading is
/// still true and is why `RIGHTS_NOTE` below says so on screen. What changed
/// is the question this page now asks: everything this app reads out of GLPI,
/// sorted by where it comes from. A field the API declares and withholds is
/// an answer to that, and the pairing with `db-read` is what makes the two
/// kinds of "schema" distinguishable at all.
///
/// GLPI's HL schemas carry `x-rights-conditions`, and a field the account may
/// not read is simply absent from an otherwise successful 200. Every field in
/// this app is `Option` by design, so that lands on screen as a blank cell -
/// which looks like missing data in GLPI rather than missing permission here.
/// 12.0.1 adds ten more such conditions than 11.0.12 has, so this stops being
/// theoretical at the upgrade. See backend/src/glpi/schema_check.rs.
#[component]
fn ApiRead() -> Element {
    let report = use_resource(|| async move { api::fetch_field_gaps().await });

    // Only when there is a document to list. An empty catalogue and an absent
    // one are different answers, and the panel below says nothing rather than
    // rendering an empty table that reads as "GLPI declares no schemas".
    let catalogue = match &*report.read_unchecked() {
        Some(Ok(r)) if r.doc_available && !r.catalogue.is_empty() => Some(r.catalogue.clone()),
        _ => None,
    };

    // Zeroed while the fetch is in flight or if it failed. The RSQL reference
    // renders either way and simply withholds the two counts - the grammar is
    // not contingent on this install answering.
    let reach = match &*report.read_unchecked() {
        Some(Ok(r)) => r.filter_reach,
        _ => api::FilterReach::default(),
    };

    let mut sub = use_signal(|| 0usize);

    rsx! {
        // A second row of tabs, inside the tab. Boxed rather than bordered so
        // it reads as nested under api-read instead of competing with the page
        // tabs above it.
        div { role: "tablist", class: "tabs tabs-box mb-4 w-fit",
            button {
                role: "tab",
                class: if sub() == 0 { "tab tab-active" } else { "tab" },
                onclick: move |_| sub.set(0),
                "fields"
            }
            button {
                role: "tab",
                class: if sub() == 1 { "tab tab-active" } else { "tab" },
                onclick: move |_| sub.set(1),
                "schema"
            }
            button {
                role: "tab",
                class: if sub() == 2 { "tab tab-active" } else { "tab" },
                onclick: move |_| sub.set(2),
                "rsql"
            }
        }

        if sub() == 1 {
            match catalogue {
                Some(rows) => rsx! { Catalogue { rows } },
                // The fetch states live on the fields tab, which is the
                // default; saying it twice would be two places to keep honest.
                None => rsx! {
                    div { class: "text-sm text-base-content/60",
                        "The schema list is read from GLPI's own API document and is not "
                        "available right now. The fields tab says why."
                    }
                },
            }
        } else if sub() == 2 {
            Rsql { reach }
        } else {
        Panel {
            title: "Fields the API withholds".to_string(),
            subtitle: "Declared against received".to_string(),
            info: "A call can succeed and still be short a field. GLPI drops fields the \
                   account may not read instead of refusing the request, so this compares \
                   what GLPI's own schema promises against what actually arrived."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is not visible anywhere else",
                    "A withheld field is absent, not null, and this app makes every field \
                     optional on purpose so that a drifting GLPI degrades to a blank cell \
                     instead of a failed page. That is the right trade and it is exactly what \
                     hides this: the cell looks like data GLPI does not have, rather than data \
                     this account may not see.",
                ),
                InfoSection::new(
                    "Where the comparison comes from",
                    "GLPI publishes its own OpenAPI document, and it is generated from the \
                     schema declarations rather than from the caller's rights - so it lists \
                     every field, while the response carries only the permitted ones. The \
                     difference between the two is the answer, and no second source is needed.",
                ),
                InfoSection::new(
                    "Write-only fields are not counted",
                    "A password field is declared and is never returned to anybody. Counting \
                     those would report a permanent problem on every call, so they are \
                     excluded and listed separately where present.",
                ),
                InfoSection::new(
                    "What it cannot tell you",
                    "Only top-level fields: a withheld key inside a nested object is invisible \
                     to it. And it reports that a promised field did not arrive, never why - a \
                     field held back by rights and a field GLPI stopped sending are both \
                     simply missing. Nothing arriving to compare against is reported as \
                     unknown rather than as clean.",
                ),
            ],

            match &*report.read_unchecked() {
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Comparing against GLPI's schema…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "The comparison could not be made." }
                            div { class: "text-sm",
                                "This says nothing about the fields themselves - only that the "
                                "check failed. Every other tab is unaffected."
                            }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },
                Some(Ok(r)) if !r.doc_available => rsx! {
                    // Not an error: an install that does not publish a schema
                    // cannot be checked this way, and saying so beats implying
                    // every endpoint came back complete.
                    div { class: "alert alert-info",
                        div {
                            div { class: "font-semibold", "GLPI served no usable schema document." }
                            div { class: "text-sm",
                                "Nothing could be compared, so nothing is claimed. This is not "
                                "the same as finding no withheld fields."
                            }
                        }
                    }
                },
                Some(Ok(r)) => {
                    let withheld: usize = r.reports.iter().map(|s| s.gaps.len()).sum();
                    rsx! {
                        div { class: "mb-4 flex flex-wrap items-center gap-2 text-sm",
                            if withheld == 0 {
                                span { class: "badge badge-success badge-sm", "complete" }
                                span { class: "text-base-content/70",
                                    "Every field {r.reports.len()} endpoint(s) promise arrived."
                                }
                            } else {
                                span { class: "badge badge-warning badge-sm", "{withheld} withheld" }
                                span { class: "text-base-content/70",
                                    "Named below, with the endpoint each belongs to."
                                }
                            }
                        }

                        div { class: "overflow-x-auto",
                            table { class: "table table-sm",
                                thead {
                                    tr {
                                        th { "Schema" }
                                        th { "State" }
                                        th { class: "text-right", "Readable" }
                                        th { class: "text-right", "Received" }
                                        th { "Fields" }
                                    }
                                }
                                tbody {
                                    for s in r.reports.iter() {
                                        FieldGapRow { report: s.clone() }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }

        div { class: "mt-2 max-w-prose text-xs text-base-content/60", "{RIGHTS_NOTE}" }
        }
    }
}

/// One audited endpoint.
#[component]
fn FieldGapRow(report: api::SchemaReport) -> Element {
    // Unknown is deliberately not styled as a success. "Could not tell" and
    // "nothing wrong" are different answers and must not share a colour.
    let (badge, label) = match report.state.as_str() {
        "ok" => ("badge-success", "ok"),
        "attention" => ("badge-warning", "withholding"),
        _ => ("badge-ghost", "unknown"),
    };

    rsx! {
        tr {
            td { class: "font-mono text-xs", "{report.schema}" }
            td { span { class: "badge {badge} badge-sm", "{label}" } }
            td { class: "text-right tabular-nums", "{report.readable}" }
            td { class: "text-right tabular-nums", "{report.received}" }
            td { class: "text-xs",
                if report.gaps.is_empty() {
                    span { class: "text-base-content/50", "\u{2014}" }
                } else {
                    div { class: "flex flex-wrap gap-1",
                        for g in report.gaps.iter() {
                            span {
                                class: if g.kind == "suppressed" {
                                    "badge badge-warning badge-outline badge-sm font-mono"
                                } else {
                                    "badge badge-info badge-outline badge-sm font-mono"
                                },
                                title: if g.kind == "suppressed" {
                                    "Declared and readable, but absent - this account is missing the right it wants"
                                } else {
                                    "Arrived without being declared - this app's view of the endpoint is behind GLPI's"
                                },
                                "{g.field}"
                            }
                        }
                    }
                }
                if !report.write_only.is_empty() {
                    div { class: "mt-1 text-base-content/50",
                        "write-only, excluded: {report.write_only.join(\", \")}"
                    }
                }
            }
        }
    }
}

/// Every schema GLPI's document declares, counted but never compared.
///
/// The denominator, and the only reason it is on screen: five schemas are
/// audited above, and "five endpoints came back complete" reads as "the API is
/// complete" unless the size of the list they were drawn from is visible.
#[component]
fn Catalogue(rows: Vec<api::SchemaSummary>) -> Element {
    let total = rows.len();
    let audited = rows.iter().filter(|r| r.audited).count();

    rsx! {
        Panel {
            title: "Every schema the API declares".to_string(),
            subtitle: format!("{total} declared, {audited} of them audited"),
            info: "GLPI's API publishes a description of itself, and this is the whole of what \
                   it describes. Every name here is the shape of an API response - not a \
                   database table, and not one of the two tables under db-read."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why the whole list is here",
                    "Because it is the denominator. The audit above covers five schemas, and \
                     five clean endpoints read as a clean API unless you can see how many \
                     there are. The count is taken from the document on every load rather \
                     than written down here, because a number nobody re-checks rots into a \
                     lie.",
                ),
                InfoSection::new(
                    "Why there is no Received column",
                    "Because it would cost one request per schema - hundreds against one \
                     install - where the audited five cost five. A figure that cannot be had \
                     cheaply is left out rather than estimated from whatever the first page \
                     of a collection happened to carry.",
                ),
                InfoSection::new(
                    "Declared, readable, write-only",
                    "Declared is every property the schema names. Write-only ones are never \
                     returned to anybody - a password field is the clear case - so readable \
                     is what a caller could actually expect back, and it is the number the \
                     audit compares against. They are split rather than summed so a schema \
                     that is mostly write-only cannot look like one that withholds.",
                ),
                InfoSection::new(
                    "Not the same thing as db-read",
                    "db-read lists two tables of GLPI's database, by column, because this app \
                     holds SELECT on them. This lists response shapes from GLPI's API. \
                     Nothing here is a table, nothing there is an endpoint, and both are \
                     called a schema by somebody - which is why they are two tabs on one page \
                     rather than one list.",
                ),
            ],

            // Capped and scrolled rather than laid out full: at this length the
            // list is something you look a name up in, and a page that scrolls
            // for hundreds of rows buries everything under it.
            div { class: "max-h-96 overflow-y-auto overflow-x-auto",
                table { class: "table table-sm table-pin-rows",
                    thead {
                        tr {
                            th { "Schema" }
                            th { class: "text-right", "Declared" }
                            th { class: "text-right", "Readable" }
                            th { class: "text-right", "Write-only" }
                            th { "Audited" }
                        }
                    }
                    tbody {
                        for r in rows.iter() {
                            CatalogueRow { key: "{r.schema}", row: r.clone() }
                        }
                    }
                }
            }
        }
    }
}

/// One declared schema.
#[component]
fn CatalogueRow(row: api::SchemaSummary) -> Element {
    rsx! {
        tr {
            td { class: "font-mono text-xs whitespace-nowrap", "{row.schema}" }
            td { class: "text-right text-xs tabular-nums", "{row.declared}" }
            td { class: "text-right text-xs tabular-nums", "{row.readable}" }
            td {
                class: if row.write_only > 0 { "text-right text-xs tabular-nums" } else { "text-right text-xs tabular-nums text-base-content/40" },
                "{row.write_only}"
            }
            td {
                if row.audited {
                    span { class: "badge badge-success badge-sm", "audited" }
                } else {
                    // Not a failing: five paths are what this app reads. An
                    // unaudited schema is one nothing here asks GLPI for.
                    span { class: "text-xs text-base-content/40", "not read" }
                }
            }
        }
    }
}

/// Every operator GLPI's RSQL parser defines - not only the ones this app
/// reaches for.
///
/// Token, GLPI's own description, whether a value follows, and whether it
/// needs 12.0.1. Read out of `src/Glpi/Api/HL/RSQL/Parser.php` in both
/// releases and diffed: 16 operators in 11.0.12, 18 in 12.0.1, the truthy
/// pair being the whole of the difference. Order is the parser's own, because
/// that is what a reader diffing this against a GLPI release will see.
const OPERATORS: &[(&str, &str, bool, bool)] = &[
    ("==", "equivalent to", true, false),
    ("!=", "not equivalent to", true, false),
    ("=in=", "in", true, false),
    ("=out=", "not in", true, false),
    ("=lt=", "less than", true, false),
    ("=le=", "less than or equal to", true, false),
    ("=gt=", "greater than", true, false),
    ("=ge=", "greater than or equal to", true, false),
    ("=like=", "like", true, false),
    ("=ilike=", "case insensitive like", true, false),
    ("=isnull=", "is null", false, false),
    ("=notnull=", "is not null", false, false),
    ("=empty=", "is empty", false, false),
    ("=notempty=", "is not empty", false, false),
    ("=notlike=", "not like", true, false),
    ("=notilike=", "case insensitive not like", true, false),
    ("=istruthy=", "is truthy", false, true),
    ("=isnottruthy=", "is not truthy", false, true),
];

/// The grammar around the operators: what joins terms, what groups them, and
/// what has to be escaped.
///
/// From `RSQL/Lexer.php` - its own constants, not inferred from examples.
const GRAMMAR: &[(&str, &str, &str)] = &[
    (";", "and", "name==\"web01\";is_deleted=istruthy="),
    (",", "or", "name==\"web01\",name==\"web02\""),
    (
        "( )",
        "group, and the value list =in= takes",
        "states_id=in=(1,2,3)",
    ),
    ("\\", "escape, in an unquoted value", "name==foo\\;bar"),
    (
        "\" or \u{27}",
        "quote a value, either kind",
        "name==\u{27}web 01\u{27}",
    ),
    (
        "a-z 0-9 _ .",
        "what a property name may contain",
        "user.name==\"pde\"",
    ),
];

/// The whole RSQL surface GLPI offers, against the sliver of it in use.
///
/// Reference rather than telemetry, like db-read - except the two reach
/// figures, which are read live off the same API document the other tabs use.
/// A filter is the difference between asking GLPI a question and fetching a
/// collection to answer it here, so what the language can express is worth
/// writing down in full rather than discovering one operator at a time.
#[component]
fn Rsql(reach: api::FilterReach) -> Element {
    rsx! {
        Panel {
            title: "Operators".to_string(),
            subtitle: format!("All {} GLPI defines, whether or not this app uses them", OPERATORS.len()),
            info: "RSQL is the filter language GLPI's API takes in its filter parameter. These \
                   are every operator its parser defines - the full surface, not the part this \
                   app currently builds."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this list comes from",
                    "GLPI's own parser, read in the source of both releases and diffed - 16 \
                     operators in 11.0.12 and 18 in 12.0.1. It is not read from the API \
                     document, because the document does not contain it: the filter parameter \
                     is described there in full as \u{201c}RSQL query string\u{201d} and names \
                     no operator at all.",
                ),
                InfoSection::new(
                    "Two of them need 12.0.1",
                    "=istruthy= and =isnottruthy= are the only operators 12.0.1 adds. An \
                     install still on 11.0.12 answers them with an RSQL error rather than \
                     ignoring them, so the failure is loud either way - which is the useful \
                     kind.",
                ),
                InfoSection::new(
                    "Case, and what empty means",
                    "=like= is matched against a BINARY cast and is therefore case sensitive, \
                     where =ilike= is not; that is the only difference between them. =empty= \
                     covers both the empty string and NULL, where =isnull= covers only NULL - \
                     so a column holding \u{27}\u{27} answers =empty= and not =isnull=.",
                ),
                InfoSection::new(
                    "All eighteen are reachable from this app",
                    "The filter builder in this app covers every operator listed here, \
                     including the two that need 12.0.1, and flags those so a caller can \
                     avoid them on an older install. What it does not do is guess: values are \
                     always quoted and escaped, because an unquoted value containing a \
                     semicolon or a comma silently changes the shape of the query rather than \
                     failing.",
                ),
            ],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "Operator" }
                            th { "Means" }
                            th { "Value" }
                            th { "Since" }
                        }
                    }
                    tbody {
                        for (token, means, takes_value, only_12) in OPERATORS.iter() {
                            tr { key: "{token}",
                                td { class: "font-mono text-xs whitespace-nowrap", "{token}" }
                                td { class: "text-xs", "{means}" }
                                td { class: "text-xs",
                                    if *takes_value {
                                        span { class: "text-base-content/70", "a value follows" }
                                    } else {
                                        // Not a missing value - the operator is
                                        // complete on its own, and appending one
                                        // makes GLPI reject the term.
                                        span { class: "text-base-content/50", "none" }
                                    }
                                }
                                td {
                                    if *only_12 {
                                        span { class: "badge badge-warning badge-sm", "12.0.1" }
                                    } else {
                                        span { class: "text-xs text-base-content/50", "11.0.12" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        div { class: "mt-5",
            Panel {
                title: "Grammar".to_string(),
                subtitle: "What joins terms, and what must be escaped".to_string(),
                info: "An operator on its own is not a query. These are the characters that \
                       combine terms, group them, and quote a value - every one of them from \
                       GLPI's lexer rather than from an example that happened to work."
                    .to_string(),
                info_sections: vec![
                    InfoSection::new(
                        "Why escaping is the whole hazard",
                        "An unquoted value runs until the next unescaped comma, semicolon or \
                         closing parenthesis. So a computer named foo;bar, interpolated \
                         straight into a filter, does not fail - it becomes two terms joined \
                         by AND, and the answer comes back wrong and plausible. Quoting and \
                         escaping is why this app builds filters rather than formatting them.",
                    ),
                    InfoSection::new(
                        "Dots reach into joined records",
                        "A property name may contain dots, and GLPI resolves the path across \
                         the joins its schema declares - so user.name filters computers by \
                         the name of the user they are assigned to, in one request. That is \
                         the part worth knowing: the filter is not restricted to the columns \
                         of one table.",
                    ),
                ],

                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                th { "Token" }
                                th { "Means" }
                                th { "Example" }
                            }
                        }
                        tbody {
                            for (token, means, example) in GRAMMAR.iter() {
                                tr { key: "{means}",
                                    td { class: "font-mono text-xs whitespace-nowrap", "{token}" }
                                    td { class: "text-xs", "{means}" }
                                    td { class: "font-mono text-xs break-all text-base-content/70", "{example}" }
                                }
                            }
                        }
                    }
                }
            }
        }

        div { class: "mt-5",
            Panel {
                title: "Reach, and what it refuses".to_string(),
                subtitle: if reach.paths > 0 {
                    format!("{} of {} operations take a filter", reach.operations, reach.paths)
                } else {
                    "Counts unavailable - the reference below is unaffected".to_string()
                },
                info: "Which calls can be filtered at all, and the three ways GLPI rejects a \
                       filter it will not run."
                    .to_string(),
                info_sections: vec![
                    InfoSection::new(
                        "The counts are live",
                        "Read off GLPI's own API document on this install each time the tab \
                         opens, not written down here - the same rule the rest of this page \
                         follows. Zero means the document could not be read, which the fields \
                         tab explains; it does not mean filtering is unavailable.",
                    ),
                    InfoSection::new(
                        "Mapped properties are refused",
                        "A property GLPI computes from another one - declared x-mapped-from in \
                         its schema - cannot be filtered, and the request fails with \
                         \u{201c}Mapped properties cannot be used in RSQL\u{201d}. There are 43 \
                         such declarations in 12.0.1 against 31 in 11.0.12, so the surface \
                         grows at the upgrade. None of them are discoverable from the \
                         document: it strips the extension, so the only way to know a \
                         property is unfilterable is to filter on it and read the error.",
                    ),
                    InfoSection::new(
                        "The three refusals",
                        "Unknown property, unknown operator, and mapped property. All three \
                         name the offending term rather than failing the whole call silently, \
                         which is the one respect in which this is friendlier than the \
                         database: a filter that cannot run says so.",
                    ),
                ],

                div { class: "max-w-prose space-y-2 text-sm text-base-content/70",
                    p {
                        "Filtering is a GET-only affair, and the document describes the "
                        span { class: "font-mono text-xs", "filter" }
                        " parameter in its entirety as "
                        span { class: "italic", "\u{201c}RSQL query string\u{201d}" }
                        " - no operator, no grammar, no mention of what cannot be filtered. "
                        "Everything above this line is why that sentence is not enough to use it."
                    }
                    p {
                        "What it buys: a question GLPI answers in one request instead of a "
                        "collection walked here and mostly discarded. The software list on "
                        "this install is 1330 rows in three requests to answer things a "
                        "filter would answer in one."
                    }
                }
            }
        }
    }
}

/// One column, and whether this app depends on it.
#[component]
fn Col(name: String, ty: String, used: Option<String>) -> Element {
    // Read before the match consumes it, so the row class and the cell cannot
    // disagree about whether this column is used.
    let is_read = used.is_some();
    rsx! {
        tr { class: if is_read { "" } else { "opacity-50" },
            td { class: "font-mono text-xs whitespace-nowrap", "{name}" }
            td { class: "font-mono text-xs text-base-content/60", "{ty}" }
            td {
                match used {
                    Some(why) => rsx! {
                        div { class: "flex flex-wrap items-center gap-2",
                            span { class: "badge badge-success badge-sm", "read" }
                            span { class: "text-xs text-base-content/70", "{why}" }
                        }
                    },
                    // Present in GLPI, not depended on - so a change to it
                    // cannot break this app, which is worth knowing too.
                    None => rsx! {
                        span { class: "text-xs text-base-content/50", "not read" }
                    },
                }
            }
        }
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
                "Rights page by itself, under "
                "\u{201c}Not on the profile form\u{201d} until the grouping copy is "
                "refreshed. Two kinds never move from there: an inactive custom asset type, "
                "and any custom dropdown type. GLPI builds the profile form from its active "
                "asset definitions only and never adds dropdown definitions to it, while "
                "creating the rights for both - so those are granted and enforced but appear "
                "on no form, and no refresh will place them."
            }
        }
    }
}
