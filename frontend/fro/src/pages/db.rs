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
use crate::app::Route;
use crate::components::nav;
use crate::components::{FetchError, InfoButton, InfoLink, Panel, StatTile};
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn Db(tab: String, sub: String) -> Element {
    let report = use_resource(|| async move { api::fetch_rights().await });

    // The reference panels do not depend on the fetch, so they render either
    // way. Only the counts need it, and a failed count says so rather than
    // taking the whole page down - the same distinction between "absent" and
    // "failed" the rest of this app insists on.
    let counts = match &*report.read_unchecked() {
        Some(Ok(r)) => Some(derive(r)),
        _ => None,
    };

    // Names, not indices, so a reordered strip cannot change what an existing
    // link points at. Unknown or absent is Overview, never an error.
    const TABS: [&str; 6] = ["overview", "db-read", "api-read", "maria", "vars", "rules"];
    let at = TABS.iter().position(|t| *t == tab).unwrap_or(0);

    rsx! {
        // Overview only. It describes the page as a whole, which is what a
        // reader who has just arrived at the top of it needs; further in, it
        // answers a question that reader has already moved past, and the
        // sidebar keeps the structure in view regardless. A named link rather
        // than the usual icon because the mistake it heads off - reading this
        // as a map of GLPI's database, or as the API's schemas - is made
        // before a reader has anything to hover.
        //
        // Every tab strip this page used to carry is in the sidebar now,
        // including the sub-tabs under Maria and api-read, which is why this
        // row holds one link and nothing else.
        if at == 0 {
            div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
                InfoButton {
                    label: "What is this".to_string(),
                    title: "What this page is".to_string(),
                    // One body, no tabs. Merged from four sections holding 1719
                    // characters between them - about fifteen lines at this
                    // modal's width, which fits unscrolled, and a tab strip that
                    // hides text the reader could already see is a click that
                    // buys nothing. Single paragraph because InfoButton renders
                    // body inside a <p>, so blank lines would collapse to spaces.
                    body: "Two tables of GLPI's database, column by column, with each column \
                           marked according to whether anything in this app actually reads it; \
                           the rest of the page is the reasoning around them - what GLPI keeps \
                           as data, what it keeps as program, and what a database account \
                           therefore cannot answer however wide you make it. Only two, but not \
                           because the rest is forbidden: the account was granted SELECT on \
                           glpi_refusedequipments and glpi_logs and nothing else, and since \
                           2026-11-13 it holds every privilege on the server, so all 447 tables \
                           of a GLPI 12.0.1 install are reachable. These two are still the only \
                           ones this app reads, for an unchanged reason - everything else comes \
                           through GLPI's API, which applies the entity and profile rules that \
                           direct database access steps around - so what was once the shape of a \
                           grant is now the shape of a decision. Nothing here describes GLPI's \
                           database as a whole, and it is not meant to grow into that: a table \
                           this app cannot read is a table it cannot be broken by, which is why \
                           it is left out, and the list is the app's own coupling to GLPI's \
                           schema, written down so it can be compared against a GLPI release \
                           rather than documentation of GLPI. One word to disentangle before you \
                           go looking: GLPI's API publishes a description of itself declaring \
                           around 330 schemas - Computer, User, Agent and so on - but those are \
                           shapes of API responses, not tables, and none of them appear here. \
                           The app compares five of them against what it is really sent, which \
                           is how it catches a field GLPI's permissions quietly drop instead of \
                           refusing, and that comparison is a diagnostic run on demand with no \
                           page of its own. If you came here expecting 330 of anything, that is \
                           the list you were thinking of."
                        .to_string(),
                }
            }
        }

        if at == 5 {
            Rules {}
        } else if at == 4 {
            Vars { sub: sub.clone() }
        } else if at == 3 {
            Maria { sub: sub.clone() }
        } else if at == 1 {
            DbRead {}
        } else if at == 2 {
            // Fetches inside the tab, never with the page: this one pulls
            // GLPI's whole OpenAPI document, about 4.6 MB, and the Overview
            // must not pay for a tab nobody opened.
            ApiRead { sub: sub.clone() }
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
/// Read from GLPI's own install SQL, not queried from the live database. That
/// was forced when the account reached two tables; since the grant was widened
/// it is a choice, and the better one - a release's schema file is the same
/// for every install and can be diffed across versions.
#[component]
fn DbRead() -> Element {
    rsx! {
        Panel {
            title: "glpi_refusedequipments".to_string(),
            subtitle: "Granted, and read - 12 of its 14 columns".to_string(),
            info: "The submissions GLPI threw away. This is the table the refused view reads, \
                  and the only source for it anywhere. Why the columns are listed at all: \
                  Because this is the cost of reading a database instead of an API. An API \
                  that drops a field says which field; a query for a column that no longer \
                  exists just fails, and a column quietly renamed returns nothing. Every \
                  column here is treated as optional and renders blank rather than failing \
                  the page, so the failure mode is a gap on screen - which is only \
                  diagnosable if the expected columns are written down somewhere. The name \
                  is plural, and that cost a GRANT: The table is glpi_refusedequipments. \
                  GLPI pluralises its table names, so searching the source for the singular \
                  matches every occurrence of the plural as a prefix and reads back as \
                  confirmation that the singular is right. That mistake reached a real GRANT \
                  statement, which failed with \u{201c}Table glpi.glpi_refusedequipment \
                  doesn't exist\u{201d}. Where this comes from: GLPI's own install SQL for \
                  12.0.1, not the live database - which the account could now be asked, \
                  since the grant was widened, and is deliberately not. A release's file is \
                  identical on every install and diffable between versions, where a live \
                  query describes this one server only. The limit is the honest one: this \
                  list is as current as the GLPI release it was read from."
                .to_string(),
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
                subtitle: "Granted, and not read yet - held for what is coming".to_string(),
                info: "GLPI's own change history. The grant covers it; no query in this app \
                      touches it. Why grant a table nothing reads: It was granted alongside \
                      the refused table because widening a grant later means another round \
                      of deciding. It is listed here rather than left implicit, so that an \
                      unused grant is visible rather than forgotten. Not a loose end to tidy \
                      away: The obvious audit - nothing reads it, so it should come out - is \
                      the wrong test while an app is still being built, and this page said \
                      otherwise until recently. The question is whether the capability is on \
                      its way, and this one is: a serial or a MAC that changed quietly \
                      between two inventories is the natural fourth failure beside stopped \
                      reporting, never landed and no agent. What the grant buys is the whole \
                      estate in one query, which is the part no API offers. What it would \
                      answer: What changed on an asset between two inventory runs - \
                      \u{201c}this machine's memory halved last Tuesday\u{201d}. Not \
                      strictly unanswerable without it, and this page claimed otherwise \
                      until recently: the older API returns an asset's history when asked \
                      for it by id, one machine per request. That is the same per-machine \
                      cost that sent the inventory pages to collections instead, and it is \
                      the whole difference - a question asked of the fleet, rather than of \
                      one machine at a time."
                    .to_string(),
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

/// What the server is and what GLPI asks of it, as distinct from what GLPI
/// puts in it.
///
/// A container for four sub-tabs: general for the server itself, storage for
/// the choices GLPI's installer makes, db's for what else is on the server,
/// heidi for the client that connects to it.
/// There was a fourth, engine, carrying a tab per storage engine and a
/// control that could move a GLPI table between them. It is gone: GLPI wraps
/// each inventory import in one transaction (Inventory.php:328), so a GLPI
/// table on a non-transactional engine loses a rollback it never reports
/// losing - which leaves the only engines the server would accept as the
/// ones GLPI cannot afford. A page whose every answer was "do not do this"
/// was not worth the room.
#[component]
fn Maria(sub: String) -> Element {
    // general first, so it is what a bare ?tab=maria lands on - orientation,
    // then detail. The strip itself is drawn by the page, in SubTabs, so it
    // can share the page tabs' line.
    let at = nav::MARIA_SUBS
        .iter()
        .position(|(n, _)| *n == sub)
        .unwrap_or(0);

    rsx! {

        if at == 3 {
            Heidi {}
        } else if at == 2 {
            Databases {}
        } else if at == 1 {
            MariaStorage {}
        } else {
            MariaGeneral {}
        }
    }
}

/// What GLPI does on its own - the rules tab.
///
/// On this page rather than a page of its own because it is the same question
/// the rest of the page asks: which of GLPI's behaviours are data you can read
/// and which are program. A rule is the awkward middle - it is a row, so it
/// looks readable, but what it DOES is a criteria tree and an action list
/// interpreted by PHP, and nothing about its effect follows from counting it.
#[component]
fn Rules() -> Element {
    let report = use_resource(|| async move { api::fetch_rules().await });

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-sm text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Asking GLPI for its rule collections\u{2026}"
                }
            },
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) if !r.available => rsx! {
                div { class: "alert alert-info text-sm",
                    "GLPI served no rule collections. That is not an install without rules - \
                     it is this profile being unable to read them, or a GLPI without the \
                     Rule controller, and the difference matters enough not to render it as \
                     a zero."
                }
            },
            Some(Ok(r)) => rsx! { RuleTiles { report: r.clone() } },
        }
    }
}

