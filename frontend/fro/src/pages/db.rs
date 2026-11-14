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
use crate::components::nav;
use crate::components::{InfoButton, InfoLink, Panel, StatTile};
use dioxus::prelude::*;

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
    const TABS: [&str; 4] = ["overview", "db-read", "api-read", "maria"];
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

        if at == 3 {
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

/// What the engine does with GLPI's schema, as distinct from what GLPI puts
/// in it.
///
/// A container for two sub-tabs: storage for the choices GLPI's installer
/// makes, db's for what else is on the server.
#[component]
fn Maria(sub: String) -> Element {
    // general first, so it is what a bare ?tab=maria lands on - the strip
    // reads as orientation, then detail. Adding it changed that default from
    // storage, which is deliberate. The strip itself is drawn by the page, in
    // SubTabs, so it can share the page tabs' line.
    let at = nav::MARIA_SUBS
        .iter()
        .position(|(n, _)| *n == sub)
        .unwrap_or(0);

    rsx! {

        if at == 3 {
            Databases {}
        } else if at == 2 {
            MariaStorage {}
        } else if at == 1 {
            MariaEngine {}
        } else {
            MariaGeneral {}
        }
    }
}

/// What GLPI requires of the server, as opposed to what it puts in it.
///
/// Requirements rather than readings: GLPI's own checks are quoted from
/// source, and the query to test each is given, because a value pasted in
/// here would rot the first time the server was upgraded. This page already
/// learned that lesson for its counts.
#[component]
fn MariaGeneral() -> Element {
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
                "Nothing here is read live. The figures on this page that are live come \
                 from this app's own backend; the server's own settings are not among \
                 them, so the queries above are the way to answer these three."
            }
        }
    }
}

/// What the server actually is, read from the running instance.
///
/// The only page on this tab that queries the server rather than GLPI's
/// source, and that is the point of it: everything else here describes what
/// GLPI expects of a database, and this says what is answering. The two
/// disagreeing - a different engine, a different collation, a row format GLPI
/// did not ask for - is a finding no other page would surface.
#[component]
fn MariaEngine() -> Element {
    let report = use_resource(|| async move { api::fetch_mariadb().await });

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Asking the server\u{2026}"
                }
            },
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not reach the backend." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
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
                rsx! { EngineFacts { facts } }
            }
        }
    }
}