/// One tile's rules, as a table under the stats row rather than inside the
/// tile.
///
/// The tiles are a stats strip and a list of 71 inside one would break it, so
/// the list unfolds below both tiles and names which one it belongs to. What
/// it shows is deliberately thin: GLPI's own name, its rank, whether it is
/// switched on, and how much criteria and action there is to interpret. It
/// does not show the criteria themselves - those are a tree PHP evaluates,
/// and rendering them as rows would read as this app knowing what they do.
#[component]
fn RuleList(
    title: String,
    subtitle: String,
    note: String,
    /// The business list merges three collections and has to say which;
    /// the import list is one collection and the column would be noise.
    show_collection: bool,
    /// Only the import collection can refuse, so the column exists only
    /// where it means something rather than being blank everywhere else.
    show_denies: bool,
    rules: Vec<(String, api::RuleSummary)>,
) -> Element {
    rsx! {
        Panel {
            title,
            subtitle,
            info: note,
            if rules.is_empty() {
                div { class: "text-sm text-base-content/60",
                    "GLPI served the count but not the rules. That is a backend older than \
                     this page, not a collection that emptied."
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-xs",
                        thead {
                            tr {
                                th { "rank" }
                                if show_collection {
                                    th { "collection" }
                                }
                                th { "name" }
                                th { "active" }
                                if show_denies {
                                    th { "refuses" }
                                }
                                th { "criteria" }
                                th { "actions" }
                            }
                        }
                        tbody {
                            for (collection , r) in rules.iter() {
                                tr {
                                    key: "{collection}-{r.ranking}-{r.name}",
                                    // A switched-off rule fires on nothing, so
                                    // it is dimmed rather than listed as an
                                    // equal of the ones that run.
                                    class: if r.is_active { "" } else { "opacity-50" },
                                    td { class: "text-base-content/50", "{r.ranking}" }
                                    if show_collection {
                                        td { class: "whitespace-nowrap text-xs", "{collection}" }
                                    }
                                    td { "{r.name}" }
                                    td { {if r.is_active { "\u{2713}" } else { "" }} }
                                    if show_denies {
                                        td {
                                            class: if r.denies { "text-warning" } else { "" },
                                            {if r.denies { "refuses" } else { "" }}
                                        }
                                    }
                                    td { class: "text-base-content/60", "{r.criteria}" }
                                    td { class: "text-base-content/60", "{r.actions}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The two tiles, and what neither is a measure of.
///
/// Side by side because they are the same mechanism pointed at two different
/// moments: one runs when a person touches a ticket, the other when an agent
/// submits an inventory. The second is the one this app exists around, which
/// is why it is not buried in the collection list below.
#[component]
fn RuleTiles(report: api::RulesReport) -> Element {
    // The three GLPI calls business rules, summed: they are one feature, and
    // two of them being empty belongs in the same sentence as the third
    // having two.
    let business: Vec<&api::RuleCollection> = ["Ticket", "Change", "Problem"]
        .iter()
        .filter_map(|t| report.collection(t))
        .collect();
    let b_total: usize = business.iter().map(|c| c.total).sum();
    let b_active: usize = business.iter().map(|c| c.active).sum();
    let b_breakdown = business
        .iter()
        .map(|c| format!("{} {}", c.total, c.rule_type.to_lowercase()))
        .collect::<Vec<_>>()
        .join(" \u{b7} ");

    // Only the business collections can carry it, so it is summed over the
    // same three the first tile counts rather than over everything.
    let b_stops: usize = business.iter().map(|c| c.stops).sum();

    let import = report.collection("ImportAsset");

    // Which tile's rules are unfolded, or neither. One at a time: the two
    // lists are 2 rows and 71, and side by side the short one would be read
    // as a continuation of the long one. Not mut - each link takes its own
    // copy to write through.
    let open = use_signal(|| None::<&'static str>);

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
            StatTile {
                title: "Business rules".to_string(),
                value: b_total.to_string(),
                desc: if b_total == 0 {
                    "none defined".to_string()
                } else {
                    format!("{b_active} active \u{b7} {b_breakdown}")
                },
                body: "GLPI's own term, and it means three collections and no others: \
                       business rules for tickets, for changes and for problems. They run \
                       when one of those is created or updated, and they can set fields on \
                       it without anyone asking - assign a group or a technician, set a \
                       category or a priority, take the location from the affected item. \
                       The two this install ships are both of that last kind. Read live from \
                       GLPI's Rule controller, one request per collection, so the figure is \
                       scoped to this profile and these entities rather than counted off \
                       the glpi_rules table, which would report rules the caller cannot \
                       see. Two things the number does not tell you. It is not a count of \
                       everything GLPI calls a rule - measured on 2026-11-20, this install \
                       holds 91 rules across nine collections, and the import rules in the \
                       tile beside this one are not business rules and are not counted \
                       here. And a rule that exists may do nothing at all, because each \
                       carries its own is_active - which is why this tile reports the \
                       active count beside the total rather than the total alone. What a \
                       rule WILL do is not readable from any count - its criteria tree and \
                       action list are interpreted by PHP, which is the same line this \
                       whole page draws between GLPI's data and GLPI's program."
                    .to_string(),
                links: vec![InfoLink::new("Rules", "front/rule.php")],
                action: rsx! {
                    button {
                        class: if open() == Some("business") { "link link-primary font-semibold" } else { "link link-primary" },
                        onclick: move |_| {
                            let mut slot = open;
                            if slot() == Some("business") { slot.set(None) } else { slot.set(Some("business")) }
                        },
                        if open() == Some("business") { "hide the rules" } else { "show the rules" }
                    }
                },
            }

            if let Some(c) = import {
                StatTile {
                    title: "Import rules".to_string(),
                    value: c.total.to_string(),
                    desc: format!("{} active \u{b7} {} can refuse", c.active, c.denies),
                    body: "The rules that decide what happens to an inventory submission \
                           before it becomes anything: GLPI runs this collection against \
                           every agent report that arrives. They are a RANKED list and the \
                           first match wins, so the number says nothing on its own - rule 1 \
                           can end the matter for every submission that reaches it, and a \
                           rule ranked last fires only on what the 70 above it did not \
                           claim. What they match on is identity: serial, uuid, mac \
                           address, name, and whether the report is partial. What they then \
                           do is one of three things - link the report to an asset that \
                           already exists, let a new asset be created, or refuse it \
                           outright. The refusals are the ones this app is built around. A \
                           rule refuses by setting _ignore_import, GLPI names them plainly \
                           - Computer import denied, Printer import denied, No creation on \
                           partial import, the constraint rules - and a submission that \
                           lands on one gets written to glpi_refusedequipments with the \
                           rule's own id, which is the table the Refused page reads and the \
                           only record that the submission ever happened. The agent is told \
                           nothing: it exits 0 and its log says the report was sent. That \
                           is the failure this whole app exists to surface, and these are \
                           the rules that cause it. Measured on 2026-11-20: 17 of the 71 \
                           can refuse and all 17 are active, the 14 switched off are the \
                           by-mac and by-uuid matching variants, and \
                           glpi_refusedequipments holds no rows - so nothing has ever \
                           actually been refused on this install, which is a finding rather \
                           than a gap."
                        .to_string(),
                    links: vec![InfoLink::new("Rules", "front/rule.php")],
                    action: rsx! {
                        button {
                            class: if open() == Some("import") { "link link-primary font-semibold" } else { "link link-primary" },
                            onclick: move |_| {
                                let mut slot = open;
                                if slot() == Some("import") { slot.set(None) } else { slot.set(Some("import")) }
                            },
                            if open() == Some("import") { "hide the rules" } else { "show the rules" }
                        }
                    },
                }
            }

            StatTile {
                title: "Skip remaining rules".to_string(),
                value: b_stops.to_string(),
                desc: if b_total == 0 {
                    "no business rules to skip".to_string()
                } else {
                    format!("of {b_total} business rules")
                },
                body: "The one action whose effect is on the other rules rather than on \
                       the ticket, and the answer to what happens when a rule chain gets \
                       long. Why it exists: the two collections on this tab are \
                       evaluated differently. The import rules stop at the first match - \
                       RuleImportAssetCollection sets stop_on_first_match true - so GLPI \
                       walks them in rank order and quits as soon as one claims the \
                       submission. Business rules do not: RuleCollection leaves that \
                       false and RuleTicketCollection does not override it, so every \
                       active rule is evaluated against every ticket create and update, \
                       and every rule whose criteria hold gets to act. That is linear in \
                       active rules, paid inside the request that creates the ticket. \
                       The escape hatch is a rule action, not a setting: \
                       Rule::getActions offers _stop_rules_processing, labelled Skip \
                       remaining rules in the interface, and RuleCollection honours it \
                       by ending the chain for whatever that rule matched. Where you can \
                       use it: getActions adds it only when the collection does not \
                       already stop on first match, so it appears on a business rule and \
                       never on an import one - GLPI hides the hatch exactly where it \
                       would be redundant. There is no maximum number of rules anywhere \
                       in GLPI; the ceilings are glpi_rules.id as an unsigned int and \
                       ranking as a signed one, which nobody will reach, so the thing \
                       that actually limits a chain is this action and the cost of the \
                       rules it skips."
                    .to_string(),
                links: vec![InfoLink::new("Rules", "front/rule.php")],
            }
        }

        if open() == Some("business") {
            div { class: "mb-5",
                RuleList {
                    title: "Business rules".to_string(),
                    subtitle: format!("{b_total} across {} collections", business.len()),
                    note: "Three collections in one list, each ranked within its own \u{2014} \
                           ranks repeat across them because a ticket rule and a change rule \
                           never compete. Unlike the import rules, these do not stop at the \
                           first match: every rule whose criteria hold gets to act."
                        .to_string(),
                    show_collection: true,
                    show_denies: false,
                    rules: business
                        .iter()
                        .flat_map(|c| c.rules.iter().map(move |r| (c.name.clone(), r.clone())))
                        .collect(),
                }
            }
        }

        if open() == Some("import") {
            if let Some(c) = import {
                div { class: "mb-5",
                    RuleList {
                        title: c.name.clone(),
                        subtitle: format!("{} in rank order", c.total),
                        note: "Rank order is the meaning here, not a sort: GLPI walks this \
                               list from the top and the first rule whose criteria hold ends \
                               the matter, so everything below it never runs on that \
                               submission. A rule marked refuses sets _ignore_import - the \
                               submission goes to glpi_refusedequipments and the agent is \
                               told nothing."
                            .to_string(),
                        show_collection: false,
                        show_denies: true,
                        rules: c.rules.iter().map(|r| (c.name.clone(), r.clone())).collect(),
                    }
                }
            }
        }

        div { class: "max-w-prose space-y-2 text-sm text-base-content/70",
            for c in report.collections.iter() {
                p { key: "{c.rule_type}",
                    span { class: "font-medium", "{c.name}: " }
                    if c.total == 0 {
                        "none."
                    } else if c.denies > 0 {
                        "{c.total} defined, {c.active} active, {c.denies} of which can refuse an import."
                    } else {
                        "{c.total} defined, {c.active} active."
                    }
                }
            }
            p { class: "text-xs text-base-content/60",
                "Four of GLPI's nine collections, because each one costs a walk of its \
                 rules. The other five - itemtype, entity, location, authorizations, \
                 software category - are reachable the same way when something wants them."
            }
        }
    }
}

/// The fourth sub-tab, named for HeidiSQL - the client this server is reached
/// with from the desktop, as distinct from the three sub-tabs beside it, which
/// are all about the server itself.
///
/// Three links, and all three answer a question HeidiSQL's own grids provoke
/// and then answer worst - every one of them a count that is easy to read off
/// the screen wrong:
///
/// - **plugins**: 71 rows in one flat table with the type column visible a few
///   rows at a time, which is how a stock server reads as three kinds of
///   plugin when it has six.
/// - **attributes**: what the table designer lists down the side - five, for
///   `glpi_alerts`, and a fact about GLPI's schema.
/// - **properties**: what it lists across the top - fourteen, and a fact about
///   HeidiSQL. Headings that look like data are what makes a five-attribute
///   table read as a fourteen-attribute one.
///
/// The second and third are one fetch, because the page's answer is the pair
/// and half of it would be misleading on its own.
#[component]
fn Heidi() -> Element {
    // Fetched inside the tab, never with the page: the three sub-tabs beside
    // this one must not pay for a query nobody asked for.
    let report = use_resource(|| async move { api::fetch_plugins().await });
    let schema = use_resource(|| async move { api::fetch_attributes().await });

    // Shut until asked, and one at a time. The reader arrived here for one
    // link, and 71 rows unfolded under it would be the thing HeidiSQL already
    // does. By name rather than index for the same reason the plugin strip
    // uses names: an index is a position, and positions move. Not mut - each
    // link's handler takes its own copy to write through.
    let open = use_signal(|| None::<&'static str>);
    // Which group is unfolded, by name rather than index: the backend orders
    // the groups and an index would silently point at a different type the
    // first time that order changed. Not mut here - the strip owns the
    // writing, and it takes its own copy of the signal to do it.
    let at = use_signal(|| None::<String>);

    rsx! {
        Panel {
            title: "heidi".to_string(),
            subtitle: "What HeidiSQL is looking at".to_string(),
            info: "Three readings of what this client puts on screen, each read live from the \
                   server rather than quoted: its plugin grid, and the two halves of its table \
                   designer. The designer lists a table's attributes down the side and their \
                   properties across the top, so the headings sit where data sits and get \
                   counted as though they were some."
                .to_string(),

            div { class: "flex flex-wrap items-center gap-x-5 gap-y-1 text-sm",

                span {
                    button {
                        class: if open() == Some("plugins") { "link link-primary font-semibold" } else { "link link-primary" },
                        onclick: move |_| {
                            let mut slot = open;
                            if slot() == Some("plugins") { slot.set(None) } else { slot.set(Some("plugins")) }
                        },
                        "plugins"
                    }
                    if let Some(Ok(r)) = &*report.read_unchecked() {
                        if r.configured {
                            span { class: "text-base-content/60",
                                " - {r.types.len()} kinds, {r.types.iter().map(|t| t.plugins.len()).sum::<usize>()} in total"
                            }
                        }
                    }
                }

                span {
                    button {
                        class: if open() == Some("attributes") { "link link-primary font-semibold" } else { "link link-primary" },
                        onclick: move |_| {
                            let mut slot = open;
                            if slot() == Some("attributes") { slot.set(None) } else { slot.set(Some("attributes")) }
                        },
                        "attributes"
                    }
                    if let Some(Ok(r)) = &*schema.read_unchecked() {
                        if r.configured {
                            span { class: "text-base-content/60", " - {r.attributes.len()} on {r.table}" }
                        }
                    }
                }

                span {
                    button {
                        class: if open() == Some("properties") { "link link-primary font-semibold" } else { "link link-primary" },
                        onclick: move |_| {
                            let mut slot = open;
                            if slot() == Some("properties") { slot.set(None) } else { slot.set(Some("properties")) }
                        },
                        "properties"
                    }
                    if let Some(Ok(r)) = &*schema.read_unchecked() {
                        // No "on glpi_alerts" here, and that is the point: the
                        // same fourteen are shown for every table on every
                        // server this client connects to.
                        span { class: "text-base-content/60", " - {r.properties.len()}, the same on every table" }
                    }
                }
            }

            if open() == Some("plugins") {
                div { class: "mt-3",
                    match &*report.read_unchecked() {
                        None => rsx! {
                            div { class: "text-sm text-base-content/60", "Reading the server\u{2019}s plugin list\u{2026}" }
                        },
                        Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
                        Some(Ok(r)) if !r.configured => rsx! {
                            div { class: "text-sm text-base-content/60",
                                "This backend has no database URL, so it cannot ask. That is not \
                                 an empty plugin list - it is no answer at all."
                            }
                        },
                        Some(Ok(r)) => rsx! { PluginKinds { types: r.types.clone(), at } },
                    }
                }
            }

            if matches!(open(), Some("attributes") | Some("properties")) {
                div { class: "mt-3",
                    match &*schema.read_unchecked() {
                        None => rsx! {
                            div { class: "text-sm text-base-content/60", "Reading the table\u{2019}s attributes\u{2026}" }
                        },
                        Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
                        Some(Ok(r)) => {
                            // Properties render with no database at all - they
                            // are this app's list, not the server's - so the
                            // unconfigured case is checked inside the
                            // attributes arm only.
                            if open() == Some("attributes") {
                                rsx! { TableAttributes { report: r.clone() } }
                            } else {
                                rsx! { AttributeProperties { report: r.clone() } }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One link per plugin type, each unfolding that type's own plugins.
///
/// Largest group first, as the backend sent them. The count rides in the link
/// because it is the whole reason the breakdown is worth having: 41 of the 71
/// are information_schema tables, so a flat list reads as a server carrying far
/// more choices than it does.
#[component]
fn PluginKinds(types: Vec<api::PluginType>, at: Signal<Option<String>>) -> Element {
    rsx! {
        div { class: "flex flex-wrap items-center gap-x-3 gap-y-1 text-sm",
            for t in types.iter() {
                {
                    let name = t.name.clone();
                    let here = at().as_deref() == Some(t.name.as_str());
                    rsx! {
                        button {
                            key: "{t.name}",
                            class: if here { "link link-primary font-semibold" } else { "link" },
                            onclick: move |_| {
                                let name = name.clone();
                                let mut slot = at;
                                // Clicking the open one closes it, so the strip
                                // is a toggle rather than a thing that can only
                                // ever be more open.
                                if slot().as_deref() == Some(name.as_str()) {
                                    slot.set(None);
                                } else {
                                    slot.set(Some(name));
                                }
                            },
                            "{t.name.to_lowercase()} ({t.plugins.len()})"
                        }
                    }
                }
            }
        }

        if let Some(name) = at() {
            if let Some(t) = types.iter().find(|t| t.name == name) {
                div { class: "mt-3", PluginRows { kind: t.clone() } }
            }
        }
    }
}

/// One type's plugins, with the note on what that type is.
///
/// The note is this app's and the table is the server's, which is the split
/// every panel on this page keeps: MariaDB publishes no explanation of what a
/// PLUGIN_TYPE means, so the info button is a judgement and is written as one,
/// while every row below it is read.
#[component]
fn PluginRows(kind: api::PluginType) -> Element {
    let note = PLUGIN_TYPE_NOTES
        .iter()
        .find(|(n, _)| *n == kind.name)
        .map(|(_, body)| (*body).to_string())
        // Not an empty string: a type this app has never seen is a finding
        // about the server, and saying nothing would read as saying it does
        // not matter.
        .unwrap_or_else(|| {
            "This app has no note on this plugin type. It came from the server, so the \
             rows below are real - what is missing is only the explanation, which means \
             this build of MariaDB carries a type that was not here when the page was \
             written."
                .to_string()
        });

    rsx! {
        Panel {
            title: kind.name.to_lowercase(),
            subtitle: format!("{} on this server", kind.plugins.len()),
            info: note,
            div { class: "overflow-x-auto",
                table { class: "table table-xs",
                    thead {
                        tr {
                            th { "name" }
                            th { "status" }
                            th { "load" }
                            th { "maturity" }
                            th { "what the server calls it" }
                        }
                    }
                    tbody {
                        for p in kind.plugins.iter() {
                            tr { key: "{p.name}",
                                td { class: "font-mono text-xs whitespace-nowrap", "{p.name}" }
                                // Anything but ACTIVE is the row worth seeing,
                                // and on this server exactly one is.
                                td {
                                    class: if p.status == "ACTIVE" { "" } else { "text-warning" },
                                    "{p.status}"
                                }
                                td { "{p.load_option}" }
                                td { "{p.maturity}" }
                                td { class: "text-base-content/70", "{p.description}" }
                            }
                        }
                    }
                }
            }
            p { class: "mt-2 text-xs text-base-content/60",
                // The column HeidiSQL shows and nobody reads. Said once under
                // the table rather than as an empty column in it.
                "Every one of these is compiled into the server binary - PLUGIN_LIBRARY is \
                 null for all 71 rows, so nothing on this install was installed as a plugin."
            }
        }
    }
}

/// What each PLUGIN_TYPE is, as against what the server says each plugin is.
///
/// Keyed by the server's own word so a build carrying a seventh type falls
/// through to the honest default rather than being quietly labelled as one of
/// these six. Read on mariadb-server-11.4.12; every claim about this install
/// was checked against it rather than recalled.
const PLUGIN_TYPE_NOTES: [(&str, &str); 6] = [
    (
        "INFORMATION SCHEMA",
        "Each one adds a table to the information_schema database, generated per query \
         rather than stored. The largest group by a long way and the one nobody chose: \
         most of the 41 arrive with InnoDB, which brings its own buffer-pool, lock, \
         compression and tablespace views. One is switched off - FEEDBACK, which would \
         report usage statistics to mariadb.org if it were on - and it is the only row in \
         all six groups whose status is not ACTIVE.",
    ),
    (
        "STORAGE ENGINE",
        "What a table can be created with: each decides how rows are stored and indexed, \
         what locking they take, and whether a transaction can be rolled back. Not all \
         twelve are engines a CREATE TABLE would name - binlog, online_alter_log and \
         PERFORMANCE_SCHEMA are pseudo-engines the server uses to present its own \
         internals, and partition is a helper wrapped around another engine. All 447 of \
         GLPI\u{2019}s tables are InnoDB, which is the only one here that gives GLPI the \
         transaction its inventory import depends on.",
    ),
    (
        "FUNCTION",
        "SQL functions the server loads rather than compiles into its grammar. All ten \
         here are the address and identifier helpers that come with the data types in the \
         group below - inet_aton, inet6_ntoa, is_ipv4, is_ipv6, uuid, sys_guid and the \
         rest - so FUNCTION and DATA TYPE on this server are two halves of the same \
         thing rather than two separate capabilities.",
    ),
    (
        "AUTHENTICATION",
        "How an account proves who it is. Each account in mysql.global_priv names one of \
         these, and which one it names decides whether a password exists at all: \
         unix_socket authenticates from the peer credential the kernel attaches to a \
         local socket, which is what this app\u{2019}s own account uses - no password is \
         stored anywhere and it cannot authenticate over TCP. The other two are password \
         schemes, mysql_old_password being the pre-4.1 one kept for compatibility.",
    ),
    (
        "DATA TYPE",
        "Column types the server\u{2019}s parser does not have built in: INET4, INET6 and UUID \
         arrive as plugins. Available rather than in use - no column in GLPI\u{2019}s 447 tables \
         has any of the three, because GLPI stores an address as a varchar and its own \
         schema has to install on MySQL too, where these do not exist.",
    ),
    (
        "REPLICATION",
        "Plugins that take part in a replication stream. Both are Galera\u{2019}s - wsrep and \
         wsrep_provider - and they are present because this build was compiled with \
         cluster support, which is not the same as replication being configured. This \
         server is a single node with wsrep_on off, so neither does anything today.",
    ),
];

/// One attribute's value for one property, by the property's heading.
///
/// The single place that knows which field answers which heading, so the two
/// panels below cannot disagree about it. None means the property is blank on
/// that attribute - which is a real answer and renders as one, distinct from
/// a property this app cannot read at all.
fn property_value(a: &api::Attribute, heading: &str) -> Option<String> {
    // "yes" rather than a tick for the booleans: these are read as text in the
    // properties table, where a tick in a column of sentences reads as noise.
    let flag = |b: bool| b.then(|| "yes".to_string());
    match heading {
        "Name" => Some(a.name.clone()),
        "Datatype" => Some(a.datatype.clone()),
        "Length/Set" => a.length_set.clone(),
        "Unsigned" => flag(a.unsigned),
        "Allow NULL" => flag(a.allow_null),
        "Zerofill" => flag(a.zerofill),
        "Default" => a.default.clone(),
        "Comment" => (!a.comment.is_empty()).then(|| a.comment.clone()),
        "Collation" => a.collation.clone(),
        "Expression" => a.expression.clone(),
        "Virtuality" => a.virtuality.clone(),
        // Not a lookup that failed. The server publishes no per-column SRID,
        // so there is nothing for any attribute to have one in.
        "SRID" => None,
        "Invisible" => flag(a.invisible),
        "Compressed" => flag(a.compressed),
        // A heading this app has no field for is a backend that grew one,
        // which the properties table reports rather than hides.
        _ => None,
    }
}

/// One table's attributes - the rows of HeidiSQL's designer, which are the
/// only things on that screen that are the table's.
///
/// Five, for `glpi_alerts`, and the count is read rather than written: it is
/// a fact about GLPI's schema and changes with GLPI, unlike the fourteen
/// beside it.
#[component]
fn TableAttributes(report: api::AttributesReport) -> Element {
    if !report.configured {
        return rsx! {
            div { class: "text-sm text-base-content/60",
                "This backend has no database URL, so it cannot ask. That is not a table \
                 with no attributes - it is no answer at all. The properties link beside \
                 this one still works: those are this app\u{2019}s list and need no server."
            }
        };
    }

    let n = report.attributes.len();
    let of_total = match report.tables_total {
        Some(t) => format!("{n} on {}, one of {t} tables", report.table),
        None => format!("{n} on {}", report.table),
    };

    rsx! {
        Panel {
            title: "attributes".to_string(),
            subtitle: of_total,
            info: format!(
                "What the table itself has, and the only part of HeidiSQL\u{2019}s designer that \
                 is about {}. Read from information_schema.COLUMNS now rather than quoted, \
                 because a GLPI upgrade can add one. The grid\u{2019}s headings are not among \
                 these - they describe each row below rather than joining it, and they are \
                 the properties link beside this one.",
                report.table,
            ),

            div { class: "overflow-x-auto",
                table { class: "table table-xs",
                    thead {
                        tr {
                            th { "#" }
                            th { "name" }
                            th { "datatype" }
                            th { "length/set" }
                            th { "unsigned" }
                            th { "allow null" }
                            th { "default" }
                            th { "collation" }
                            th { "key" }
                        }
                    }
                    tbody {
                        for a in report.attributes.iter() {
                            tr { key: "{a.name}",
                                td { class: "text-base-content/50", "{a.position}" }
                                td { class: "font-mono text-xs whitespace-nowrap", "{a.name}" }
                                td { "{a.datatype}" }
                                td { class: "font-mono text-xs", {a.length_set.clone().unwrap_or_default()} }
                                td { {if a.unsigned { "\u{2713}" } else { "" }} }
                                td { {if a.allow_null { "\u{2713}" } else { "" }} }
                                // "No default" rather than an empty cell: the
                                // attribute must then be given a value on
                                // every INSERT, which an empty cell reads as
                                // the opposite of.
                                td {
                                    class: if a.default.is_none() { "text-base-content/50 italic" } else { "font-mono text-xs" },
                                    {a.default.clone().unwrap_or_else(|| "no default".to_string())}
                                }
                                td { class: "text-base-content/70 text-xs", {a.collation.clone().unwrap_or_default()} }
                                // What the designer draws as key icons, which
                                // is the one thing on its grid that is neither
                                // an attribute nor one of the fourteen.
                                td { class: "text-xs", "{a.key}" }
                            }
                        }
                    }
                }
            }
            p { class: "mt-2 text-xs text-base-content/60",
                "The key column is HeidiSQL\u{2019}s row of key icons, not a property: it says which \
                 indexes an attribute is in, and an attribute can be in more than one."
            }
        }
    }
}

/// The fourteen properties the designer shows for each attribute - the
/// headings across the top, which are HeidiSQL's and not the table's.
///
/// Each row carries where the value is read from and how many of this table's
/// attributes actually have one, because the second is what settles the
/// confusion: a property that is blank on all five is still a property, and a
/// grid full of empty columns is what makes the headings look like a list of
/// things the table has.
#[component]
fn AttributeProperties(report: api::AttributesReport) -> Element {
    let attrs = &report.attributes;
    let n = attrs.len();

    // Counted, not quoted - and worth not hard-coding: glpi_alerts was read as
    // seven from the designer screenshot and is eight, because every attribute
    // is NOT NULL and so leaves Allow NULL unticked as well. Blank here means
    // the grid cell is empty, which for a boolean property is a real answer
    // ("none of them") rather than a missing one.
    let blank = report
        .properties
        .iter()
        .filter(|(h, _, _)| !attrs.iter().any(|a| property_value(a, h).is_some()))
        .count();

    rsx! {
        Panel {
            title: "properties".to_string(),
            subtitle: format!("{} per attribute, on every table", report.properties.len()),
            info: "HeidiSQL\u{2019}s designer shows these fourteen across the top, one column each, \
                   for every table on every server. They describe an attribute rather than \
                   being one, so counting them as the table\u{2019}s is how a five-attribute \
                   table reads as a fourteen-attribute one. This list is written into this \
                   app, unlike everything else on this sub-tab: MariaDB publishes columns, \
                   and which of them a client lays out as a heading is the client\u{2019}s choice.\n\n\
                   Compressed is the one worth spelling out, because what decides it is not \
                   size. Set it by putting the word after the type - ALTER TABLE t MODIFY c \
                   VARCHAR(255) COMPRESSED - and the server stores it as the version-gated \
                   comment /*M!100301 COMPRESSED*/, which is what the Compressed column here \
                   is reading.\n\n\
                   It takes only the variable-length string types: VARCHAR, VARBINARY, TEXT \
                   and BLOB. CHAR(255) COMPRESSED is a syntax error rather than a warning, a \
                   fixed-width type having nothing to shorten. And a compressed column cannot \
                   be indexed at all - ADD INDEX answers 4097, a prefix index included - so \
                   any attribute you search or sort on is ruled out before its size is even \
                   asked about. Measured on this server, not quoted.\n\n\
                   Two things that make it look like it did nothing. Values shorter than \
                   column_compression_threshold are stored uncompressed - 100 bytes here - so \
                   a compressed attribute full of short strings is byte-for-byte what it was. \
                   And on a glpi_ table it does not last: GLPI\u{2019}s upgrade migrations issue \
                   their own ALTER TABLE, and one that redefines the attribute drops the \
                   COMPRESSED with it, silently, because nothing in GLPI knows it was there."
                .to_string(),

            div { class: "overflow-x-auto",
                table { class: "table table-xs",
                    thead {
                        tr {
                            th { "property" }
                            th { "read from" }
                            if report.configured {
                                th { class: "whitespace-nowrap", "on {report.table}" }
                            }
                            th { "what it is" }
                        }
                    }
                    tbody {
                        for (heading , source , note) in report.properties.iter() {
                            tr { key: "{heading}",
                                td { class: "font-semibold whitespace-nowrap", "{heading}" }
                                // Empty source is not a gap in this app. The
                                // server publishes nothing for it, so the
                                // client's column is empty on every table it
                                // ever opens.
                                td {
                                    class: if source.is_empty() { "text-warning text-xs whitespace-nowrap" } else { "font-mono text-xs whitespace-nowrap" },
                                    {if source.is_empty() { "nothing".to_string() } else { source.clone() }}
                                }
                                if report.configured {
                                    {
                                        let set = attrs.iter().filter(|a| property_value(a, heading).is_some()).count();
                                        rsx! {
                                            td {
                                                class: if set == 0 { "text-base-content/40 whitespace-nowrap" } else { "whitespace-nowrap" },
                                                {if set == 0 { "blank".to_string() } else { format!("{set} of {n}") }}
                                            }
                                        }
                                    }
                                }
                                td { class: "text-base-content/70", "{note}" }
                            }
                        }
                    }
                }
            }
            if report.configured {
                p { class: "mt-2 text-xs text-base-content/60",
                    "{blank} of the {report.properties.len()} are blank on every attribute of \
                     {report.table}, so the grid devotes that many columns to nothing. They \
                     are still properties, and a property with nothing in it is still not an \
                     attribute."
                }
            }
        }
    }
}

/// Which server is answering, how it is set up, and what GLPI requires of
/// one.
///
/// The readings and the requirements sit side by side, which is the
/// comparison the two are for: the version on the left is what the three
/// checks on the right are checks on. The requirements are quoted from GLPI's
/// source
/// and each is given as a query rather than a value, because a value pasted
/// in here would rot the first time the server was upgraded. This page
/// already learned that lesson for its counts.
#[component]
fn MariaGeneral() -> Element {
    rsx! {
        // Passed in rather than placed here: it belongs beside the reading it
        // is a check on, and only the component that owns the fetch knows
        // where that is. It is reference text, so it renders whatever the
        // fetch did.
        MariaFacts { Requirements {} }

        // Between the reading and the InnoDB reference, because it is the
        // index to both: the four tiers cut across every variable the server
        // has, InnoDB's 137 included.
        div { class: "mt-4", Tiers {} }

        // Last and full width: it is the longest thing on the page and it is
        // reference, so it belongs after everything the page reads rather
        // than beside any of it.
        div { class: "mt-4", Innodb {} }
    }
}

/// The three conditions GLPI puts on a server, as opposed to on its schema.
#[component]
fn Requirements() -> Element {
    rsx! {
        Panel {
            title: "What GLPI requires of the server".to_string(),
            subtitle: "Three checks, all run at install time".to_string(),
            info: "Read in src/Glpi/System/Requirement/: DbEngine.php line 72 for the \
                   MariaDB minimum and line 76 for MySQL's, DbConfiguration.php lines 63 \
                   to 71 for the page size, DbTimezones.php lines 64 to 68 for the \
                   timezone tables. GLPI runs all three itself and shows them on its own \
                   status page, so this panel is here to say what they are and how to \
                   check them by hand - not to duplicate a verdict GLPI already gives."
                .to_string(),

            p { class: "mb-3 text-sm",
                "These are conditions on the server, not on the schema. GLPI checks each \
                 one during installation and refuses or warns rather than failing later."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "MariaDB 10.11, or MySQL 8.0. " }
                    "The engine is told apart by looking for "
                    code { class: "text-xs", "-MariaDB" }
                    " in the version string, then the numeric part is compared. "
                    code { class: "text-xs", "select version()" }
                }
                li {
                    span { class: "font-medium", "innodb_page_size at least 8 KB. " }
                    "Smaller pages cannot hold GLPI's wider indexes. The default is 16 KB, \
                     so this only bites on a server someone tuned down. "
                    code { class: "text-xs", "select @@global.innodb_page_size" }
                }
                li {
                    span { class: "font-medium", "The timezone tables must be loaded. " }
                    "GLPI counts the rows and treats zero as timezone support being \
                     unavailable rather than as an error, so the symptom is a missing \
                     feature, not a failure. "
                    code { class: "text-xs", "select count(*) from mysql.time_zone_name" }
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "Nothing in this panel is read live. The first of the three is answered \
                 anyway, by the version in the panel beside it; the other two are not, so \
                 the queries are the way to answer them."
            }
        }
    }
}

/// What the server actually is, read from the running instance.
///
/// The only fetch on this tab that queries the server rather than quoting
/// GLPI's source, and that is the point of it: everything else here describes
/// what GLPI expects of a database, and this says what is answering. The two
/// disagreeing - a different engine, a different collation, a row format GLPI
/// did not ask for - is a finding no other page would surface.
#[component]
fn MariaFacts(
    /// What to put beside the reading, in the other cell of its row. The
    /// requirements panel; nothing else passes anything.
    children: Element,
) -> Element {
    let report = use_resource(|| async move { api::fetch_mariadb().await });

    // One match, one destination. It carried a second until 2026-11-18, when
    // the settings panel under this row was deleted; every arm paired its
    // reading with an empty rsx! for that slot, and none of them needs to now.
    let reading = match &*report.read_unchecked() {
        None => rsx! {
            div { class: "flex items-center gap-2 text-base-content/60",
                span { class: "loading loading-spinner loading-sm" }
                "Asking the server\u{2026}"
            }
        },
        Some(Err(e)) => rsx! {
            FetchError { error: e.clone() }
        },
        // Not an error, and deliberately not an empty table: the app runs
        // without a database URL and only this page and Refused go dark.
        Some(Ok(r)) if !r.configured => rsx! {
            div { class: "alert alert-info",
                "No database connection is configured, so there is no server to ask. "
                "Everything else on this tab is read from GLPI's source and is unaffected."
            }
        },
        Some(Ok(r)) => {
            let facts = r.facts.clone().unwrap_or_else(|| api::ServerFacts {
                version: String::new(),
                flavour: String::new(),
                uptime_seconds: 0,
                variables: Vec::new(),
                engines: Vec::new(),
                glpi_tables: 0,
                glpi_bytes: 0,
                glpi_other_engines: Vec::new(),
            });
            rsx! { ServerAnswering { facts } }
        }
    };

    rsx! {
        // The reading and the check on it, as two cells of one row, so they
        // start at the same line - they are a pair and are read as one.
        // items-start is what keeps that honest: without it the shorter of
        // the two stretches to the row's height and shows a tail of empty
        // card below its last line.
        div { class: "grid gap-4 items-start lg:grid-cols-2",
            {reading}
            {children}
        }
    }
}

/// What is answering, in three readings: which program, how long it has been
/// up, and how much of GLPI is in it.
///
/// First on the page because it is orientation - the first thing to
/// establish about a database is which one is replying - and directly beside
/// what GLPI *requires* of a server, which is the comparison these three
/// readings are for.
#[component]
fn ServerAnswering(facts: api::ServerFacts) -> Element {
    let days = facts.uptime_seconds / 86_400;
    let hours = (facts.uptime_seconds % 86_400) / 3_600;
    let gib = facts.glpi_bytes as f64 / 1_073_741_824.0;

    rsx! {
        Panel {
            title: "The server answering".to_string(),
            subtitle: facts.flavour.clone(),
            info: "Read from the live instance: @@version, @@version_comment and the \
                   Uptime status variable. This panel and the ones below it are read \
                   from the server - what GLPI requires of a server, beside this, is \
                   quoted from its source, and the two disagreeing is the finding."
                .to_string(),
            // Here because the uptime below is what a restart resets, so this
            // is the panel the question arises on.
            actions: rsx! { RestartLinks {} },
            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Version. " }
                    code { class: "text-xs", "{facts.version}" }
                    " - the server's own version, not GLPI's; the two sit together here \
                     and are unrelated. The schema this app quotes was read from GLPI \
                     12.0.1's installer, which says what a fresh install creates, not what \
                     this server has become since."
                }
                li {
                    span { class: "font-medium", "Up for " }
                    "{days} days, {hours} hours. A restart since the last GLPI upgrade is \
                     worth knowing: migrations run once, and a server that came back \
                     afterwards has not re-run them."
                }
                li {
                    span { class: "font-medium", "GLPI's database holds " }
                    "{facts.glpi_tables} tables, {gib:.2} GiB including indexes. The \
                     installer creates 447; a count below that is an incomplete install, \
                     above it a plugin."
                }
            }
        }
    }
}

/// The two restart links, and whatever the script said.
///
/// Both run the same script - scripts/db-restart.sh - which works out which
/// operating system it is on before it does anything, because the answer
/// differs completely: on this host the plain restart is a polkit prompt away
/// and the settings one is impossible, since /etc/my.cnf is a read-only Nix
/// store path. On Debian or RHEL both work. On Windows the file is writable
/// and the consent is not. None of that is decided here; the script decides
/// it and says what it found.
///
/// NEITHER LINK RESTARTS ANYTHING ON THE FIRST CLICK. The first click is a
/// preview: the script runs with DRY_RUN set, printing the commands it would
/// run. Restarting takes a second, separate click on what the preview
/// produced - and for the settings link on this host there is nothing to
/// confirm, because the preview is the whole answer.
#[component]
fn RestartLinks() -> Element {
    let mut run = use_signal(|| None::<Result<api::RestartRun, String>>);
    let mut busy = use_signal(|| false);
    // What the preview was for, so the confirm button repeats the same thing
    // rather than whichever link was clicked last.
    let mut pending = use_signal(|| None::<(String, Vec<String>)>);

    let mut start = move |mode: &str, settings: Vec<String>, preview: bool| {
        let mode = mode.to_string();
        busy.set(true);
        run.set(None);
        if preview {
            pending.set(Some((mode.clone(), settings.clone())));
        } else {
            pending.set(None);
        }
        spawn(async move {
            let r = api::db_restart(&mode, &settings, preview).await;
            run.set(Some(r));
            busy.set(false);
        });
    };

    rsx! {
        div { class: "flex flex-wrap items-center gap-3",
            button {
                class: "link link-primary text-xs",
                disabled: busy(),
                onclick: move |_| start("plain", Vec::new(), true),
                "db restart"
            }
            button {
                class: "link link-primary text-xs",
                disabled: busy(),
                onclick: move |_| start("settings", Vec::new(), true),
                "db restart with non-default InnoDB variables"
            }
            // On this link and not the other one: a plain restart does what
            // its two words say, while this one is named after an operation
            // that, on this host, it cannot perform - and that needs saying
            // before the click rather than only in the output.
            InfoButton {
                title: "db restart with non-default InnoDB variables".to_string(),
                body: "Runs scripts/db-restart.sh, which works out which operating system it \
                       is on before it does anything, because the answer is not the same \
                       twice. The first click only previews: the script runs with DRY_RUN \
                       set and prints what it would do, and a second button appears if there \
                       is anything to confirm. ON THIS HOST THERE IS NOT. NixOS starts the \
                       server with --defaults-file=/etc/my.cnf, a symlink into the Nix store \
                       - mode r--r--r--, owned by root, on a read-only filesystem - so \
                       appending to it fails as root, and the four lines in it are generated \
                       from services.mysql in /etc/nixos/glpi.nix. What comes back is \
                       therefore the instructions: the settings block to paste and the \
                       rebuild to run. That rebuild is also the restart, so do not use the \
                       link beside this one afterwards - mysql.service carries \
                       X-Restart-Triggers naming my.cnf's store path, so a changed variable \
                       means a changed unit and nixos-rebuild switch restarts the server \
                       itself. Before any of it, check whether a restart is needed at all: \
                       106 of this server's 137 InnoDB variables are dynamic and can be \
                       changed over SQL, and only 31 are read at startup. Three of those 31 \
                       are refused by name rather than by validation - innodb_page_size, \
                       innodb_data_file_path and innodb_undo_tablespaces are valid values \
                       that leave the server unable to start against its existing datadir, \
                       which a config file then keeps true across every later restart. With \
                       no variable named the script uses innodb_log_buffer_size=32M as a \
                       stand-in, which is harmless and reversible. On a conventional Linux \
                       host the same click does the work: validate with the server's own \
                       option parser, back up, write one owned drop-in, restart, and put \
                       the backup back if the server does not come up. Running it for real \
                       needs the backend started with GLPI_INV_DB_RESTART=1; previews work \
                       without it."
                    .to_string(),
            }
            if busy() {
                span { class: "loading loading-spinner loading-xs" }
            }
        }

        match &*run.read() {
            None => rsx! {},
            Some(Err(e)) => rsx! {
                div { class: "mt-3 alert alert-error text-sm",
                    div {
                        div { class: "font-semibold", "The script did not run." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },
            Some(Ok(r)) => {
                // The script's exit code, not the HTTP status: 3 means it
                // looked, decided this OS cannot do it, and said what to do
                // instead. That is an answer, not a failure, and colouring it
                // red would be a lie about the commonest case on this host.
                let tone = match r.code {
                    0 => "alert-success",
                    3 => "alert-info",
                    _ => "alert-warning",
                };
                let heading = match (r.code, r.preview) {
                    (0, true) => "This is what it would run. Nothing has run yet.",
                    (0, false) => "Done.",
                    (3, _) => "Not possible on this operating system.",
                    (2, _) => "Refused before anything ran.",
                    _ => "It tried and failed; anything it changed was put back.",
                };
                rsx! {
                    div { class: "mt-3 {tone} alert items-start text-sm",
                        div { class: "w-full",
                            div { class: "font-semibold", "{heading}" }
                            pre { class: "mt-2 max-h-80 overflow-auto text-xs whitespace-pre-wrap",
                                "{r.output}"
                            }
                            div { class: "mt-2 flex flex-wrap items-center gap-3",
                                code { class: "text-xs opacity-70", "{r.script}" }
                                // Only ever offered on a preview that could
                                // actually proceed. Code 3 has nothing to
                                // confirm, and 2 was refused on purpose.
                                if r.preview && r.code == 0 {
                                    if let Some((mode, settings)) = pending() {
                                        button {
                                            class: "btn btn-warning btn-xs",
                                            disabled: busy(),
                                            onclick: move |_| {
                                                let (m, s) = (mode.clone(), settings.clone());
                                                start(&m, s, false)
                                            },
                                            "Run it for real"
                                        }
                                    }
                                }
                                button {
                                    class: "btn btn-ghost btn-xs",
                                    onclick: move |_| {
                                        run.set(None);
                                        pending.set(None);
                                    },
                                    "Dismiss"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What each tier is called, what it costs, and why.
///
/// Shared by the index tile and the four pages, so the sentence a reader
/// clicks and the sentence they land on cannot drift apart.
const TIERS: [(&str, &str, &str); 4] = [
    (
        "live",
        "A statement.",
        "SET GLOBAL takes effect at once and is gone at the next restart, unless Keep \
         writes it into the file MariaDB replays with --init-file. There is no SET \
         PERSIST in MariaDB - that is MySQL 8.",
    ),
    (
        "restart",
        "A config edit and a restart.",
        "The server refuses SET GLOBAL on these and reads them at startup. On this host \
         that means a nixos-rebuild: /etc/my.cnf is a read-only store path, and mysqld runs \
         with --defaults-file, which replaces the whole config search path with that one \
         file - so a drop-in under /etc/my.cnf.d would be well-formed and never read.",
    ),
    (
        "install",
        "A dump, a re-initialised datadir, a restore.",
        "Valid values that leave the server unable to start against the data it already \
         has. Written into a file that persists, they keep it down across every restart \
         after, which is why scripts/db-restart.sh refuses them by name rather than by \
         validation - the value is not what is wrong, so nothing validates against it.",
    ),
    (
        "reports",
        "Nothing that can be paid.",
        "These report the build, the host, or the last statement. Read-only is not the \
         same as constant - warning_count moves with every statement - but no edit reaches \
         one. You change what it reports on instead.",
    ),
];

/// The four tiers, as an index with a link and a count each.
///
/// Cost of change, not subject. The seven groups on the InnoDB tile answer
/// where a variable lives and are read in place; these answer what moving
/// one costs, which is the question somebody sends a colleague a link to.
/// That is why these four are destinations and those seven are not.
///
/// The counts are read live and summed from the tier the backend put each
/// variable in, never typed here. This page has already shipped one wrong
/// denominator and does not need a fifth number nobody re-checks.
#[component]
fn Tiers() -> Element {
    let report = use_resource(|| async move { api::fetch_variables().await });

    // Finding one variable without first knowing which tier it is in.
    //
    // The four links answer "show me everything that costs this much"; this
    // answers "what does THIS one cost, and can I change it" - which is the
    // question someone arrives with, and the one the four links cannot take.
    // Results carry their tier, because a row with no control is read-only
    // rather than broken and the tier is what says which.
    let mut query = use_signal(String::new);
    let drafts = use_signal(Vec::<(String, String)>::new);
    let pending = use_signal(|| None::<(String, String)>);
    let notice = use_signal(|| None::<(String, String)>);

    // Only the figures need the server. The four links and what each costs
    // are in the header, where they are true before any read lands and stay
    // true when one fails - which is why the loading and error states below
    // are about the counts alone and never about the tiers.
    //
    // One row per tier rather than a row of four numbers: the count says how
    // big a tier is, and who chose the values in it is the part a reader can
    // act on. They are opposite answers on this server - every variable
    // anyone has set is in one tier, and the largest tier holds none of them
    // - and a bare count cannot show that.
    let rows: Option<Vec<(usize, String)>> = match &*report.read_unchecked() {
        Some(Ok(r)) if r.configured => Some(
            TIERS
                .iter()
                .map(|(name, ..)| {
                    let mine: Vec<&api::Variable> =
                        r.variables.iter().filter(|v| v.tier == *name).collect();
                    // The server's own origins, in three groups a reader can
                    // do something with: someone typed it, the server worked
                    // it out at startup, or nobody has ever touched it.
                    // AUTO is its own group deliberately - it is not a
                    // default and it is not a decision, and folding it into
                    // either would misreport both.
                    let chosen = mine
                        .iter()
                        .filter(|v| matches!(v.origin.as_str(), "CONFIG" | "COMMAND-LINE" | "SQL"))
                        .count();
                    let auto = mine.iter().filter(|v| v.origin == "AUTO").count();
                    let rest = mine.len() - chosen - auto;
                    // Said in words when the whole tier is untouched, because
                    // "3 untouched" beside a count of 3 prints the same number
                    // twice and reads as two facts.
                    if rest == mine.len() {
                        return (mine.len(), "none set by anyone".to_string());
                    }
                    let mut parts = Vec::new();
                    if chosen > 0 {
                        parts.push(format!("{chosen} set by hand"));
                    }
                    if auto > 0 {
                        parts.push(format!("{auto} sized at startup"));
                    }
                    if rest > 0 {
                        parts.push(format!("{rest} untouched"));
                    }
                    (mine.len(), parts.join(" \u{b7} "))
                })
                .collect(),
        ),
        _ => None,
    };
    let total: Option<usize> = rows.as_ref().map(|r| r.iter().map(|(n, _)| n).sum());

    // Substring and case-insensitive, over the name only. Searching the
    // value would match 378 rows on OFF and tell a reader nothing.
    let needle = query().trim().to_ascii_lowercase();
    let found: Vec<api::Variable> = match &*report.read_unchecked() {
        Some(Ok(r)) if !needle.is_empty() => r
            .variables
            .iter()
            .filter(|v| v.name.to_ascii_lowercase().contains(&needle))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    // A prefix like "innodb" matches 137. Showing all of them would bury the
    // panel this search sits in, so the list is capped and says what it cut
    // rather than silently truncating.
    const HITS: usize = 12;
    let cut = found.len().saturating_sub(HITS);
    let editing = match &*report.read_unchecked() {
        Some(Ok(r)) => Editing {
            writes_enabled: r.writes_enabled,
            can_persist: r.writes_enabled && r.persist_path.is_some(),
            drafts,
            pending,
            notice,
            report,
        },
        _ => Editing {
            writes_enabled: false,
            can_persist: false,
            drafts,
            pending,
            notice,
            report,
        },
    };

    rsx! {
        Panel {
            title: "What changing a system variable costs".to_string(),
            subtitle: match total {
                Some(n) => format!("Four tiers over this server's {n} variables, cheapest first"),
                None => "Four tiers, cheapest first".to_string(),
            },
            info: "Cost of change rather than subject, which is the one cut of these \
                   variables that says what to do next: a subject taxonomy answers where a \
                   variable lives, this answers what moving it will take.\n\n\
                   MariaDB publishes no such cut. It states READ_ONLY per variable and \
                   stops there, so live is the server's own answer and the other three are \
                   a reading of the ones it refuses. Two had to be written down by hand \
                   because no query produces them: information_schema's \
                   COMMAND_LINE_ARGUMENT is null for character_set_server, which -C sets \
                   perfectly well, and the variables table in mariadbd --help --verbose is \
                   a dump of current values that lists read-only names beside settable \
                   ones. What defeats a name join even in principle is that MariaDB's \
                   option names and its variable names are separate namespaces - --memlock \
                   sets locked_in_memory and --external-locking sets \
                   skip_external_locking. Both sat in reports until their options were \
                   found, and both are ordinary restart variables.\n\n\
                   Reports is last because it is the tier with no price rather than a \
                   cheap one; first would read as cheapest. It is 26 of this server's 693 \
                   and nothing you can write reaches one: ten have_* flags compiled into \
                   the binary, five that report the last statement, five replication \
                   positions, and the rest identity - hostname, license, \
                   character_set_system.\n\n\
                   Why a variable nobody can set is worth having: the use case is the \
                   READ, and being unsettable is what makes the answer worth trusting. A \
                   value you can set is a claim the next statement can falsify; one you \
                   cannot is a fact a client may branch on. have_crypt=NO tells a driver \
                   not to call ENCRYPT() - setting it to YES would not compile the \
                   function in, only make the server lie. character_set_system is how a \
                   client knows to encode an identifier. warning_count is read after a \
                   statement: SELECT @@warning_count is the API, and writing it is \
                   incoherent.\n\n\
                   This app already leans on that. Of GLPI's three install-time server \
                   checks, on the panel beside this one, two read variables that cannot be \
                   changed on a running server - version, and innodb_page_size, which is \
                   one of the three in install. GLPI is not configuring anything there; it \
                   is asking whether the server is admissible, and a settable answer would \
                   be useless for that.\n\n\
                   Read-only is not the same as constant - warning_count moves with every \
                   statement - and READ_ONLY on its own would have made this tier 196 \
                   rather than 26."
                .to_string(),

            // In the title row directly after the info button, not in the
            // actions slot: these four are the whole of what this panel is,
            // so they belong to the heading rather than parked at the far end
            // of it, where justify-between put them on a line of their own.
            // Each keeps the one line that used to follow it, and the
            // paragraph that used to follow that is behind its own info
            // button - the tier is a figure a reader could misread, so it
            // gets a modal rather than a tooltip.
            beside_title: rsx! {
                div { class: "flex flex-wrap items-center gap-x-5 gap-y-2",
                    for (name , cost , why) in TIERS {
                        div { key: "{name}", class: "flex items-center gap-1.5",
                            Link {
                                class: "link link-primary font-mono text-sm",
                                to: Route::Db {
                                    tab: "vars".to_string(),
                                    sub: name.to_string(),
                                },
                                "{name}"
                            }
                            span { class: "text-xs text-base-content/60", "{cost}" }
                            InfoButton { title: name.to_string(), body: why.to_string() }
                        }
                    }
                }
            },

            match rows.as_ref() {
                Some(rs) => rsx! {
                    // Grid rather than four flex rows, so the counts align on
                    // their own column: 497 beside 3 only reads as a scale
                    // when the digits line up. `contents` lets each tier's
                    // three cells belong to the one grid.
                    // The same stats strip the Counts panel and the Computers
                    // page use, so four figures read here the way four
                    // figures read everywhere else in this app. Four across
                    // rather than four stacked rows: the tiers are peers
                    // being compared, and a row of three short cells left
                    // most of the panel's width empty.
                    //
                    // Not StatTile, which is the component for this shape,
                    // and the reason is the info buttons: StatTile carries
                    // one beside its title, and these four already have
                    // theirs on the links in the header. Two buttons per
                    // tier opening the same text is a choice a reader should
                    // not have to make.
                    div {
                        class: "stats stats-vertical w-full border border-base-300 bg-base-100 sm:stats-horizontal",
                        for (i , (name , ..)) in TIERS.iter().enumerate() {
                            div { key: "{name}", class: "stat",
                                // A label, not a second link, for the same
                                // reason there is no second info button.
                                div { class: "stat-title font-mono", "{name}" }
                                div { class: "stat-value text-2xl", "{rs[i].0}" }
                                div { class: "stat-desc", "{rs[i].1}" }
                            }
                        }
                    }

                    div { class: "mt-4",
                        div { class: "flex flex-wrap items-baseline gap-2",
                            input {
                                class: "input input-sm w-72 font-mono",
                                r#type: "search",
                                placeholder: "find a variable by name\u{2026}",
                                value: "{query}",
                                oninput: move |e: Event<FormData>| query.set(e.value()),
                            }
                            if !needle.is_empty() {
                                span { class: "text-sm text-base-content/60",
                                    "{found.len()} of {total.unwrap_or(0)} match"
                                }
                                button {
                                    class: "btn btn-ghost btn-xs",
                                    onclick: move |_| query.set(String::new()),
                                    "Clear"
                                }
                            }
                        }

                        if !needle.is_empty() {
                            if found.is_empty() {
                                p { class: "mt-2 text-sm text-base-content/60",
                                    "Nothing here is called that. The name is the server's own "
                                    "spelling - innodb_buffer_pool_size, not innodb buffer pool."
                                }
                            } else {
                                div { class: "mt-2 overflow-x-auto",
                                    table { class: "table table-sm",
                                        tbody {
                                            for v in found.iter().take(HITS) {
                                                {variable_rows(v, false, true, editing)}
                                            }
                                        }
                                    }
                                }
                                if cut > 0 {
                                    p { class: "mt-1 text-sm text-base-content/60",
                                        "{cut} more match. Narrow the name to see them."
                                    }
                                }
                            }
                        }
                    }
                },
                // Three states rather than an empty row, and none of them
                // takes the header down with it: a blank line here would read
                // as a tier with nothing in it.
                None => match &*report.read_unchecked() {
                    Some(Err(e)) => rsx! {
                        FetchError { error: e.clone() }
                    },
                    Some(Ok(_)) => rsx! {
                        div { class: "alert alert-info",
                            "No database connection is configured, so the variables cannot be "
                            "counted. What each tier costs is unaffected."
                        }
                    },
                    None => rsx! {
                        div { class: "flex items-center gap-2 text-base-content/60",
                            span { class: "loading loading-spinner loading-sm" }
                            "Counting\u{2026}"
                        }
                    },
                },
            }
        }
    }
}

/// Subsystems whose variables are inert when their own switch is off.
///
/// (switch variable, what to call it, name prefixes). The switch is READ FROM
/// THE SAME PAYLOAD as the rows, never assumed: a server running Galera must
/// not have its 43 wsrep rows folded away because a list in here said they
/// were dormant. On this server wsrep_on, performance_schema, log_bin and
/// query_cache_type are all OFF, which is 104 of live's 497 and 60 of
/// restart's 167.
///
/// Aria and MyISAM are deliberately absent though they are just as unused
/// here. That rests on all 447 GLPI tables being InnoDB, which is a fact
/// about the schema and not about any variable on this page - nothing in
/// this payload can check it, so folding on it would be an assertion wearing
/// a measurement's clothes.
const DORMANT: [(&str, &str, &[&str]); 4] = [
    ("wsrep_on", "Galera", &["wsrep"]),
    (
        "performance_schema",
        "performance_schema",
        &["performance_schema"],
    ),
    (
        "log_bin",
        "replication",
        &[
            "slave_",
            "master_",
            "binlog_",
            "relay_",
            "rpl_",
            "gtid_",
            "replicate_",
            "sync_binlog",
        ],
    ),
    ("query_cache_type", "the query cache", &["query_cache"]),
];

/// How the rows inside each group may be ordered.
///
/// (key, label, what it does). Editable is here because it was asked for and
/// because the column is real, but it cannot reorder anything on these four
/// pages and the control says so rather than appearing to work: a tier IS
/// the editability cut. `variable_tier` returns live if and only if the
/// server reports READ_ONLY = NO, so live is 497 rows all editable and the
/// other three are 196 rows none of which are. The page annotates the key
/// with which of the two it is looking at, so a dead control is at least an
/// informative one.
/// Everything a row's controls need from the page around them.
///
/// Grouped rather than passed one at a time: there are six, they travel
/// together to every row, and a function taking eight positional arguments
/// has call sites nobody can read. Copy, because every field already is -
/// a Signal and a Resource are handles, not the data.
#[derive(Clone, Copy)]
struct Editing {
    writes_enabled: bool,
    can_persist: bool,
    drafts: Signal<Vec<(String, String)>>,
    pending: Signal<Option<(String, String)>>,
    notice: Signal<Option<(String, String)>>,
    report: Resource<Result<api::VariablesReport, api::ApiError>>,
}

/// One variable's row, with the confirm and result rows that belong under it.
///
/// `dim` is the dormant case - the subsystem this belongs to is switched off,
/// so the row is quieter. It keeps its control regardless: a dormant variable
/// is as settable as any other, and `wsrep_on` is both dormant here and one
/// of the nine that needs confirming, so the confirm row has to reach these
/// too.
fn variable_rows(v: &api::Variable, dim: bool, show_tier: bool, ed: Editing) -> Element {
    // The search on the tiers panel reaches every tier at once, so a row
    // there has to say which one it is in - that is the panel's whole
    // subject, and without it a row with no control looks broken rather
    // than read-only. The tier pages already say it in their heading.
    let cols = if show_tier { "5" } else { "4" };
    let Editing {
        drafts,
        mut pending,
        mut notice,
        report,
        ..
    } = ed;
    let name = v.name.clone();
    let current = v.value.clone();
    rsx! {
        tr {
            key: "{v.name}",
            class: if dim { "opacity-60" } else { "" },
            if show_tier {
                td { class: "font-mono text-xs whitespace-nowrap text-base-content/60", "{v.tier}" }
            }
            td { class: "font-mono text-xs whitespace-nowrap", "{v.name}" }
            // Empty rather than absent, and left that way: a variable with no
            // global value has nothing to show, and an em dash would read as
            // the server declining to say.
            td { class: "font-mono text-xs break-all", "{v.value}" }
            td { class: "text-xs whitespace-nowrap text-base-content/60", "{v.origin}" }
            td { class: "text-xs",
                {control_cell(v, ed)}
            }
        }
        // Under the row it is about rather than in a modal: what is being
        // confirmed is this variable at this value, and a dialog would take
        // both off the screen to ask about them.
        if pending().is_some_and(|(n, _)| n == v.name) {
            tr { key: "confirm-{v.name}",
                td { class: "p-0", colspan: "{cols}",
                    div { class: "flex flex-wrap items-baseline gap-2 bg-warning/20 px-2 py-1 text-xs",
                        span { class: "font-semibold", "Setting this breaks something:" }
                        span { "{pending().map(|(_, why)| why).unwrap_or_default()}" }
                        button {
                            class: "btn btn-warning btn-xs",
                            onclick: move |_| {
                                let name = name.clone();
                                let wanted = draft_for(&drafts(), &name, &current);
                                spawn(async move {
                                    write_variable(name, wanted, true, pending, notice, report).await;
                                });
                            },
                            "Set it anyway"
                        }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| pending.set(None),
                            "Leave it"
                        }
                    }
                }
            }
        }
        if notice().is_some_and(|(n, _)| n == v.name) {
            tr { key: "notice-{v.name}",
                td { class: "p-0", colspan: "{cols}",
                    div { class: "flex flex-wrap items-baseline gap-2 bg-base-300 px-2 py-1 text-xs",
                        span { "{notice().map(|(_, what)| what).unwrap_or_default()}" }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| notice.set(None),
                            "Dismiss"
                        }
                    }
                }
            }
        }
    }
}

/// The control one row gets, or nothing where a control cannot work.
///
/// A plain function rather than a component: it needs five pieces of the
/// caller's state and gives nothing back, so props would be ceremony around
/// a block of markup. Shared by the grouped rows and the dormant ones, which
/// is the point - a dormant variable is as settable as any other, it just
/// belongs to a subsystem that is switched off, and giving it a control in
/// one place and not the other would be a distinction the server does not
/// make.
fn control_cell(v: &api::Variable, ed: Editing) -> Element {
    let Editing {
        writes_enabled,
        can_persist,
        mut drafts,
        pending,
        notice,
        report,
    } = ed;
    if !writes_enabled || !v.settable {
        return rsx! {};
    }
    let name = v.name.clone();
    let current = v.value.clone();
    let keep_name = v.name.clone();
    let keep_current = v.value.clone();
    let kept = v.persisted.clone();
    let disagrees = v.persisted.clone().filter(|k| *k != v.value);
    let shown = draft_for(&drafts(), &v.name, &v.value);

    // One closure for both controls: a select and a text box differ in what
    // they offer, not in what they record.
    let mut remember = {
        let name = v.name.clone();
        move |typed: String| {
            let mut d = drafts.write();
            match d.iter_mut().find(|(n, _)| *n == name) {
                Some((_, slot)) => *slot = typed,
                None => d.push((name.clone(), typed)),
            }
        }
    };

    rsx! {
        div { class: "flex items-center gap-1",
            if v.options.is_empty() {
                input {
                    class: "input input-xs w-32",
                    r#type: if is_numeric(&v.kind) { "number" } else { "text" },
                    value: "{shown}",
                    oninput: move |e: Event<FormData>| remember(e.value()),
                }
            } else {
                // The server's own enum list, so the control cannot propose a
                // value the server would refuse.
                select {
                    class: "select select-xs",
                    onchange: move |e: Event<FormData>| remember(e.value()),
                    for o in v.options.iter() {
                        option { key: "{o}", value: "{o}", selected: shown == *o, "{o}" }
                    }
                }
            }
            button {
                class: "btn btn-xs",
                onclick: move |_| {
                    let name = name.clone();
                    let wanted = draft_for(&drafts(), &name, &current);
                    spawn(async move {
                        write_variable(name, wanted, false, pending, notice, report).await;
                    });
                },
                "Set"
            }
            if can_persist {
                button {
                    class: if kept.is_some() {
                        "btn btn-xs btn-active"
                    } else {
                        "btn btn-ghost btn-xs"
                    },
                    onclick: {
                        let name = keep_name.clone();
                        let current = keep_current.clone();
                        let already = kept.is_some();
                        move |_| {
                            let name = name.clone();
                            // Dropping a line needs no value; adding one
                            // keeps whatever the control is showing, which is
                            // the draft if anything has been typed.
                            let wanted = (!already)
                                .then(|| draft_for(&drafts(), &name, &current));
                            spawn(async move {
                                keep_variable(name, wanted, notice, report).await;
                            });
                        }
                    },
                    if kept.is_some() { "Kept" } else { "Keep" }
                }
            }
            // Only when they disagree, which is the whole reason the kept
            // value is carried beside the running one: this row will become
            // something else at the next start.
            if let Some(k) = disagrees {
                span { class: "text-warning whitespace-nowrap",
                    "restart \u{2192} {k}"
                }
            }
        }
    }
}

/// Keep a variable across restarts, or stop keeping it.
///
/// No confirm step even for the nine, and deliberately: a line in the startup
/// file changes nothing until the next start and can be removed again before
/// then. What needs the second look is the write that takes effect at once.
async fn keep_variable(
    name: String,
    value: Option<String>,
    mut notice: Signal<Option<(String, String)>>,
    mut report: Resource<Result<api::VariablesReport, api::ApiError>>,
) {
    match api::persist_variable(&name, value.as_deref()).await {
        Ok(()) => {
            notice.set(Some((
                name,
                match value {
                    Some(v) => format!("Kept as {v}. The next restart will set it again."),
                    None => "No longer kept. The running server is unchanged - what \
                             changed is the next restart."
                        .to_string(),
                },
            )));
            report.restart();
        }
        Err(e) => notice.set(Some((name, e))),
    }
}

/// One write, and what it does to the three signals that report its outcome.
///
/// Out of the tree because Set and "Set it anyway" differ by one bool, and
/// two copies inline would be two places for the success path to drift.
async fn write_variable(
    name: String,
    value: String,
    confirm: bool,
    mut pending: Signal<Option<(String, String)>>,
    mut notice: Signal<Option<(String, String)>>,
    mut report: Resource<Result<api::VariablesReport, api::ApiError>>,
) {
    match api::set_variable(&name, &value, confirm).await {
        Ok(api::SetOutcome::Done(change)) => {
            pending.set(None);
            notice.set(Some((
                name,
                if change.kept {
                    format!(
                        "The server now reports {}, and the startup file carries it, so a \
                         restart sets it again.",
                        change.value
                    )
                } else {
                    // The write happened; only the keeping did not. Saying
                    // "failed" here would describe the wrong half.
                    format!(
                        "The server now reports {}. It was NOT kept - there is no startup \
                         file configured - so the next restart undoes it.",
                        change.value
                    )
                },
            )));
            // The row's value, its origin, and the headline counting what was
            // set by someone all come from this one fetch. Re-reading is what
            // makes the page agree with the server rather than with what was
            // asked of it - they differ whenever the server rounds a value to
            // a block size.
            report.restart();
        }
        Ok(api::SetOutcome::NeedsConfirmation(why)) => {
            notice.set(None);
            pending.set(Some((name, why)));
        }
        Err(e) => {
            pending.set(None);
            notice.set(Some((name, e)));
        }
    }
}

/// Where a value came from, ranked most deliberate first.
///
/// The server's own vocabulary, not one invented here: GLOBAL_VALUE_ORIGIN
/// has exactly these five and a variable carries exactly one, whichever won.
/// SQL leads because it is both the most deliberate and the most volatile -
/// it is the only origin that does not survive a restart on its own, which
/// is the thing worth seeing first. COMPILE-TIME sorts last because it is
/// the absence of a decision rather than one.
///
/// The init file does NOT get an origin of its own: it replays SET GLOBAL
/// before any client connects, so its values arrive as SQL, indistinguishable
/// from someone typing the statement. Whether a value survives a restart is
/// answered by `persisted`, never by this.
fn origin_rank(origin: &str) -> u8 {
    match origin {
        "SQL" => 0,
        "CONFIG" => 1,
        "COMMAND-LINE" => 2,
        "AUTO" => 3,
        // COMPILE-TIME, and anything a later MariaDB adds.
        _ => 4,
    }
}

/// What a row's control should show: whatever has been typed into it, or the
/// server's current value when nothing has.
fn draft_for(drafts: &[(String, String)], name: &str, current: &str) -> String {
    drafts
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| current.to_string())
}

/// Whether to offer a number box or a text box.
///
/// The server's own VARIABLE_TYPE decides, and this is a presentation choice
/// only - what a value is allowed to be is checked by the backend against the
/// same column, so a text box cannot smuggle anything past it.
fn is_numeric(kind: &str) -> bool {
    kind.contains("INT") || kind == "DOUBLE"
}

const SORTS: [(&str, &str); 4] = [
    ("set", "set by someone"),
    ("method", "method"),
    ("name", "name"),
    ("editable", "editable"),
];

/// One page per tier: what it costs, then every variable in it.
///
/// The rows are filtered on the tier the backend assigned, not re-derived
/// here. Two lists of names in two languages would drift, and the backend's
/// is the one with tests behind it.
#[component]
fn Vars(sub: String) -> Element {
    let report = use_resource(|| async move { api::fetch_variables().await });
    // Copy, and passed to write_variable so a successful write can re-read
    // rather than patch the row it changed.
    let mut show_dormant = use_signal(|| false);
    // "set" is the default because it is the only key that puts the few rows
    // worth finding on top. See SORTS for why "editable" is offered at all
    // when it cannot reorder anything on these four pages.
    let mut sort_by = use_signal(|| "set");
    // Which group headers are open. Every group starts closed, so the page
    // opens as an index of the taxonomy rather than as a wall of rows: with
    // 384 shown on live, whichever group sorted first used to fill the
    // screen on its own and the page read as a list of that one subject.
    // Names rather than indices, for the reason TABS gives - a reordered
    // list cannot then change which group is open.
    let mut expanded = use_signal(Vec::<String>::new);
    // Edits in progress, keyed by name. A row with no entry shows the
    // server's current value, so typing in one row cannot disturb another and
    // nothing has to be seeded for 484 rows that will never be touched.
    let drafts = use_signal(Vec::<(String, String)>::new);
    // (name, why) for a hazardous variable that has been asked for once and
    // not yet confirmed. One at a time: this is a question being put to the
    // reader, and two of them at once is two unanswered questions.
    let pending = use_signal(|| None::<(String, String)>);
    // (name, what happened) for the last write, successful or refused.
    let notice = use_signal(|| None::<(String, String)>);

    // live first, so a bare ?tab=vars lands on the cheapest tier rather than
    // on whichever name happens to sort first.
    let at = nav::VARS_SUBS
        .iter()
        .position(|(n, _)| *n == sub)
        .unwrap_or(0);
    let (name, cost, why) = TIERS[at];

    let state = match &*report.read_unchecked() {
        None => rsx! {
            div { class: "flex items-center gap-2 text-base-content/60",
                span { class: "loading loading-spinner loading-sm" }
                "Asking the server\u{2026}"
            }
        },
        Some(Err(e)) => rsx! {
            FetchError { error: e.clone() }
        },
        Some(Ok(r)) if !r.configured => rsx! {
            div { class: "alert alert-info",
                "No database connection is configured, so the server's variables cannot be "
                "read. What this page takes from GLPI's source is unaffected."
            }
        },
        Some(Ok(r)) => {
            let is_off = |switch: &str| {
                r.variables
                    .iter()
                    .find(|v| v.name == switch)
                    .is_some_and(|v| matches!(v.value.as_str(), "OFF" | "0"))
            };
            let dormant_label = |var: &str| -> Option<&'static str> {
                DORMANT
                    .iter()
                    .find(|(switch, _, prefixes)| {
                        is_off(switch) && prefixes.iter().any(|p| var.starts_with(p))
                    })
                    .map(|(_, label, _)| *label)
            };

            let mut shown: Vec<api::Variable> = Vec::new();
            let mut dormant: Vec<api::Variable> = Vec::new();
            let mut folded: Vec<&'static str> = Vec::new();
            for v in r.variables.iter().filter(|v| v.tier == name) {
                match dormant_label(&v.name) {
                    Some(label) => {
                        folded.push(label);
                        dormant.push(v.clone());
                    }
                    None => shown.push(v.clone()),
                }
            }

            let chosen = shown.iter().filter(|v| v.origin != "COMPILE-TIME").count();
            let writes_enabled = r.writes_enabled;
            let can_persist = writes_enabled && r.persist_path.is_some();
            let ed = Editing {
                writes_enabled,
                can_persist,
                drafts,
                pending,
                notice,
                report,
            };
            // Said once, above the table, rather than as 484 identical notes
            // in a column of disabled controls.
            let settable_here = shown.iter().filter(|v| v.settable).count();
            let unsettable_here = shown.len() - settable_here;

            // Grouped by what each variable is FOR - the backend's opinion,
            // carried on the row so the two never drift. The order of the
            // groups is decided below.
            //
            // Sorted WITHIN each group, so whichever key is chosen the rows
            // stay under the subject they belong to - the grouping answers
            // "where does this live", the key answers "which of these first",
            // and they are different questions. Stable in every case, so the
            // backend's order survives underneath.
            match sort_by() {
                "name" => shown.sort_by(|a, b| a.name.cmp(&b.name)),
                "method" => shown.sort_by_key(|v| origin_rank(&v.origin)),
                "editable" => shown.sort_by_key(|v| v.read_only),
                // "set", the default.
                _ => shown.sort_by_key(|v| v.origin == "COMPILE-TIME"),
            }
            // Said out loud when the key cannot reorder anything, for the same
            // reason the editable one is - but computed rather than asserted.
            // Editable is uniform by construction: the tier IS that cut. This
            // is uniform only as a fact about this server today - install and
            // reports happen to be all COMPILE-TIME, and a future MariaDB
            // that ships one of them as AUTO would make the key work without
            // anything here changing.
            let mut origins: Vec<&str> = shown.iter().map(|v| v.origin.as_str()).collect();
            origins.sort_unstable();
            origins.dedup();
            let one_origin = (origins.len() == 1).then(|| origins[0].to_string());

            let mut groups: Vec<(String, Vec<api::Variable>)> = Vec::new();
            for v in shown {
                let key = if v.group.is_empty() {
                    "Others".to_string()
                } else {
                    v.group.clone()
                };
                match groups.iter_mut().find(|(g, _)| *g == key) {
                    Some((_, rows)) => rows.push(v),
                    None => groups.push((key, vec![v])),
                }
            }
            // Two groups are pinned after the rest, for two different
            // reasons, and everything else is largest first because the
            // reader is scanning for a subject.
            //
            // Others last however big it gets: it is the absence of an
            // answer, not an answer. InnoDB second to last despite being the
            // largest group on three of the four tiers - 106 of live's 384,
            // 28 of restart's 106, 3 of install's 3 - because it is the one
            // group with a panel of its own, which cuts the same 137 seven
            // ways. Leading with it made these pages read as a list of InnoDB
            // settings and buried the eighteen groups that have no panel of
            // their own, which is the opposite of what vargroup says this cut
            // is for. The header links there rather than competing with it.
            fn rank(group: &str) -> u8 {
                match group {
                    "InnoDB" => 1,
                    "Others" => 2,
                    _ => 0,
                }
            }
            groups.sort_by(|a, b| rank(&a.0).cmp(&rank(&b.0)).then(b.1.len().cmp(&a.1.len())));

            // The headline, so the table is evidence rather than the message.
            // The same move the settings tile makes: a page of rows that all
            // say COMPILE-TIME has told you one thing, once.
            let shown_total: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
            let headline = if chosen == 0 {
                format!(
                    "Nothing in this tier was set by anyone: all {shown_total} are this \
                     build's compiled-in defaults."
                )
            } else {
                format!(
                    "{chosen} of {shown_total} were set by something other than the compiled \
                     default."
                )
            };

            let fold_summary = {
                let mut counts: Vec<(&str, usize)> = Vec::new();
                for label in &folded {
                    match counts.iter_mut().find(|(l, _)| l == label) {
                        Some((_, n)) => *n += 1,
                        None => counts.push((label, 1)),
                    }
                }
                counts
                    .iter()
                    .map(|(l, n)| format!("{n} from {l}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            // All editable or none, never a mix - see SORTS. Said out loud
            // beside the key, because a control that cannot change anything
            // is worse than absent unless it explains itself.
            let editable_note = if groups
                .iter()
                .flat_map(|(_, rows)| rows.iter())
                .chain(dormant.iter())
                .all(|v| !v.read_only)
            {
                "all editable"
            } else {
                "none editable"
            };

            // Flattened to owned rows before the tree, because each header's
            // click handler has to own its group name: a handler outlives the
            // render that built it, so it cannot borrow the iteration it came
            // from.
            let open_now = expanded.read().clone();
            let rendered: Vec<(String, Vec<api::Variable>, bool)> = groups
                .into_iter()
                .map(|(group, rows)| {
                    let open = open_now.contains(&group);
                    (group, rows, open)
                })
                .collect();

            rsx! {
                p { class: "mt-3 text-sm", "{headline}" }

                div { class: "mt-2 flex flex-wrap items-baseline gap-x-2 text-sm text-base-content/70",
                    span { "Sort rows within each group by" }
                    for (key , label) in SORTS {
                        button {
                            key: "{key}",
                            class: if sort_by() == key {
                                "btn btn-ghost btn-xs font-semibold text-primary"
                            } else {
                                "btn btn-ghost btn-xs"
                            },
                            onclick: move |_| sort_by.set(key),
                            "{label}"
                        }
                    }
                    if sort_by() == "editable" {
                        span { class: "text-base-content/50",
                            "- every row in this tier is the same here: {editable_note}."
                        }
                    }
                    if sort_by() == "method" {
                        if let Some(only) = one_origin.clone() {
                            span { class: "text-base-content/50",
                                "- every row in this tier came the same way: {only}."
                            }
                        }
                    }
                }

                // Once, above the table. The alternative is 484 rows each
                // carrying the same note beside a control that is not there.
                if settable_here > 0 {
                    div { class: "mt-2 text-sm text-base-content/70",
                        if writes_enabled {
                            "{settable_here} of these can be set on the running server, "
                            "taking effect at once. "
                            if can_persist {
                                "Setting one keeps it: the value goes into the startup file "
                                "as well, so a restart sets it again. Kept drops that line "
                                "again without touching the running server."
                            } else {
                                "Nothing outlives a restart: no startup file is configured, "
                                "so there is nowhere to keep a value."
                            }
                        } else {
                            "{settable_here} of these could be set on the running server, but "
                            "writes are off in this backend. Start it with "
                            span { class: "font-mono text-xs", "GLPI_INV_VARIABLE_WRITES=1" }
                            " to turn the controls on."
                        }
                        if unsettable_here > 0 {
                            " {unsettable_here} exist per connection and have no global value, "
                            "so there is nothing server-wide to set."
                        }
                    }
                }

                if !dormant.is_empty() {
                    div { class: "mt-2 flex flex-wrap items-baseline gap-2 text-sm text-base-content/70",
                        span { "{dormant.len()} are from subsystems this server has switched off - {fold_summary}." }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| {
                                let was = show_dormant();
                                show_dormant.set(!was);
                            },
                            if show_dormant() { "Hide them" } else { "Show them" }
                        }
                    }
                }

                div { class: "mt-3 overflow-x-auto",
                    table { class: "table table-sm",
                        tbody {
                            for (group , rows , open) in rendered {
                                tr { key: "group-{group}",
                                    td { class: "bg-base-200 p-0", colspan: "4",
                                        div { class: "flex flex-wrap items-baseline gap-x-2 px-2",
                                            button {
                                                class: "btn btn-ghost btn-xs font-semibold",
                                                onclick: {
                                                    let name = group.clone();
                                                    move |_| {
                                                        let mut names = expanded.write();
                                                        match names.iter().position(|g| *g == name) {
                                                            Some(at) => {
                                                                names.remove(at);
                                                            }
                                                            None => names.push(name.clone()),
                                                        }
                                                    }
                                                },
                                                if open {
                                                    "\u{25be} "
                                                } else {
                                                    "\u{25b8} "
                                                }
                                                "{group}"
                                                span { class: "ml-1 font-normal text-base-content/60",
                                                    "{rows.len()}"
                                                }
                                            }
                                            // Only on InnoDB, and a pointer
                                            // rather than a second copy: the
                                            // panel it names cuts these 137
                                            // seven ways and this page is not
                                            // the place to repeat that.
                                            if group == "InnoDB" {
                                                span { class: "text-xs text-base-content/60",
                                                    "its own panel is on "
                                                    Link {
                                                        class: "link link-primary",
                                                        to: Route::Db {
                                                            tab: "maria".to_string(),
                                                            sub: "general".to_string(),
                                                        },
                                                        "Maria \u{00b7} general"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if open {
                                    for v in rows.iter() {
                                        {variable_rows(v, false, false, ed)}
                                    }
                                }
                            }
                            if show_dormant() {
                                for v in dormant.iter() {
                                    {variable_rows(v, true, false, ed)}
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        Panel {
            title: name.to_string(),
            subtitle: cost.to_string(),
            info: "One of four tiers cut by what changing a variable costs. The index, with \
                   all four and the reasoning behind the cut, is the panel of the same name \
                   on Maria \u{b7} general. Three columns: the variable, what it is set to, \
                   and the server's own word for where that value came from. COMPILE-TIME \
                   means nobody set it; SQL means someone ran SET GLOBAL, so the running \
                   server and the next restart disagree. Nothing here is compared against \
                   anything - which of it matters depends on the question being asked."
                .to_string(),

            // The only way back, and that is why it is here rather than
            // implied. These four tiers have no sidebar entry on purpose -
            // Maria - general already carries the panel they belong to - so
            // nothing in the menu is lit while this page is open, and a
            // reader who arrived by clicking a tier would otherwise have to
            // guess which tab to return to.
            actions: rsx! {
                Link {
                    class: "link link-primary text-sm",
                    to: Route::Db {
                        tab: "maria".to_string(),
                        sub: "general".to_string(),
                    },
                    "\u{2190} all four tiers"
                }
            },

            p { class: "text-sm text-base-content/70", "{why}" }
            {state}
        }
    }
}

/// Every InnoDB setting the server has, in seven groups.
///
/// Reference rather than telemetry, and the largest thing on the page: 137
/// rows on this server. It is here because InnoDB is not one engine choice
/// among several for GLPI - it is named in all 447 of the installer's CREATE
/// TABLE statements, so its settings are the terms GLPI's storage is
/// actually on.
///
/// A strip rather than seven stacked tables, which stacked are one page
/// nobody reaches the end of. Which group is open is local view state, like
/// the RSQL strip: the sidebar does not take this level, because a group of
/// settings is not a destination.
///
/// The grouping is this app's and the panel says so. MariaDB publishes none -
/// SHOW VARIABLES LIKE 'innodb%' is 137 alphabetical rows and nothing else -
/// so every placement is a reading, and the rules are in glpi::refused where
/// they are tested.
#[component]
fn Innodb() -> Element {
    // Mutable because a write re-reads it: one setting can move another, so
    // the panel refetches the lot rather than patching the row that changed.
    let mut report = use_resource(|| async move { api::fetch_innodb().await });
    let mut at = use_signal(|| 0usize);

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Asking the server\u{2026}"
                }
            },
            Some(Err(e)) => rsx! {
                FetchError { error: e.clone() }
            },
            Some(Ok(r)) if !r.configured => rsx! {
                div { class: "alert alert-info",
                    "No database connection is configured, so the engine's own variables "
                    "cannot be read. What this page takes from GLPI's source is unaffected."
                }
            },
            Some(Ok(r)) => {
                let groups = r.groups.clone();
                let total: usize = groups.iter().map(|g| g.variables.len()).sum();
                // One past Startup, which is one past the last group. Both
                // live outside the group indices so that a strip whose tabs
                // changed with the server could not silently select one of
                // them.
                let top_index = groups.len() + 1;
                let on_top = at() == top_index;
                let startup_count = groups
                    .iter()
                    .flat_map(|g| g.variables.iter())
                    .filter(|v| v.read_only)
                    .count();
                // One past the last group is the Startup tab, so the clamp
                // that keeps a stale index inside the strip has to allow it.
                let on_startup = at() == groups.len();
                let current = groups.get(at().min(groups.len().saturating_sub(1))).cloned();
                rsx! {
                    Panel {
                        title: "InnoDB".to_string(),
                        subtitle: match (&r.persist_path, r.writes_enabled) {
                            (_, false) => format!("{total} variables, in {} groups - read-only", groups.len()),
                            (Some(p), true) => format!("{total} variables, in {} groups - kept ones are replayed from {p}", groups.len()),
                            (None, true) => format!(
                                "{total} variables, in {} groups - changes last until the server restarts",
                                groups.len(),
                            ),
                        },
                        info: "Every innodb_% variable the server reports, read live from the \
                               running instance. The seven groups are this app's reading \
                               and not MariaDB's: the server publishes no taxonomy of its \
                               own, SHOW VARIABLES LIKE 'innodb%' being alphabetical rows \
                               and nothing more. Others is \
                               the honest remainder - instrumentation, recovery, the \
                               full-text family, the internal spin tuning - and it is also \
                               where a variable a future MariaDB adds will appear, \
                               uncategorised rather than dropped. Nothing here is compared \
                               against anything: this is what the engine is set to, and \
                               which of it matters depends on the question being asked. \
                               WHAT A CHANGE LASTS. Nothing in MariaDB persists SET GLOBAL - \
                               there is no SET PERSIST, which is MySQL 8, and no \
                               mysqld-auto.cnf - so on its own a change here holds until the \
                               server restarts and my.cnf wins. Keep writes it instead into \
                               a file MariaDB replays with --init-file before any client \
                               connects, which is why it covers these variables and not the \
                               31 taken at startup: that file is SQL, and SQL can only set \
                               what SET GLOBAL can set. One thing to know about it, measured \
                               on 11.4.12 rather than assumed: a line that fails does not \
                               stop the server, but the rest of the file is skipped, so one \
                               variable a later MariaDB no longer understands takes every \
                               line below it with it. That is what the warning beside a row \
                               means - the file asks for one thing and the server has \
                               another."
                            .to_string(),

                        div { class: "mb-3 flex items-center gap-2",

                        div {
                            role: "tablist",
                            class: "tabs tabs-sm tabs-lift flex-nowrap overflow-x-auto",

                            // First, before the seven groups, because it is
                            // where to start: a reader who does not already
                            // know which of 137 settings decide anything is
                            // otherwise left to read all seven groups to
                            // find out. The groups answer "where is this
                            // setting"; this one answers "which should I
                            // look at".
                            button {
                                role: "tab",
                                class: if on_top { "tab tab-active" } else { "tab" },
                                onclick: move |_| at.set(top_index),
                                "Top 8"
                            }

                            for (i , g) in groups.iter().enumerate() {
                                button {
                                    key: "{g.category}",
                                    role: "tab",
                                    class: if i == at() { "tab tab-active" } else { "tab" },
                                    onclick: move |_| at.set(i),
                                    "{g.category}"
                                    span { class: "badge badge-ghost badge-xs ml-2",
                                        "{g.variables.len()}"
                                    }
                                }
                            }

                            // Last, after the seven, because it is not an
                            // eighth category - it is the same settings cut
                            // the other way, by whether a running server
                            // will take a new value at all. Each of these
                            // also appears under its own group above, with
                            // its value and no control; here they are
                            // together, which is the question "why does
                            // that row have no dropdown" answered in one
                            // place.
                            button {
                                role: "tab",
                                class: if at() == groups.len() { "tab tab-active" } else { "tab" },
                                onclick: move |_| at.set(groups.len()),
                                "Startup"
                                span { class: "badge badge-ghost badge-xs ml-2", "{startup_count}" }
                            }
                        }

                        // Beside the strip rather than in the tab: a button
                        // inside a tab is a second thing to hit in the place
                        // a reader is aiming to switch tabs, and nesting one
                        // inside the other is not markup a browser will keep.
                        // Out here it still reads as belonging to the tab it
                        // follows, and it is reachable before the tab is
                        // opened - which is when "what is this" gets asked.
                        StartupInfo {}

                        }

                        if on_top {
                            TopSettings {
                                groups: groups.clone(),
                                writes_enabled: r.writes_enabled,
                                can_persist: r.persist_path.is_some(),
                                on_set: move |_| report.restart(),
                            }
                        } else if on_startup {
                            Startup { groups: groups.clone() }
                        } else if let Some(g) = current {
                            if g.variables.is_empty() {
                                // Not an error, and not hidden: a group with
                                // nothing in it is a fact about this server,
                                // and a strip whose tabs came and went with
                                // the server would be worse.
                                p { class: "text-sm text-base-content/60",
                                    "This server reports no variables in this group."
                                }
                            } else {
                                div { class: "overflow-x-auto",
                                    table { class: "table table-sm",
                                        tbody {
                                            for v in g.variables.iter() {
                                                tr { key: "{v.name}",
                                                    td { class: "font-mono text-xs whitespace-nowrap",
                                                        "{v.name}"
                                                    }
                                                    td {
                                                        SettingValue {
                                                            setting: v.clone(),
                                                            writes_enabled: r.writes_enabled,
                                                            can_persist: r.persist_path.is_some(),
                                                            on_set: move |_| report.restart(),
                                                        }
                                                    }
                                                    // A column of its own, fixed width and
                                                    // last, so 137 buttons line up down one
                                                    // edge instead of tracking the name
                                                    // beside them - innodb_ft_server_stopword_table
                                                    // and innodb_doublewrite differ by 26
                                                    // characters, and a button that moves with
                                                    // the text is a button the eye has to hunt
                                                    // for on every row.
                                                    td { class: "w-8 text-right align-top",
                                                        SettingInfo {
                                                            name: v.name.clone(),
                                                            help: v.help.clone(),
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The settings worth looking at first, and what each one decides.
///
/// THIS LIST IS THIS APP'S, not the server's, and it is the only thing on
/// this panel that is a judgement rather than a reading. MariaDB ranks
/// nothing: `SHOW VARIABLES` is 137 alphabetical rows and the help text says
/// what each one does, never which matter. So the order below is argued for
/// in the panel's own explanation rather than presented as fact, and every
/// value beside it is still read live.
///
/// Why these eight. The first three are the classic throughput levers - how
/// much stays in memory, how often a commit reaches disk, and how much redo
/// can be outstanding before a checkpoint stalls writes. The next three are
/// about the storage underneath: whether pages are cached twice, and the two
/// that between them decide how fast the background flusher may go - a floor
/// without its ceiling settles nothing, which is why they are a pair rather
/// than one entry. The seventh trades a write for crash safety. The last
/// cannot be changed at all after the data directory exists, which is why it
/// is here: a setting nobody can fix later belongs where it is seen early.
///
/// IT WAS SEVEN UNTIL 2026-11-15, and the eighth was left off for a bad
/// reason worth recording: `innodb_io_capacity_max` mattered, but including
/// it would have made the count disagree with the label. That is the label
/// deciding the content. The fix is the one that was available all along -
/// rename the label - and the rule it leaves behind is that a name counting
/// things follows the list, never the other way round.
const TOP_SETTINGS: [(&str, &str); 8] = [
    (
        "innodb_buffer_pool_size",
        "How much data and index stays in RAM. The single biggest lever on read          performance, and the one that starts to matter the moment the working set          outgrows it.",
    ),
    (
        "innodb_flush_log_at_trx_commit",
        "Whether a commit reaches disk before it is acknowledged. 1 is durable and          slowest; 2 and 0 trade a window of committed-but-lost writes for throughput.",
    ),
    (
        "innodb_log_file_size",
        "How much redo can be outstanding. Too small under write load means constant          checkpoint flushing. Resizable while the server runs, on this version.",
    ),
    (
        "innodb_flush_method",
        "Whether pages are cached twice - once by InnoDB, once by the operating system.          O_DIRECT skips the second copy.",
    ),
    (
        "innodb_io_capacity",
        "How fast the background flusher may go, in IOPS. The default assumes a spinning          disk and is far below what an SSD will take.",
    ),
    (
        "innodb_io_capacity_max",
        "The ceiling the flusher may reach when it has fallen behind. Raising io_capacity \
         without it moves the floor and leaves the ceiling where it was, so the two are \
         set together or not at all.",
    ),
    (
        "innodb_doublewrite",
        "Protection against half-written pages after a power loss. Turning it off is          faster and risks a corrupt page nothing will repair.",
    ),
    (
        "innodb_page_size",
        "Row and index density. Fixed when the data directory was created: changing it          later means dumping, re-initialising and restoring, which gets more expensive          with every machine that reports.",
    ),
];

/// The eight, in that order, with the same controls the groups give them.
///
/// Rows, not prose: a reader who came here to see whether the buffer pool is
/// big enough wants this server's number and a way to change it, not an
/// article. The sentence beside each says why it is on the list, which is
/// the only part this app is asserting.
///
/// A name the server does not report is skipped rather than shown empty -
/// the list is written here and the settings come from the server, so the
/// two can disagree after an upgrade, and a row with no value would be this
/// app claiming a setting exists because it used to.
#[component]
fn TopSettings(
    groups: Vec<api::InnodbGroup>,
    writes_enabled: bool,
    can_persist: bool,
    on_set: EventHandler<()>,
) -> Element {
    let mut rows: Vec<(api::InnodbSetting, &'static str)> = Vec::new();
    for (name, why) in TOP_SETTINGS {
        if let Some(v) = groups
            .iter()
            .flat_map(|g| g.variables.iter())
            .find(|v| v.name == name)
        {
            rows.push((v.clone(), why));
        }
    }

    rsx! {
        div { class: "mb-2 flex flex-wrap items-center gap-x-3",
            div { class: "flex items-center gap-2",
                h3 { class: "text-sm font-semibold tracking-tight", "Top 8" }
                TopInfo {}
            }
            span { class: "text-xs text-base-content/60",
                "This app's ranking, not the server's. Every value is read live."
            }
        }

        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                tbody {
                    for (v , why) in rows.iter() {
                        tr { key: "{v.name}",
                            td { class: "font-mono text-xs whitespace-nowrap align-top",
                                "{v.name}"
                            }
                            td { class: "align-top",
                                SettingValue {
                                    setting: v.clone(),
                                    writes_enabled,
                                    can_persist,
                                    on_set: move |_| on_set.call(()),
                                }
                            }
                            td { class: "max-w-md text-xs text-base-content/70", "{why}" }
                            td { class: "w-8 text-right align-top",
                                SettingInfo { name: v.name.clone(), help: v.help.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Why these eight and not eight others.
#[component]
fn TopInfo() -> Element {
    rsx! {
        InfoButton {
            title: "Top 8".to_string(),
            body: "The only list on this panel that is an opinion. MariaDB ranks nothing - \
                   its help text says what each variable does and never which of them \
                   matter - so these eight are chosen here, and the sentence beside each \
                   row is the argument for it rather than something read off the server. \
                   What they have in common is that each one decides something no other \
                   variable can compensate for: how much stays in memory, how often a \
                   commit reaches disk, how much redo may be outstanding, whether pages \
                   are cached twice, the floor and the ceiling on how fast the flusher \
                   may go, whether a half-written page is survivable, and how densely \
                   rows are packed. It was seven until the io_capacity ceiling was left \
                   off to keep the count tidy, which is the label deciding the list; the \
                   list decides the label. Two of them are \
                   taken at startup and appear under Startup as well - and the last, the \
                   page size, is fixed when the data directory is created, so it is the \
                   one whose cost to change rises with every machine that reports. \
                   Nothing here says any of them is currently wrong on this server: the \
                   values are read live beside each row, and on a database that fits \
                   entirely in the buffer pool none of these will be measurable. They are \
                   the ones to look at first when that stops being true, which is a \
                   question of how large the install grows rather than of how it is \
                   configured today."
                .to_string(),
        }
    }
}

/// What the Startup tab is, beside the tab itself.
///
/// Its own component because it is rendered from the strip, which is in a
/// different part of the tree from the list it describes - and because the
/// explanation is the longest thing on this page, which would otherwise sit
/// in the middle of the markup that draws eight tab buttons.
#[component]
fn StartupInfo() -> Element {
    rsx! {
        InfoButton {
            title: "Variables taken at startup".to_string(),
                body: "These are read once, as the server starts, before any client can \
                       connect and therefore before any SQL exists to send - so neither the \
                       controls above nor the file this app keeps them in can touch them. \
                       Only a configuration file the server reads at startup can, and on \
                       this machine there is exactly one: /etc/my.cnf, a read-only Nix store \
                       path. Three things close every other route, each measured on this \
                       install rather than assumed. The unit starts the server with \
                       --defaults-file, and that makes --defaults-extra-file be ignored \
                       outright - with both given, the server reports only bind-address, \
                       datadir and port, where the extra file alone is read in full - so \
                       there is no second file to drop beside the first. /etc/systemd/system \
                       is a symlink into /etc/static and read-only, so there is no unit \
                       drop-in either, and the start script's only variable parts are two \
                       Galera environment variables that come from the unit itself. And \
                       restarting the database is a system service, which is root. So no \
                       script, and nothing this app runs as, can restart MariaDB with one of \
                       these changed - which is the whole of what cannot be automated here, \
                       and it is about this operating system's model rather than about \
                       MariaDB. What does work is the declarative route: set it under \
                       services.mysql.settings.mysqld in the NixOS configuration and \
                       rebuild. That rewrites my.cnf and restarts the database into the new \
                       value by itself, because the unit's restart trigger is the my.cnf \
                       store path - so it is one root action, not a config edit plus a \
                       restart someone has to remember."
                        .to_string(),
        }
    }
}

/// The settings SQL cannot reach, and what changing one actually takes.
///
/// The counterpart to the panel above: those are the settings a running
/// server will take a new value for, these are the ones it reads once before
/// any SQL exists to send. The page would be incoherent without them - 106
/// rows with controls and 31 without, and no statement anywhere about why -
/// so the division is named rather than left to be inferred from which rows
/// happen to have a dropdown.
///
/// Built from the same fetch as the panel: this is a filter over rows
/// already on screen, not a second request, and the count is whatever the
/// server reported rather than a number written here. The group each one
/// falls in is carried through because it is the only grouping this app has
/// - the same seven the strip above uses - and a reader looking for one of
/// these will look for it where the panel put it.
#[component]
fn Startup(groups: Vec<api::InnodbGroup>) -> Element {
    // Group order is the panel's order, which is INNODB_GROUPS' order, so a
    // reader scanning this list meets the categories in the sequence the
    // strip above showed them.
    let mut rows: Vec<(String, api::InnodbSetting)> = Vec::new();
    for g in groups.iter() {
        for v in g.variables.iter().filter(|v| v.read_only) {
            rows.push((g.category.clone(), v.clone()));
        }
    }
    let categories = rows
        .iter()
        .map(|(c, _)| c.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let total: usize = groups.iter().map(|g| g.variables.len()).sum();

    rsx! {
        div { class: "mb-2 flex flex-wrap items-center gap-x-3",
            span { class: "text-xs text-base-content/60",
                "{rows.len()} of {total} variables, across {categories} of the 7 groups"
            }
        }

        div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    tbody {
                        for (category , v) in rows.iter() {
                            tr { key: "{v.name}",
                                td { class: "font-mono text-xs whitespace-nowrap", "{v.name}" }
                                td { class: "font-mono text-xs break-all", "{v.value}" }
                                // The group it would be under in the strip
                                // above, so a setting found here can be
                                // found there - this app has one grouping
                                // and both lists use it.
                                td { class: "text-xs whitespace-nowrap text-base-content/60",
                                    "{category}"
                                }
                            td { class: "w-8 text-right align-top",
                                SettingInfo { name: v.name.clone(), help: v.help.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One setting's value: a control where the server will take a change, the
/// value as text where it will not.
///
/// Which it is comes from the server, never from a list here: READ_ONLY says
/// whether there is a control, VARIABLE_TYPE and ENUM_VALUE_LIST say which
/// one, and the range and block size furnish it. 106 of this server's 137
/// settings are writable; the other 31 are taken at startup and render as
/// they always did.
///
/// Every control is applied explicitly rather than on change. A select that
/// writes as it is opened is a server setting changed by scrolling, and the
/// one request this app makes that is not a read should take a decision.
///
/// What is shown after a write is what the server reports, not what was
/// typed: ask the buffer pool for 100000000 and it is 100663296, because the
/// value is rounded to the setting's block size.
#[component]
fn SettingValue(
    setting: api::InnodbSetting,
    writes_enabled: bool,
    /// Whether a startup file is configured. Without one there is nothing to
    /// write, so the row offers no Keep button and the panel's header says
    /// why - rather than a button that writes a file nothing reads.
    can_persist: bool,
    /// Fires after a write lands, so the panel can re-read every value - one
    /// setting can move another, and a table that only updated the row that
    /// was touched would be quietly wrong about the rest.
    on_set: EventHandler<()>,
) -> Element {
    let api::InnodbSetting {
        name,
        value,
        read_only,
        options,
        kind,
        min,
        max,
        block,
        persisted,
        ..
    } = setting.clone();

    let mut draft = use_signal(|| value.clone());
    let mut busy = use_signal(|| false);
    let mut failure = use_signal(|| None::<String>);
    let mut settled = use_signal(|| None::<String>);

    // The row re-renders with a fresh value after every refetch, so the draft
    // has to follow the server rather than keep whatever was in the box, or
    // a rounded value would be overwritten by the number that was asked for.
    use_effect(use_reactive(&value, move |v| {
        draft.set(v);
        settled.set(None);
    }));

    let dirty = draft() != value;
    let numeric = options.is_empty() && (kind.contains("INT") || kind == "DOUBLE");

    // The whole point of showing the file beside the server: MariaDB stops
    // replaying the startup file at its first failing line, so a setting can
    // be written there and silently not applied. The server's value is the
    // truth; this says what the next restart intends.
    let drifted = persisted.as_ref().is_some_and(|p| *p != value);

    // Cloned per closure rather than captured: three of them want the name,
    // and the component owns exactly one.
    let apply_name = name.clone();
    let apply = move |_| {
        let (n, v) = (apply_name.clone(), draft());
        busy.set(true);
        failure.set(None);
        spawn(async move {
            match api::set_innodb(&n, &v).await {
                Ok(change) => {
                    // Said out loud only when it differs from the request:
                    // the server rounding 100000000 to 100663296 is the
                    // kind of thing a reader must not have to notice.
                    if change.value != change.requested {
                        settled.set(Some(change.value.clone()));
                    }
                    on_set.call(());
                }
                Err(e) => failure.set(Some(e)),
            }
            busy.set(false);
        });
    };

    // Writes the value the server actually has, not the draft: Keep only
    // appears when the two already agree, so there is one value to mean.
    let persist_keep = {
        let (n, v) = (name.clone(), value.clone());
        move |_| {
            let (n, v) = (n.clone(), v.clone());
            busy.set(true);
            failure.set(None);
            spawn(async move {
                if let Err(e) = api::persist_innodb(&n, Some(&v)).await {
                    failure.set(Some(e));
                }
                busy.set(false);
                on_set.call(());
            });
        }
    };

    let persist_drop = {
        let n = name.clone();
        move |_| {
            let n = n.clone();
            busy.set(true);
            failure.set(None);
            spawn(async move {
                if let Err(e) = api::persist_innodb(&n, None).await {
                    failure.set(Some(e));
                }
                busy.set(false);
                on_set.call(());
            });
        }
    };

    if read_only {
        return rsx! {
            div { class: "flex items-center gap-2",
                span { class: "font-mono text-xs break-all", "{value}" }
                // Said, not implied by an absent control: a row with nothing
                // beside it reads as an oversight.
                span { class: "badge badge-ghost badge-xs whitespace-nowrap", "at startup" }
            }
        };
    }

    rsx! {
        div { class: "flex flex-col gap-1",
            div { class: "flex flex-wrap items-center gap-2",
                if !options.is_empty() {
                    select {
                        class: "select select-bordered select-xs w-40 font-mono",
                        disabled: !writes_enabled || busy(),
                        value: "{draft}",
                        onchange: move |e| draft.set(e.value()),
                        for o in options.iter() {
                            option { key: "{o}", value: "{o}", "{o}" }
                        }
                    }
                } else {
                    input {
                        class: "input input-bordered input-xs w-56 font-mono",
                        r#type: if numeric { "number" } else { "text" },
                        min: min.clone().unwrap_or_default(),
                        max: max.clone().unwrap_or_default(),
                        step: block.clone().unwrap_or_default(),
                        disabled: !writes_enabled || busy(),
                        value: "{draft}",
                        oninput: move |e| draft.set(e.value()),
                    }
                }

                // Only once the value differs from the server's, so 106 live
                // buttons do not sit on a page where nothing has been asked
                // for yet.
                if dirty && writes_enabled {
                    button {
                        class: "btn btn-xs",
                        disabled: busy(),
                        onclick: apply,
                        if busy() {
                            span { class: "loading loading-spinner loading-xs" }
                        }
                        "Apply"
                    }
                    button {
                        class: "btn btn-ghost btn-xs",
                        disabled: busy(),
                        onclick: move |_| {
                            draft.set(value.clone());
                            failure.set(None);
                        },
                        "Reset"
                    }
                }
            }

            // Keeping is a second decision, taken after a value is the
            // one you want: the live change is instant, this is about what
            // the server comes back as.
            if can_persist && writes_enabled && !dirty {
                div { class: "flex flex-wrap items-center gap-2",
                    if persisted.is_some() {
                        span {
                            class: if drifted { "badge badge-warning badge-xs whitespace-nowrap" } else { "badge badge-ghost badge-xs whitespace-nowrap" },
                            "kept at startup"
                        }
                        button {
                            class: "btn btn-ghost btn-xs",
                            disabled: busy(),
                            onclick: persist_drop,
                            "Stop keeping"
                        }
                    } else {
                        button {
                            class: "btn btn-ghost btn-xs",
                            disabled: busy(),
                            onclick: persist_keep,
                            "Keep"
                        }
                    }
                }
            }

            if drifted {
                p { class: "text-xs text-warning break-words",
                    "The startup file asks for "
                    span { class: "font-mono", "{persisted.clone().unwrap_or_default()}" }
                    ", which is not what the server has. MariaDB stops replaying that file "
                    "at its first failing line, so a line above this one may have been "
                    "rejected."
                }
            }

            if let Some(v) = settled() {
                p { class: "text-xs text-base-content/60",
                    "The server rounded it to "
                    span { class: "font-mono", "{v}" }
                    "."
                }
            }
            if let Some(e) = failure() {
                p { class: "text-xs text-error break-words", "{e}" }
            }
        }
    }
}

/// The info button on one InnoDB setting.
///
/// Its own component because the absent case needs saying, not hiding. Two of
/// this server's 137 variables have no description in `mariadbd --help
/// --verbose`, and a row with no button there would read as "nothing to
/// explain" rather than "the server does not say" - the same distinction
/// between absent and unavailable the rest of this app insists on.
///
/// The body is the server's own sentence verbatim; see glpi::innodb_help for
/// why none of it is written here.
#[component]
fn SettingInfo(name: String, help: Option<String>) -> Element {
    rsx! {
        InfoButton {
            title: name.clone(),
            body: match &help {
                Some(h) => h.clone(),
                None => format!(
                    "The server does not describe this one. {name} is reported by SHOW \
                     VARIABLES but has no entry in `mariadbd --help --verbose`, which is \
                     where every other description on this panel comes from - so there is \
                     nothing to quote, and nothing is invented in its place. Two of this \
                     server's 137 InnoDB variables are in that position."
                ),
            },
        }
    }
}

/// MariaDB's storage choices, as GLPI's installer makes them.
///
/// Both panels are read out of install/mysql/glpi-12.0.0-empty.sql rather than
/// the live server. That was once a necessity - the account reached two tables
/// - and since 2026-11-13 it is a preference: the file is version-pinned and
/// re-checkable by anyone without a database at all.
#[component]
fn MariaStorage() -> Element {
    rsx! {
        // A named link rather than the icon, for the same reason the page-level
        // one above is: this answers what the whole tab is for, and a reader who
        // does not know that has nothing to hover yet. Body only, no sections -
        // two short tabs here would be a click that reveals less than a
        // paragraph.
        div { class: "mb-2",
            InfoButton {
                label: "What is this".to_string(),
                title: "What the storage tab is".to_string(),
                body: "How MariaDB actually keeps GLPI's data, as opposed to which databases \
                       exist beside it - that is the other tab. Two things decide it, and \
                       both are settled by the installer rather than by anything \
                       configurable later: the type each column really has, and the four \
                       table-level choices repeated identically across all 447 tables \
                       (InnoDB, utf8mb4, utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC). It matters \
                       in two places a reader meets by accident. A column GLPI calls json is \
                       longtext with a CHECK constraint, because MariaDB has no JSON type, so \
                       a schema diff against another server reports a difference that is not \
                       one - GLPI's own checker rewrites it before comparing. And \
                       utf8mb4_unicode_ci folds case, so two values differing only in \
                       capitalisation collide on a UNIQUE key, which is how a custom asset \
                       definition can refuse a system name that looks unused. Everything here \
                       was read in install/mysql/glpi-12.0.0-empty.sql and in GLPI's source, \
                       so it can be re-checked without a database account at all, by \
                       anyone, on any install - which is why it stays the source even now \
                       that this app's account could query the live server instead."
                    .to_string(),
            }
        }

        div { class: "grid gap-4 items-start lg:grid-cols-2",
            Panel {
                title: "json is longtext with a constraint".to_string(),
                subtitle: "MariaDB has no JSON storage type".to_string(),
                info: "Read in the install schema and in GLPI's own schema checker. \
                       DatabaseSchemaIntegrityChecker.php line 470 carries a regex that \
                       rewrites longtext ... CHECK (json_valid(col)) back to json before \
                       comparing a live table against the expected schema - which is GLPI \
                       stating the same thing from the other direction. The two undeclared \
                       columns are glpi_forms_questions.default_value and .extra_data, both \
                       text with a comment beginning JSON."
                    .to_string(),

                p { class: "mb-3 text-sm",
                    "A "
                    code { class: "text-xs", "json" }
                    " column is a "
                    code { class: "text-xs", "longtext" }
                    " plus a "
                    code { class: "text-xs", "CHECK (json_valid(col))" }
                    ". That matters here because it decides what a direct read can and \
                     cannot do."
                }

                ul { class: "ml-5 list-disc space-y-2 text-sm",
                    li {
                        span { class: "font-medium", "SHOW CREATE TABLE will not say json. " }
                        "It reports the longtext and the constraint, so a schema diff that \
                         expects the word json reports drift that is not there. GLPI undoes \
                         this with a regex rather than trusting the type name."
                    }
                    li {
                        span { class: "font-medium", "No key, no constraint, no cascade. " }
                        "Nothing inside the document can be indexed or referenced, so a value \
                         held there is invisible to a query that does not know to look inside \
                         it, and nothing in the database protects it."
                    }
                    li {
                        span { class: "font-medium", "Declared and actual differ. " }
                        "21 columns are declared json in the 12.0.0 schema; 23 hold JSON. The \
                         other two are text with a comment saying so, which means counting by \
                         type undercounts, and those two get no json_valid check at all."
                    }
                }
            }

            Panel {
                title: "447 tables, one set of choices".to_string(),
                subtitle: "InnoDB, utf8mb4, utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC".to_string(),
                info: "Counted in install/mysql/glpi-12.0.0-empty.sql: 447 CREATE TABLE \
                       statements, all four settings identical in every one. Worth stating \
                       because a grep for the literal ENGINE=InnoDB returns 445 - \
                       glpi_validationsteps and glpi_itils_validationsteps write the clause \
                       with spaces around the equals signs, so a pattern count invites the \
                       conclusion that two tables are unpinned. They are not."
                    .to_string(),

                p { class: "mb-3 text-sm",
                    "There is no per-table variation to account for. Every table declares the \
                     same engine, charset, collation and row format."
                }

                ul { class: "ml-5 list-disc space-y-2 text-sm",
                    li {
                        span { class: "font-medium", "Nothing inherits a server default. " }
                        "Each CREATE TABLE pins all four, so the schema does not change meaning \
                         with the server's own defaults - which is what makes reading the file \
                         a substitute for querying the database."
                    }
                    li {
                        span { class: "font-medium", "utf8mb4_unicode_ci is case-insensitive. " }
                        "Comparisons and unique keys on text fold case, so two rows differing \
                         only in capitalisation collide on a UNIQUE key. The system_name \
                         uniqueness on asset definitions is one such key."
                    }
                    li {
                        span { class: "font-medium", "Count by pattern and you get 445. " }
                        "The two outliers are a whitespace style in the DDL, not a different \
                         engine. Named here because the wrong number is the easy one to reach."
                    }
                }
            }
        }
    }
}

/// The databases on this MariaDB server, and what each is for.
///
/// Every claim here was measured on this install rather than taken from
/// MariaDB's manual: the row counts, the disabled performance_schema, the
/// privilege errors, and the grants that decide who sees what.
#[component]
fn Databases() -> Element {
    rsx! {
        // Named link, not the icon, and above the list rather than beside one
        // entry: what it heads off - reading seven databases as seven things
        // GLPI uses - is a conclusion a reader reaches from the list itself,
        // before there is anything to hover.
        div { class: "mb-2",
            InfoButton {
                label: "What is this".to_string(),
                title: "What these seven are".to_string(),
                body: "Seven databases, and only two of them have anything to do with GLPI. \
                       Four are the server's own bookkeeping and exist on every MariaDB \
                       install whether GLPI is there or not: mysql, which holds the accounts, \
                       grants and plugins; information_schema, a description of everything \
                       else that is generated per query rather than stored; \
                       performance_schema, the instrumentation, switched off on this server; \
                       and sys, a layer of readable views over that instrumentation. test is \
                       an empty historical leftover that MariaDB has shipped for decades. \
                       That leaves glpi, the live database, and test_glpi_schema, the schema \
                       copy made for reading in this app. GLPI's own data never crosses \
                       between them: no GLPI table joins across schemas, so nothing in the \
                       other six changes what any GLPI page shows. The one crossing is \
                       metadata rather than data - DBmysql.php and the schema checker read \
                       information_schema to list GLPI's own tables, and telemetry reads it \
                       for the database size - and a lookup there is not a join."
                    .to_string(),
            }
        }

        div { class: "mb-4 text-sm",
            span { class: "text-base-content/60", "Seven on this server: " }
            a { class: "link link-primary", href: "#db-glpi", "glpi" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-schema-copy", "test_glpi_schema" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-test", "test" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-mysql", "mysql" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-information-schema", "information_schema" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-performance-schema", "performance_schema" }
            " \u{b7} "
            a { class: "link link-primary", href: "#db-sys", "sys" }
            span { class: "text-base-content/60",
                " - how many of them you see depends on the account you connect with."
            }
        }

        div { class: "grid gap-4 items-start lg:grid-cols-2",
            div { id: "db-glpi",
                Panel {
                    title: "glpi".to_string(),
                    subtitle: "The live one".to_string(),
                    info: "Created by the NixOS module - services.mysql.ensureDatabases = \
                           [ \"glpi\" ] in /etc/nixos/glpi.nix - and filled by GLPI's own \
                           installer. Row counts below measured on this install."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "Everything GLPI knows: assets, agents, tickets, profiles, rights, \
                         logs. 447 tables, the same set listed in the install schema. This is \
                         production data for the GLPI running on this machine - nothing here \
                         is a copy, and a dropped table is a restore from backup, not an undo."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "This app reads one table of it. " }
                            code { class: "text-xs", "glpi_refusedequipments" }
                            ", in a single query. Its account was once limited to that table \
                             and "
                            code { class: "text-xs", "glpi_logs" }
                            ", and since 2026-11-13 holds every privilege on the server - so \
                             a schema browser pointed at it now shows all 447 tables, and \
                             what this app reads is decided by its code rather than by the \
                             database."
                        }
                        li {
                            span { class: "font-medium", "What is actually in those two. " }
                            "Measured 2026-11-14: glpi_logs holds 1451 rows, and \
                             glpi_refusedequipments holds none - no inventory submission has \
                             ever been refused on this install, which is a finding rather \
                             than a gap."
                        }
                    }
                }
            }

            div { id: "db-schema-copy",
                Panel {
                    title: "test_glpi_schema".to_string(),
                    subtitle: "A readable copy of the schema, with no data in it".to_string(),
                    info: "Imported from install/mysql/glpi-12.0.0-empty.sql, which carries \
                           447 DROP/CREATE TABLE pairs and zero INSERTs. No root was needed: \
                           see the test entry."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "Every GLPI table with every column, index and type, and not one row. \
                         It exists because the account above cannot browse the real schema, \
                         while the question \u{201c}what columns does this table have\u{201d} comes up \
                         constantly. Here it can be answered in a GUI instead of by grepping \
                         a 400 KB SQL file."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "It is disposable. " }
                            "Nothing points at it and nothing writes to it. "
                            code { class: "text-xs", "DROP DATABASE test_glpi_schema" }
                            " costs nothing and it can be re-imported in seconds."
                        }
                        li {
                            span { class: "font-medium", "It is a schema, not a mirror. " }
                            "It reflects the shipped 12.0.0 schema file, not the live database. \
                             Plugins, migrations or hand edits on the real install would not \
                             appear here."
                        }
                    }
                }
            }

            div { id: "db-test",
                Panel {
                    title: "test".to_string(),
                    subtitle: "MariaDB's own scratch database, and it is not inert".to_string(),
                    info: "SHOW GRANTS on this install lists two PUBLIC grants: full rights \
                           on `test`.* and on `test\\_%`.* for every account."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "Shipped empty by MariaDB and usually ignored, but on this server it \
                         is the reason an unprivileged account can do anything at all: the \
                         default PUBLIC grants hand every authenticated account full rights \
                         on test and on any database whose name begins test_."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "That is why the schema copy exists. " }
                            "test_glpi_schema matches "
                            code { class: "text-xs", "test\\_%" }
                            ", so creating and filling it needed no root and no new grant."
                        }
                        li {
                            span { class: "font-medium", "It is also a soft spot. " }
                            "Any account that can log in can create and fill databases under \
                             that prefix. Harmless on a single-user laptop, worth knowing \
                             anywhere else."
                        }
                    }
                }
            }

            div { id: "db-mysql",
                Panel {
                    title: "mysql".to_string(),
                    subtitle: "The server's own bookkeeping".to_string(),
                    info: "Reading it needs privileges this app's account does not have: \
                           SELECT on mysql.user is refused with ERROR 1142."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "Accounts, their authentication plugins, every grant, stored \
                         routines, time zones and the event scheduler's state. What any \
                         account may reach - the two-table scope this app used to have, and \
                         the server-wide one it has now - is not a setting in GLPI or in \
                         this app, but rows in here."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "It decides who may connect how. " }
                            "Accounts on this server authenticate with the unix_socket plugin, \
                             which the kernel can only satisfy over a local socket - so they \
                             cannot authenticate over TCP at all, whatever password is offered."
                        }
                        li {
                            span { class: "font-medium", "Editing it by hand is the wrong tool. " }
                            "CREATE USER and GRANT keep the in-memory privilege tables in step; \
                             direct writes here need a FLUSH PRIVILEGES to take effect and can \
                             leave the two views disagreeing."
                        }
                    }
                }
            }

            div { id: "db-information-schema",
                Panel {
                    title: "information_schema".to_string(),
                    subtitle: "Not stored anywhere - generated per query".to_string(),
                    info: "Privilege-filtered by design: the same query returns different \
                           rows to different accounts."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "A virtual database describing the others: tables, columns, indexes, \
                         constraints, engines, character sets. There are no files behind it - \
                         the server composes the answer while you ask."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "It reports what you may see, not what exists. " }
                            "Counting glpi's tables through it as this app's account returns 2, \
                             not 447. That is the grant talking, and it is the single easiest \
                             way to mislead yourself about a database you do not fully own."
                        }
                        li {
                            span { class: "font-medium", "Useful anyway. " }
                            "Column types, nullability and index membership for everything you \
                             can reach, queryable with ordinary SQL - which is how the figures \
                             elsewhere on this page are produced."
                        }
                    }
                }
            }

            div { id: "db-performance-schema",
                Panel {
                    title: "performance_schema".to_string(),
                    subtitle: "Present and switched off".to_string(),
                    info: "Measured here: @@performance_schema = 0, so the instrumentation \
                           tables exist but stay empty."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "MariaDB's instrumentation: where time goes, which statements run, \
                         what waits on what, memory by allocation site. On this server it is \
                         disabled, so the database is listed and its tables return nothing."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "Turning it on is not free. " }
                            "It costs memory and some throughput, and it needs a server \
                             restart - performance_schema cannot be switched on at runtime."
                        }
                        li {
                            span { class: "font-medium", "Nothing here depends on it. " }
                            "No page in this app reads it, and GLPI does not either. It is \
                             listed because it is one of the seven, not because it is in use."
                        }
                    }
                }
            }

            div { id: "db-sys",
                Panel {
                    title: "sys".to_string(),
                    subtitle: "Readable views over performance_schema".to_string(),
                    info: "Its usefulness is tied to the entry above: with \
                           performance_schema off, most of these views have nothing to show."
                        .to_string(),
                    p { class: "mb-3 text-sm",
                        "A thin layer of views and helper routines that turn \
                         performance_schema's raw counters into readable answers - slowest \
                         statements, unused indexes, tables by I/O. It stores nothing of its \
                         own."
                    }
                    ul { class: "ml-5 list-disc space-y-2 text-sm",
                        li {
                            span { class: "font-medium", "Empty here, by consequence. " }
                            "With the instrumentation disabled these views return empty \
                             results rather than errors, which reads as \u{201c}nothing is wrong\u{201d} \
                             when it actually means \u{201c}nothing is being measured\u{201d}."
                        }
                    }
                }
            }
        }
    }
}

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
fn ApiRead(sub: String) -> Element {
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

    // The strip is drawn by the page, in SubTabs, so it shares the page tabs'
    // line instead of owning one.
    let at = nav::API_SUBS
        .iter()
        .position(|(n, _)| *n == sub)
        .unwrap_or(0);

    rsx! {

        if at == 1 {
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
        } else if at == 2 {
            Rsql { reach }
        } else {
        Panel {
            title: "Fields the API withholds".to_string(),
            subtitle: "Declared against received".to_string(),
            info: "A call can succeed and still be short a field. GLPI drops fields the \
                  account may not read instead of refusing the request, so this compares \
                  what GLPI's own schema promises against what actually arrived. Why this is \
                  not visible anywhere else: A withheld field is absent, not null, and this \
                  app makes every field optional on purpose so that a drifting GLPI degrades \
                  to a blank cell instead of a failed page. That is the right trade and it \
                  is exactly what hides this: the cell looks like data GLPI does not have, \
                  rather than data this account may not see. Where the comparison comes \
                  from: GLPI publishes its own OpenAPI document, and it is generated from \
                  the schema declarations rather than from the caller's rights - so it lists \
                  every field, while the response carries only the permitted ones. The \
                  difference between the two is the answer, and no second source is needed. \
                  Write-only fields are not counted: A password field is declared and is \
                  never returned to anybody. Counting those would report a permanent problem \
                  on every call, so they are excluded and listed separately where present. \
                  What it cannot tell you: Only top-level fields: a withheld key inside a \
                  nested object is invisible to it. And it reports that a promised field did \
                  not arrive, never why - a field held back by rights and a field GLPI \
                  stopped sending are both simply missing. Nothing arriving to compare \
                  against is reported as unknown rather than as clean."
                .to_string(),
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
            info: "GLPI's API publishes a description of itself, and this is the whole of \
                  what it describes. Every name here is the shape of an API response - not a \
                  database table, and not one of the tables under db-read. Why the whole \
                  list is here: Because it is the denominator. The audit above covers five \
                  schemas, and five clean endpoints read as a clean API unless you can see \
                  how many there are. The count is taken from the document on every load \
                  rather than written down here, because a number nobody re-checks rots into \
                  a lie. Why there is no Received column: Because it would cost one request \
                  per schema - hundreds against one install - where the audited five cost \
                  five. A figure that cannot be had cheaply is left out rather than \
                  estimated from whatever the first page of a collection happened to carry. \
                  Declared, readable, write-only: Declared is every property the schema \
                  names. Write-only ones are never returned to anybody - a password field is \
                  the clear case - so readable is what a caller could actually expect back, \
                  and it is the number the audit compares against. They are split rather \
                  than summed so a schema that is mostly write-only cannot look like one \
                  that withholds. Not the same thing as db-read: db-read lists the two \
                  tables of GLPI's database this app reads, by column. This lists response \
                  shapes from GLPI's API. Nothing here is a table, nothing there is an \
                  endpoint, and both are called a schema by somebody - which is why they are \
                  two tabs on one page rather than one list."
                .to_string(),
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
    let mut t = use_signal(|| 0usize);

    rsx! {
        // Above the strip and named rather than an icon, by the same rule as
        // the Overview tab's own link: what it heads off is a misreading of
        // the page's title, which a reader has made before there is anything
        // on the page to hover. The R has been read as Rust more than once,
        // and this app's own builder being a .rs file is what makes that easy.
        div { class: "mb-3",
            InfoButton {
                label: "What RSQL is".to_string(),
                title: "RESTful Service Query Language, not Rust SQL".to_string(),
                body: "The R is RESTful: RSQL is the RESTful Service Query Language, and it \
                       is an extension of FIQL, the Feed Item Query Language. GLPI's own \
                       lexer says so in its header - src/Glpi/Api/HL/RSQL/Lexer.php:41 \
                       describes itself as parsing an RSQL query, \u{201c}extension of \
                       FIQL\u{201d}, into tokens. Nothing about it is Rust, and it predates \
                       this app and its language entirely. Where each half runs: an RSQL \
                       string is a value in an HTTP query parameter, and the thing that \
                       executes it is PHP - GLPI's RSQL/Parser.php, on GLPI's side of the \
                       wire, turning the filter into SQL against GLPI's own schema. This \
                       app never parses one. The Rust in the picture is incidental: \
                       backend/src/glpi/rsql.rs builds the filter string this app sends, \
                       and it is Rust only because the whole backend is - it would be the \
                       same language in any other client. Nor is RSQL anything the database \
                       knows: MariaDB speaks SQL and has no notion of RSQL, so a filter \
                       means something only to the API in front of it, which is the \
                       distinction the whole of this tab rests on."
                    .to_string(),
            }
        }

        // The only tab strip left on this page, and the one level the sidebar
        // deliberately does not take: these four are local view state, not
        // part of the URL, so there is nothing for a menu entry to link to.
        // Lifted rather than bordered or boxed, which is what it has always
        // been - it used to be the third of three rows and the styling was
        // what told a reader which level a click had moved.
        div { role: "tablist", class: "tabs tabs-sm tabs-lift mb-2 w-fit",
            button {
                role: "tab",
                class: if t() == 0 { "tab tab-active" } else { "tab" },
                onclick: move |_| t.set(0),
                "operator"
            }
            button {
                role: "tab",
                class: if t() == 1 { "tab tab-active" } else { "tab" },
                onclick: move |_| t.set(1),
                "grammar"
            }
            button {
                role: "tab",
                class: if t() == 2 { "tab tab-active" } else { "tab" },
                onclick: move |_| t.set(2),
                "use"
            }
        }

        if t() == 1 {
            Grammar {}
        } else if t() == 2 {
            Usage { reach }
        } else {
            Operators {}
        }
    }
}

/// Every operator the parser defines - the operator tab.
#[component]
fn Operators() -> Element {
    rsx! {
        Panel {
            title: "Operators".to_string(),
            subtitle: format!("All {} GLPI defines, whether or not this app uses them", OPERATORS.len()),
            info: "RSQL is the filter language GLPI's API takes in its filter parameter. \
                  These are every operator its parser defines - the full surface, not the \
                  part this app currently builds. Where this list comes from: GLPI's own \
                  parser, read in the source of both releases and diffed - 16 operators in \
                  11.0.12 and 18 in 12.0.1. It is not read from the API document, because \
                  the document does not contain it: the filter parameter is described there \
                  in full as \u{201c}RSQL query string\u{201d} and names no operator at all. \
                  Two of them need 12.0.1: =istruthy= and =isnottruthy= are the only \
                  operators 12.0.1 adds. An install still on 11.0.12 answers them with an \
                  RSQL error rather than ignoring them, so the failure is loud either way - \
                  which is the useful kind. Case, and what empty means: =like= is matched \
                  against a BINARY cast and is therefore case sensitive, where =ilike= is \
                  not; that is the only difference between them. =empty= covers both the \
                  empty string and NULL, where =isnull= covers only NULL - so a column \
                  holding \u{27}\u{27} answers =empty= and not =isnull=. All eighteen are \
                  reachable from this app: The filter builder in this app covers every \
                  operator listed here, including the two that need 12.0.1, and flags those \
                  so a caller can avoid them on an older install. What it does not do is \
                  guess: values are always quoted and escaped, because an unquoted value \
                  containing a semicolon or a comma silently changes the shape of the query \
                  rather than failing."
                .to_string(),
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
    }
}

/// What joins, groups and quotes a term - the grammar tab.
#[component]
fn Grammar() -> Element {
    rsx! {
        Panel {
            title: "Grammar".to_string(),
            subtitle: "What joins terms, and what must be escaped".to_string(),
            info: "An operator on its own is not a query. These are the characters that \
                  combine terms, group them, and quote a value - every one of them from \
                  GLPI's lexer rather than from an example that happened to work. Why \
                  escaping is the whole hazard: An unquoted value runs until the next \
                  unescaped comma, semicolon or closing parenthesis. So a computer named \
                  foo;bar, interpolated straight into a filter, does not fail - it becomes \
                  two terms joined by AND, and the answer comes back wrong and plausible. \
                  Quoting and escaping is why this app builds filters rather than formatting \
                  them. Dots reach into joined records: A property name may contain dots, \
                  and GLPI resolves the path across the joins its schema declares - so \
                  user.name filters computers by the name of the user they are assigned to, \
                  in one request. That is the part worth knowing: the filter is not \
                  restricted to the columns of one table."
                .to_string(),
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
}

/// Where a filter may be used, and the three ways it is refused - the use tab.
///
/// The only one of the three that depends on the fetch: its counts come off
/// the live document, where the operators and the grammar are read from GLPI's
/// source and hold whether or not this install answers.
#[component]
fn Usage(reach: api::FilterReach) -> Element {
    rsx! {
        Panel {
            title: "Reach, and what it refuses".to_string(),
            subtitle: if reach.operations_total > 0 {
                format!(
                    "{} of {} operations take a filter, across {} paths",
                    reach.operations, reach.operations_total, reach.paths
                )
            } else {
                "Counts unavailable - the reference below is unaffected".to_string()
            },
            info: "Which calls can be filtered at all, and the three ways GLPI rejects a \
                  filter it will not run. The counts are live: Read off GLPI's own API \
                  document on this install each time the tab opens, not written down here - \
                  the same rule the rest of this page follows. Zero means the document could \
                  not be read, which the fields tab explains; it does not mean filtering is \
                  unavailable. Mapped properties are refused: A property GLPI computes from \
                  another one - declared x-mapped-from in its schema - cannot be filtered, \
                  and the request fails with \u{201c}Mapped properties cannot be used in \
                  RSQL\u{201d}. There are 43 such declarations in 12.0.1 against 31 in \
                  11.0.12, so the surface grows at the upgrade. None of them are \
                  discoverable from the document: it strips the extension, so the only way \
                  to know a property is unfilterable is to filter on it and read the error. \
                  The three refusals: Unknown property, unknown operator, and mapped \
                  property. All three name the offending term rather than failing the whole \
                  call silently, which is the one respect in which this is friendlier than \
                  the database: a filter that cannot run says so."
                .to_string(),
            div { class: "max-w-prose space-y-2 text-sm text-base-content/70",
                p {
                    "Filtering is a GET-only affair, and the "
                    // The document is GLPI's own, and it is worth opening: the
                    // claim being made about it is that it says almost nothing,
                    // which a reader should be able to check in one click
                    // rather than take on trust. Points at the Swagger UI
                    // rather than doc.json, which is 4.6 MB of it.
                    match crate::glpi_link::link("api.php/v2.3/doc") {
                        Some(h) => rsx! {
                            a {
                                class: "link link-primary",
                                href: "{h}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                title: "GLPI High-Level REST API 2.3.0 - GLPI's own OpenAPI document",
                                "document ↗"
                            }
                        },
                        // GLPI's address is not known yet, so the sentence
                        // still reads - it just does not link anywhere.
                        None => rsx! { "document" },
                    }
                    " describes the "
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
                       tickets, agents, and so on. One name each, and not a fixed number: it \
                       grows. A custom asset type defined in GLPI creates a new right named \
                       after it, and a plugin can add its own, which is why this figure is \
                       counted from what GLPI reports rather than written into this page."
                    .to_string(),
            }

            StatTile {
                title: "Stored rows".to_string(),
                value: counts.stored_rows.to_string(),
                desc: "profile × right pairs".to_string(),
                body: "The actual size of the one table that holds any of this. Counted, not \
                       multiplied: GLPI stores only the pairs that exist, and profiles times \
                       rights would overstate it, because a few rights have no entry on some \
                       profiles at all. An entry that does not exist denies just as one set \
                       to zero does, but it is a different statement - nobody chose it. The \
                       Rights page marks those as “no entry”."
                    .to_string(),
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
                  in its code. Why it is worth knowing: It decides what can be changed, by \
                  whom, and what a database account would be good for. Anything in the \
                  database can be edited by an administrator and differs between \
                  installations. Anything in the code is the same everywhere and changes \
                  only when GLPI is upgraded. This app does not read the database: \
                  Everything it shows comes through GLPI's API, as the account on the Status \
                  page. That is deliberate: the API applies the account's own permissions \
                  and its entity scope on every request, and reading the database directly \
                  would bypass both - as well as tying this app to GLPI's internal table \
                  layout across upgrades."
                .to_string(),
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
            info: "Four things that are commonly assumed to be in the database and are not. \
                   Each was looked for in both of GLPI's APIs, in the machine-readable \
                   description the API publishes of itself, and in GLPI's own command-line \
                   tool: none of the four is available by any of those routes, and none is in \
                   the database either - they are all fixed in the program. So a database \
                   account is the wrong tool for discovering or changing what the API does. \
                   It would help only for reading data the API declines to expose - a \
                   different question, with its own cost, since it steps around the \
                   permission and entity checks every API request applies."
                .to_string(),

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
                  creates both a new API address and a new right. How it works: A custom \
                  type is a row describing it. GLPI builds the class for that type when it \
                  starts, taking the right's name from the row - so a type whose internal \
                  name is kiosk gets a right called asset_kiosk - and keeps the rights table \
                  in step as the definition's profiles are edited. No code changes. Why it \
                  is worth flagging: It means the API's address list is not fixed, and \
                  neither is the set of rights. Anything that treats either as a constant - \
                  including a page like this one - is wrong in principle. It is also why the \
                  figures above are counted from GLPI rather than written down here. Not \
                  observed here: This installation defines no custom types, so the addresses \
                  exist as templates with nothing behind them. The behaviour described above \
                  was read in GLPI's source and confirmed only as far as the empty result; \
                  it has never been watched happen on this box."
                .to_string(),
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