/// The facts themselves, once there are some.
#[component]
fn EngineFacts(facts: api::ServerFacts) -> Element {
    let days = facts.uptime_seconds / 86_400;
    let hours = (facts.uptime_seconds % 86_400) / 3_600;
    let gib = facts.glpi_bytes as f64 / 1_073_741_824.0;

    rsx! {
        Engines { facts: facts.clone() }

        div { class: "mt-4 grid gap-4 items-start lg:grid-cols-2",
            Panel {
                title: "The server answering".to_string(),
                subtitle: facts.flavour.clone(),
                info: "Read from the live instance: @@version, @@version_comment and the \
                       Uptime status variable. Everything else on this tab comes from GLPI's \
                       source, so this is the one page that can disagree with the others - \
                       and a disagreement here is the finding."
                    .to_string(),
                ul { class: "ml-5 list-disc space-y-2 text-sm",
                    li {
                        span { class: "font-medium", "Version. " }
                        code { class: "text-xs", "{facts.version}" }
                        " - the schema this app quotes was read from GLPI 12.0.1's installer, \
                         which is a different thing from the server running it."
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

            Panel {
                title: "Settings that change what GLPI sees".to_string(),
                subtitle: "Ten of roughly six hundred".to_string(),
                info: "Named one by one rather than dumped: SHOW VARIABLES returns about six \
                       hundred rows and answers nothing. These are the ones that decide how \
                       GLPI's data is stored, compared or truncated."
                    .to_string(),
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        tbody {
                            for (name , value) in facts.variables.iter() {
                                tr { key: "{name}",
                                    td { class: "font-mono text-xs whitespace-nowrap", "{name}" }
                                    td { class: "font-mono text-xs break-all", "{value}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What each engine the server has compiled in would mean for GLPI's schema.
///
/// A tab per engine, where this used to be a two-column table. The table
/// answered one question - which engine a CREATE TABLE gets when it names none
/// - and had no room for the one worth asking, which is what it would mean to
/// find a GLPI table on any of the other seven.
///
/// The strip is built from what the server reports and not from the notes
/// below it: an engine this app has never heard of still gets a tab carrying
/// the one fact the server gave about it, and an engine the server does not
/// compile in has no tab at all. Which tab is open is local view state, like
/// the RSQL strip, so the sidebar deliberately does not take this level.
#[component]
fn Engines(facts: api::ServerFacts) -> Element {
    // Open on the default rather than on whatever sorts first: the default is
    // the engine GLPI's 447 tables are actually on, and Aria is a poor thing
    // to land on when the question is what the schema is stored by.
    let default_at = facts
        .engines
        .iter()
        .position(|(_, support)| support == "DEFAULT")
        .unwrap_or(0);
    let mut t = use_signal(|| default_at);
    let at = t().min(facts.engines.len().saturating_sub(1));
    let current = facts.engines.get(at).cloned();

    rsx! {
        // The head of the page, above both readings. Which engine is being
        // asked about is the page's subject, and a strip placed under the
        // answer is a strip that is read after it is needed - this page had
        // its tab row at the top before the sidebar took that level, and an
        // engine is not a destination the sidebar can hold.
        div {
            role: "tablist",
            class: "tabs tabs-sm tabs-lift mb-3 flex-nowrap overflow-x-auto",
            for (i , (name , support)) in facts.engines.iter().enumerate() {
                button {
                    key: "{name}",
                    role: "tab",
                    class: if i == at { "tab tab-active" } else { "tab" },
                    onclick: move |_| t.set(i),
                    "{name}"
                    if support == "DEFAULT" {
                        span { class: "badge badge-primary badge-xs ml-2", "default" }
                    }
                }
            }
        }

        match current {
            Some((name, support)) => rsx! {
                Panel {
                    // The engine's own name, because the strip above has
                    // already said what the subject is and a panel titled
                    // "Engines" under eight tabs named after engines says it
                    // a second time.
                    title: name.clone(),
                    subtitle: engine_note(&name)
                        .map(|(what, _)| what.to_string())
                        .unwrap_or_else(|| "Compiled into this server".to_string()),
                    info: "The strip above is read from information_schema.ENGINES on the \
                           live server, so it is what this install has, not a list of what \
                           MariaDB ships. SUPPORT = DEFAULT marks the engine a CREATE TABLE \
                           gets when it names none. What each tab then says about GLPI is \
                           not read from anywhere - it is what finding a GLPI table on that \
                           engine would mean - and the line at the foot of this panel is the \
                           check for it: GLPI's installer writes ENGINE=InnoDB into all 447 \
                           statements, so anything else is a table that was altered \
                           afterwards."
                        .to_string(),

                    Engine {
                        name,
                        support,
                        glpi_tables: facts.glpi_tables,
                        others: facts.glpi_other_engines.clone(),
                    }

                    // At the foot rather than inside the body: it is a
                    // finding about the whole database, not about whichever
                    // engine is open, and it is the reason the strip exists.
                    if facts.glpi_other_engines.is_empty() {
                        p { class: "mt-4 border-t border-base-300 pt-3 text-sm text-base-content/70",
                            "Every table in GLPI's database is InnoDB, which is what its \
                             installer writes and what its foreign keys need."
                        }
                    } else {
                        div { class: "mt-4 alert alert-warning",
                            div {
                                div { class: "font-semibold",
                                    "{facts.glpi_other_engines.len()} table(s) are not InnoDB"
                                }
                                p { class: "mt-1 text-xs",
                                    "Transactions and foreign keys do not work the same on \
                                     another engine, and nothing in GLPI's own screens \
                                     reports this."
                                }
                            }
                        }
                    }
                }
            },
            // Only reachable if the server reported no engines at all, which
            // would itself be the finding.
            None => rsx! {
                p { class: "text-sm text-base-content/70",
                    "The server reported no engines, which it cannot do while answering \
                     queries. Treat this as the backend having read something it did not \
                     understand rather than as a server without storage."
                }
            },
        }
    }
}

/// One engine: what the server says it is, and what it would mean here.
#[component]
fn Engine(
    name: String,
    support: String,
    glpi_tables: u64,
    /// GLPI's tables whose engine is neither NULL nor InnoDB, as the backend
    /// reports them. Passed whole rather than pre-filtered because InnoDB's
    /// own count is derived from the length of it.
    others: Vec<(String, String)>,
) -> Element {
    let note = engine_note(&name);
    let here: Vec<&(String, String)> = others.iter().filter(|(_, e)| *e == name).collect();
    // The backend lists only the tables that are neither InnoDB nor a view, so
    // InnoDB's count is the rest. A view has a NULL engine and would be
    // counted as InnoDB here; GLPI's installer creates none.
    let in_glpi = if name == "InnoDB" {
        glpi_tables.saturating_sub(others.len() as u64)
    } else {
        here.len() as u64
    };

    rsx! {
        // Capped rather than left to the panel's width: the panel is as wide
        // as the page, and a paragraph set 1300px wide is a paragraph nobody
        // finishes.
        div { class: "max-w-4xl text-sm",
            // The name and the one-line description are the panel's title and
            // subtitle, so the body starts at what it means here.
            if let Some((_, means)) = note {
                p { class: "mb-2", "{means}" }
            } else {
                p { class: "mb-2",
                    "This app carries no note about it, which means only that it was not one \
                     of the eight the page was written against."
                }
            }

            p { class: "text-base-content/70",
                if support == "DEFAULT" {
                    "It is the default, so a CREATE TABLE naming no engine gets this one. "
                } else {
                    "Available, but not the default: a table is only on it if something said so. "
                }
                if in_glpi == 0 {
                    "No table in GLPI's database uses it."
                } else {
                    "{in_glpi} of GLPI's {glpi_tables} tables are on it."
                }
            }

            if !here.is_empty() {
                ul { class: "mt-2 ml-5 list-disc space-y-1",
                    for (table , _) in here.iter() {
                        li { key: "{table}", class: "font-mono text-xs", "{table}" }
                    }
                }
            }
        }
    }
}

/// -> what this engine is, and what a GLPI table on it would mean.
fn engine_note(name: &str) -> Option<(&'static str, &'static str)> {
    ENGINE_NOTES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, what, means)| (*what, *means))
}

/// Engine, what it is, and what a GLPI table on it would mean.
///
/// The middle column is the engine's own description, checked against
/// information_schema.ENGINES.COMMENT on this server rather than taken from
/// documentation - on a MariaDB server that text is a column. The third is the
/// part no engine says about itself and the reason the tabs exist: InnoDB is a
/// choice GLPI's installer made, and every other row is what it would mean to
/// find that choice undone.
///
/// Eight because that is what this server compiles in. The strip is driven by
/// the server's list, not by this one, so an engine missing here costs a
/// sentence rather than a tab.
const ENGINE_NOTES: [(&str, &str, &str); 8] = [
    (
        "InnoDB",
        "Transactions, row-level locking, foreign keys and encryption.",
        "What GLPI's installer writes into every one of its 447 CREATE TABLE statements, and \
         what its foreign keys and its transactional writes need. Every other engine on this \
         strip is therefore a finding rather than a choice.",
    ),
    (
        "Aria",
        "Crash-safe, with MyISAM's heritage and not its fragility.",
        "MariaDB's own: the privilege tables are Aria, and so are the internal temporary \
         tables a large sort or GROUP BY spills to disk, so it is busy on every server \
         whether or not anything you wrote names it. It is not transactional, so a GLPI \
         table on it would lose rollback.",
    ),
    (
        "CSV",
        "Stores a table as a plain CSV file, with no indexes and no NULLs.",
        "Nothing GLPI creates. It is on the strip because the server compiles it in, and it \
         turns up in a database because a human made a table for an export.",
    ),
    (
        "MEMORY",
        "Hash-indexed and held entirely in RAM.",
        "The rows are lost on restart while the table definition survives, so a GLPI table \
         here would come back empty rather than missing - which reads as data that was never \
         collected rather than as a failure.",
    ),
    (
        "MRG_MyISAM",
        "One table that reads a set of identical MyISAM tables as though they were one.",
        "It needs MyISAM tables beneath it, and GLPI has none. Nothing in GLPI creates or \
         expects one.",
    ),
    (
        "MyISAM",
        "Non-transactional, table-level locking, no foreign keys.",
        "What older GLPI installs were built on. An install upgraded in place rather than \
         reinstalled can still have tables on it, and that is precisely what the line below \
         this strip looks for.",
    ),
    (
        "PERFORMANCE_SCHEMA",
        "Not storage: the instrumentation tables in the performance_schema database.",
        "Its rows are the server's own counters, generated on read and held in memory. \
         Nothing of GLPI's can be created on it.",
    ),
    (
        "SEQUENCE",
        "Generates a table of sequential values on demand - seq_1_to_100 exists the moment \
         it is named.",
        "Nothing is stored and no table is created, so it can never appear as a GLPI table's \
         engine. It is on the strip because the server compiles it in.",
    ),
];

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
