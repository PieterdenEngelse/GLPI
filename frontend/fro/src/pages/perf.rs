//! How fast GLPI is, and where the time goes.
//!
//! ABOUT GLPI, NOT ABOUT THIS DASHBOARD. Every figure here is a reading of
//! the install being watched - PHP answering its own endpoints, and MariaDB
//! answering a query of the shape GLPI's list views run. This app's own
//! latency is deliberately absent: it is the instrument, and an instrument
//! that reports on itself invites tuning the wrong thing.
//!
//! WHY THE COMPARISON IS THE POINT. A millisecond count on its own says
//! nothing - nobody knows whether 20 ms is good. The two numbers beside each
//! other do say something: what a GLPI request costs, and what the database
//! inside it costs. When the second is a rounding error against the first,
//! the database is not what to tune, however tempting its 137 settings are.
//! If that ratio ever closes, this page is where it shows.
use crate::api;
use crate::app::Route;
use crate::components::{FetchError, InfoButton, Panel, TuningField};
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn Perf() -> Element {
    // Measured when the page opens, and again on a reload - there is no
    // refresh control, because the reading is the page.
    let report = use_resource(|| async move { api::fetch_perf().await });

    rsx! {
        div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
            InfoButton {
                label: "What is this".to_string(),
                title: "GLPI's speed, not this app's".to_string(),
                body: "Every number on this page is a measurement of the GLPI install this \
                       dashboard watches, taken when the page was opened. Nothing here \
                       describes this app: it is the instrument, and an instrument \
                       reporting its own latency invites tuning the wrong thing. Two \
                       endpoints are timed because both are unauthenticated and cheap - \
                       status.php runs GLPI's health checks, index.php renders the login \
                       page - which makes them a FLOOR rather than a typical page: the \
                       overhead every request pays before doing anything particular. A \
                       filtered list view costs that plus its own work. Beside them is a \
                       query of the shape those list views run, against the same database. \
                       The comparison is the point. A millisecond figure alone says \
                       nothing, but a request that costs thirty times the query inside it \
                       says plainly that the database is not the thing to tune, whatever \
                       the engine panel's 137 settings suggest. The day that ratio closes \
                       is the day the database starts deciding how fast GLPI feels, and \
                       this page is where it would show. Each timing is the best and the \
                       median of several runs: a single sample reports a cold start - a \
                       fresh connection, whatever PHP has let fall out of opcache - as \
                       though it were the normal case."
                    .to_string(),
            }
        }

        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Timing GLPI\u{2026}"
                }
            },
            Some(Err(e)) => rsx! {
                FetchError { error: e.clone() }
            },
            Some(Ok(r)) => rsx! { Readings { report: r.clone() } },
        }
    }
}

/// The readings, once there are some.
#[component]
fn Readings(report: api::PerfReport) -> Element {
    let api::PerfReport {
        glpi_url,
        php,
        http,
        query,
        samples,
        engine,
        cron,
        tables,
        total_kib,
        total_tables,
    } = report;

    // The ratio the page exists for. Taken against the login page rather
    // than status.php: status.php is the cheapest thing GLPI serves and
    // would flatter the comparison.
    let page = http.iter().find(|t| t.what.contains("index.php"));
    let ratio = match (page, query.as_ref()) {
        (Some(p), Some(q)) if q.median_ms > 0.0 && p.error.is_none() => {
            Some(p.median_ms / q.median_ms)
        }
        _ => None,
    };

    rsx! {
        // FIRST on the page, because it is what the readings below are read
        // against: a request time measured once says nothing about whether
        // it is normal, and the history is the only thing here that can say
        // what normal has been.
        //
        // A link rather than the charts themselves. Sixteen plots above the
        // two timings made the timings the footnote, and the charts are
        // worth more room than a shared page can give them - so they have
        // their own, and this keeps the pointer where the question arises.
        div { class: "mb-4", HistoryLink {} }

        div { class: "grid gap-4 items-start lg:grid-cols-2",
            Panel {
                title: "What a GLPI request costs".to_string(),
                subtitle: format!("best and median of {samples}, measured just now"),
                info: "Both are unauthenticated and small, so this is the floor a request \
                       pays before doing anything particular - not what a filtered list \
                       view costs. Timed including the body: stopping at the headers would \
                       measure the first byte and call it the request. Connections are not \
                       pooled, because a browser arriving at GLPI does not have one either."
                    .to_string(),
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        tbody {
                            for t in http.iter() {
                                tr { key: "{t.what}",
                                    td { class: "align-top",
                                        div { class: "font-mono text-xs", "{t.what}" }
                                        div { class: "mt-1 max-w-sm text-xs text-base-content/60",
                                            "{t.detail}"
                                        }
                                    }
                                    td { class: "whitespace-nowrap align-top text-right",
                                        if let Some(e) = t.error.clone() {
                                            div { class: "text-xs text-error", "{e}" }
                                        } else {
                                            div { class: "font-mono text-sm", "{t.median_ms:.1} ms" }
                                            div { class: "text-xs text-base-content/60",
                                                "best {t.best_ms:.1}, worst {t.worst_ms:.1}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                p { class: "mt-3 max-w-prose text-xs text-base-content/60",
                    "Answering at "
                    span { class: "font-mono", "{glpi_url}" }
                    if let Some(v) = php.clone() {
                        ", which reports itself as "
                        span { class: "font-mono", "{v}" }
                        "."
                    } else {
                        ". The server does not volunteer its PHP version, which is a \
                         hardening choice rather than a fault."
                    }
                }
            }

            Panel {
                title: "What the database costs it".to_string(),
                subtitle: "The same install, underneath".to_string(),
                info: "A COUNT over the largest table GLPI's inventory fills - what a list \
                       view does before drawing a page of rows. The round trip from this \
                       service over the unix socket, driver included: MariaDB has no \
                       per-statement stopwatch a client can read back, so this measures \
                       what can be measured and says what is in it. The socket and driver \
                       are worth tenths of a millisecond, and where this number matters is \
                       when it stops being tenths."
                    .to_string(),
                match query {
                    None => rsx! {
                        div { class: "alert alert-info",
                            "No database connection is configured, so only the HTTP side of \
                             this page has anything to report."
                        }
                    },
                    Some(q) => rsx! {
                        div { class: "font-mono text-sm", "{q.median_ms:.2} ms" }
                        div { class: "text-xs text-base-content/60",
                            "best {q.best_ms:.2}, worst {q.worst_ms:.2}"
                        }
                        p { class: "mt-2 max-w-prose text-xs text-base-content/60", "{q.detail}" }
                    },
                }

                if let Some(x) = ratio {
                    div { class: "mt-4 border-t border-base-300 pt-3",
                        p { class: "max-w-prose text-sm",
                            "A login page costs about "
                            span { class: "font-semibold", "{x:.0}\u{d7}" }
                            " the query inside it. While that holds, GLPI's speed is \
                             decided by PHP and by how many requests a page makes - not by \
                             the engine's settings, however many of them there are."
                        }
                        p { class: "mt-2 max-w-prose text-xs text-base-content/60",
                            "The ratio is what to watch rather than either number. It \
                             closes as the database outgrows memory, and that is the point \
                             at which the InnoDB panel stops being reference and starts \
                             being a tool."
                        }
                    }
                }
            }
        }

        // The two explanatory panels, side by side on anything wide enough to
        // hold them. Both are prose with a figure or two, both read after the
        // timings above rather than instead of them, and at full width each
        // left half the screen empty - their paragraphs are capped at a
        // readable measure, so the width was going spare rather than being
        // used. Stacked below lg, where two columns of prose would be two
        // narrow columns of prose.
        //
        // Route comparison first: it answers the question the two readings
        // above provoke - why the same data costs so differently depending on
        // how it is reached - and it is the DB page's "could we just read the
        // database for that?" arriving from the other side.
        div { class: "mt-4 grid gap-4 items-start lg:grid-cols-2",
            Panel {
                title: "Through the API, or straight to the database".to_string(),
                subtitle: "The same rows, two routes, two orders of magnitude".to_string(),
                info: "Both halves of this page read the same install. The difference \
                       between them is not the database being fast and GLPI being slow - \
                       it is that the two routes do different amounts of work, and almost \
                       none of the difference is spent in MariaDB. What the API adds is \
                       listed below, and what it buys is at the end of it: the direct \
                       query is quick partly because it skips every one of them. This app \
                       takes both routes deliberately - the API by default, SQL where the \
                       API cannot answer - and these numbers are the cost side of that \
                       decision, not an argument against either."
                    .to_string(),

                if let Some(x) = ratio {
                    p { class: "max-w-prose text-sm",
                        "The readings above are the two routes at their extremes: a page \
                         that does almost nothing still costs about "
                        span { class: "font-semibold", "{x:.0}\u{d7}" }
                        " a real query against the largest table GLPI fills. Every API \
                         call pays that floor before it does anything of its own."
                    }
                }

                p { class: "mt-3 max-w-prose text-sm", "Where the time goes, in order of size:" }
                ul { class: "mt-2 ml-5 max-w-prose list-disc space-y-2 text-sm",
                    li {
                        span { class: "font-medium", "PHP's per-request bootstrap. " }
                        "Autoload, configuration, session, rights setup - paid in full \
                         before GLPI looks at what was asked. It is the whole of what the \
                         two readings above measure, which is why they are called a floor: \
                         those endpoints do nearly nothing, and the time is the request \
                         existing rather than the request working."
                    }
                    li {
                        span { class: "font-medium", "The round trip and the JSON. " }
                        "A socket to localhost and a payload encoded on one side and \
                         decoded on the other. Small, but it is per request, and it is \
                         there a second time when a token has to be fetched first."
                    }
                    li {
                        span { class: "font-medium", "More SQL than one query. " }
                        "What arrives as one endpoint is several statements with the \
                         permission and entity joins the direct route has no equivalent \
                         of. Still milliseconds of database time against tens of \
                         milliseconds of everything else."
                    }
                }

                p { class: "mt-3 max-w-prose text-sm",
                    span { class: "font-medium", "The multiplier that actually bites is paging. " }
                    "A collection arrives a page at a time and every page pays the \
                     bootstrap again. Assets/Software is 1330 rows in three requests on \
                     this install - three bootstraps to answer one question - which is why \
                     a filter is a performance lever and not only a correctness one. "
                    "Pushing the condition into "
                    span { class: "font-mono text-xs", "filter=" }
                    " makes the cost scale with the answer instead of the dataset, and \
                     what it saves is whole requests rather than queries."
                }

                p { class: "mt-3 max-w-prose text-sm",
                    span { class: "font-medium", "What the slower route buys. " }
                    "Profile rights, entity scoping, fields GLPI derives rather than \
                     stores, and a response shape that survives a column being renamed \
                     upstream. The quick query enforces none of it and breaks on that \
                     rename at runtime. Speed is not the only axis, and this page is \
                     deliberately only about the one."
                }

                p { class: "mt-3 max-w-prose text-xs text-base-content/60",
                    span { class: "font-medium", "What this panel does not measure. " }
                    "No authenticated collection fetch is timed here - both HTTP readings \
                     are unauthenticated on purpose, so nothing above is a reading of what \
                     a real filtered list view costs. The floor is measured; everything a \
                     page builds on top of it is not, and would need a token and a profile \
                     to be, which would make this page an actor in the install it watches."
                }
            }

            // Its own fetch, and rendered here rather than beside the page's
            // own resource on purpose: this one costs four requests to GLPI,
            // and firing them while the timings above are being taken would
            // add load to the thing being measured. By the time this renders,
            // /api/perf has already answered.
            RuleLoad {}
        }

        // Same reasoning as RuleLoad above: its own fetch, mounted after the
        // timings so a page load does not ask the database anything while
        // the database is being timed.
        div { class: "mt-4", SlowLog {} }

        div { class: "mt-4", SelfTiming {} }

        if !engine.is_empty() {
            div { class: "mt-4",
                Panel {
                    title: "How the engine is behaving".to_string(),
                    subtitle: "Counters since the server started, not settings".to_string(),
                    info: "The DB page says what InnoDB is set to; this says what came of \
                           it. Every figure is cumulative since the server last started, so \
                           a rate here is an average over that period rather than what is \
                           happening now - an instantaneous rate needs two samples a second \
                           apart, which is a second this page is not worth. Nothing here is \
                           marked good or bad: a hit rate that is excellent for one \
                           workload is a symptom in another, and a threshold written into \
                           this app would be its opinion wearing the server's clothes. The \
                           raw numbers each reading was computed from are printed beside it \
                           so the arithmetic can be checked rather than trusted."
                        .to_string(),
                    // Four across on a wide screen rather than three: seven
                    // readings in two rows of four and three, instead of
                    // three rows with a third of the last one empty.
                    div { class: "grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4",
                        for g in engine.iter() {
                            div {
                                key: "{g.label}",
                                class: "rounded-box border border-base-300 p-3",
                                div { class: "text-xs text-base-content/60", "{g.label}" }
                                div { class: "font-mono text-lg", "{g.value}" }
                                p { class: "mt-1 text-xs text-base-content/60", "{g.detail}" }
                            }
                        }
                    }
                }
            }
        }

        div { class: "mt-4 grid gap-4 items-start lg:grid-cols-2",
            if !cron.is_empty() {
                Panel {
                    title: "GLPI's own background work".to_string(),
                    subtitle: "Timed by GLPI, not by this app".to_string(),
                    info: "GLPI records how long each scheduled task took, in seconds, in \
                           glpi_crontasklogs - so this is the application's own account of \
                           its background work rather than anything inferred from outside. \
                           Slowest first. A task that is scheduled but has never run shows \
                           no time at all, which is worth seeing: on an install where cron \
                           is not wired up, every one of these is quietly not happening, \
                           and the symptom is stale data rather than an error. Frequency is \
                           what GLPI intends, not what it achieved."
                        .to_string(),
                    div { class: "overflow-x-auto",
                        table { class: "table table-sm",
                            tbody {
                                for t in cron.iter() {
                                    tr { key: "{t.name}",
                                        td { class: "align-top",
                                            div { class: "font-mono text-xs", "{t.name}" }
                                            div { class: "text-xs text-base-content/60",
                                                "every {t.frequency / 60} min"
                                                if t.state == 0 {
                                                    span { class: "badge badge-ghost badge-xs ml-2",
                                                        "disabled"
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "whitespace-nowrap text-right align-top",
                                            if t.runs == 0 {
                                                span { class: "text-xs text-base-content/60",
                                                    "never run"
                                                }
                                            } else {
                                                div { class: "font-mono text-sm",
                                                    "{t.avg_seconds:.2} s"
                                                }
                                                div { class: "text-xs text-base-content/60",
                                                    "worst {t.max_seconds:.2}, {t.runs} runs"
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

            if !tables.is_empty() {
                Panel {
                    title: "Where the data is".to_string(),
                    subtitle: format!(
                        "{:.1} MiB across {total_tables} tables",
                        total_kib as f64 / 1024.0,
                    ),
                    info: "The ten largest tables, by data plus indexes. Row counts are \
                           InnoDB's estimate rather than a COUNT, because an exact figure \
                           for every table is a scan each and the question here is scale. \
                           This is the panel that decides whether anything else on this \
                           page will matter later: while the whole database fits in the \
                           buffer pool, the engine's settings cannot be felt. The tables \
                           that grow with the fleet rather than with the schema - software \
                           installs, device rows, and the history in glpi_logs, which has \
                           no ceiling at all - are the ones to watch."
                        .to_string(),
                    div { class: "overflow-x-auto",
                        table { class: "table table-sm",
                            tbody {
                                for t in tables.iter() {
                                    tr { key: "{t.name}",
                                        td { class: "font-mono text-xs", "{t.name}" }
                                        td { class: "text-right font-mono text-xs whitespace-nowrap",
                                            "{t.kib} KiB"
                                        }
                                        td { class: "text-right text-xs text-base-content/60 whitespace-nowrap",
                                            "~{t.rows} rows"
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

/// The pointer from /perf to the history, carrying enough of the history to
/// be worth following.
///
/// A bare link would be a dead end - a reader has no way to tell whether
/// there is a day of data behind it or nothing at all. The span and the
/// sample count answer that before the click, and are the same two figures
/// the history page leads with.
#[component]
fn HistoryLink() -> Element {
    // The day, because that is what the link leads to. A longer window here
    // would advertise a span the click does not open on.
    let report = use_resource(|| async move { api::fetch_status_history("day").await });

    rsx! {
        div { class: "rounded-box border border-base-300 bg-base-100 px-4 py-3 shadow-sm",
            div { class: "flex flex-wrap items-baseline gap-x-3 gap-y-1",
                Link {
                    to: Route::PerfHistory { window: String::new() },
                    class: "link link-primary font-medium",
                    // Named for what is behind it rather than for what it is.
                    // "History" says the shape of the data and leaves the
                    // reader to guess whose - and on a page that spends every
                    // other panel separating GLPI's time from the database's,
                    // which system these counters belong to is the useful half.
                    "MariaDB"
                }
                match &*report.read_unchecked() {
                    Some(Ok(r)) if r.sampling => {
                        let h = r.history.clone().unwrap_or(api::StatusHistory {
                            hours: 0,
                            since: None,
                            until: None,
                            samples: 0,
                            span_seconds: None,
                            missing: Vec::new(),
                            series: Vec::new(),
                        });
                        let span = match (h.since.as_ref(), h.until.as_ref()) {
                            (Some(a), Some(b)) => format!("{a} to {b}"),
                            _ => "no samples yet".to_string(),
                        };
                        rsx! {
                            span { class: "text-sm text-base-content/60",
                                "{h.series.len()} counters, {h.samples} samples \u{b7} {span}"
                            }
                        }
                    }
                    // Nothing sampling, or not configured. The link still
                    // goes - the page it lands on explains why there is
                    // nothing there, which is more use than a link that
                    // disappears.
                    Some(Ok(_)) => rsx! {
                        span { class: "text-sm text-base-content/60", "not sampling yet" }
                    },
                    _ => rsx! {},
                }

                // PHP immediately after MariaDB, on the same line: the two
                // are the halves of the comparison this whole page is built
                // on - a GLPI request costs many times the query inside it -
                // and these are the kept versions of each side.
                //
                // The rule keeps them apart where the row wraps. Both
                // summaries are dotted lists of their own, so without it
                // "...15k samples" runs straight into "PHP" and reads as one
                // sentence about one thing.
                span { class: "text-base-content/30", "\u{2502}" }

                Link {
                    to: Route::PerfPhp { window: String::new() },
                    class: "link link-primary font-medium",
                    "PHP"
                }
                // Its own fetch rather than a second field on the one above:
                // different endpoint, different sampler, and one can be
                // empty while the other is full.
                PhpLinkSummary {}

                // nginx last, because it is the outermost layer and the one
                // that costs least: a request meets it first and leaves it
                // having spent almost nothing there. Reading the row left to
                // right is then the request's own order, cheapest last.
                span { class: "text-base-content/30", "\u{2502}" }

                Link {
                    to: Route::PerfNginx {},
                    class: "link link-primary font-medium",
                    "nginx"
                }
                NginxLinkSummary {}
            }

            p { class: "mt-2 text-sm text-base-content/70",
                "Everything else on this page is measured now, by an instrument that had to \
                 create the load it reports. These two are what actually ran."
            }
        }
    }
}

/// The figures beside the PHP link, or why there are none.
///
/// Split out rather than inlined because it needs its own fetch, and a
/// component is the smallest thing that can have one without the MariaDB
/// summary waiting on it.
#[component]
fn PhpLinkSummary() -> Element {
    let report = use_resource(|| async move { api::fetch_php_history("day").await });

    rsx! {
        match &*report.read_unchecked() {
            Some(Ok(r)) if r.sampling => {
                let h = r.history.clone().unwrap_or(api::StatusHistory {
                    hours: 0,
                    since: None,
                    until: None,
                    samples: 0,
                    span_seconds: None,
                    missing: Vec::new(),
                    series: Vec::new(),
                });
                // The latest reading of each endpoint rather than a sample
                // count: unlike the counters, the useful summary of a
                // response time is the response time.
                let latest = h
                    .series
                    .iter()
                    .map(|s| format!("{} {:.0}ms", s.label, s.latest))
                    .collect::<Vec<_>>()
                    .join(" \u{b7} ");
                rsx! {
                    span { class: "text-sm text-base-content/60",
                        "{latest} \u{b7} {h.samples} samples"
                    }
                }
            }
            Some(Ok(_)) => rsx! {
                span { class: "text-sm text-base-content/60", "not sampling yet" }
            },
            _ => rsx! {},
        }
    }
}

/// The history's own page.
///
/// Everything here was on /perf until the charts outgrew it. Nothing about
/// the data changed with the move - same endpoint, same sampler - only the
/// room each plot gets, which is what the info buttons and the doubled plot
/// size are for.
#[component]
pub fn PerfHistory(window: String) -> Element {
    // Matched against the four names here as well as in the backend, so the
    // strip can mark one link as current even before the response arrives.
    let at = api::history_window(&window);

    rsx! {
        div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
            Link { to: Route::Perf {}, class: "link link-primary text-sm", "\u{2190} Perf" }
            InfoButton {
                label: "What is this".to_string(),
                title: "Readings the server kept, not readings taken now".to_string(),
                body: "The perf page measures GLPI the moment it is opened, which means an \
                       instrument creating the load it reports on, against an install that \
                       is otherwise idle. This page is the opposite and the two are worth \
                       reading together: these are the server's own counters, sampled once \
                       a minute by a MariaDB EVENT into glpi_inv.status_samples and read \
                       back, so whatever actually ran - GLPI's cron, a person in the UI, an \
                       agent submitting an inventory - is still here afterwards. RATES, NOT \
                       TOTALS. Almost every counter MariaDB publishes is cumulative since \
                       startup, so its raw value only rises and says nothing about any \
                       particular minute; what is drawn is the difference between \
                       consecutive samples, per minute. The two thread counts are gauges, \
                       already a reading, and are drawn as they came. A RESTART RESETS \
                       EVERY COUNTER to zero, making that difference negative - those \
                       intervals are dropped rather than drawn as zero, because no reading \
                       exists for them and a zero would claim the server went quiet at the \
                       moment it was restarting. EACH PANEL HAS ITS OWN Y SCALE, zero to \
                       its own peak, so the shapes are comparable and the heights are not. \
                       Every panel carries its own button explaining what that counter is \
                       and when it is worth looking at."
                    .to_string(),
            }
        }

        // Between the page's own explanation and the charts, because it
        // changes what the charts are rather than adding to them.
        div { class: "mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1 text-sm",
            span { class: "text-base-content/60", "Window:" }
            for w in api::HISTORY_WINDOWS.iter() {
                Link {
                    key: "{w}",
                    class: if *w == at {
                        "link link-primary font-semibold"
                    } else {
                        "link"
                    },
                    to: Route::PerfHistory { window: (*w).to_string() },
                    "{w}"
                }
            }
        }

        StatusHistoryPanel { window: at.to_string() }
    }
}

/// GLPI's own response times, kept.
///
/// MariaDB's history and this one answer the same question about different
/// halves of the install, and the perf page's whole thesis is the ratio
/// between them: a GLPI request costs many times the query inside it. That
/// page measures both once, now. These two keep them.
///
/// WHAT MAKES THIS ONE DIFFERENT FROM ITS SIBLING, and it is worth knowing
/// before reading a gap in a line. MariaDB's counters are sampled by an
/// EVENT inside the database, so that history survives this process being
/// stopped for a week. Nothing can time GLPI except something that can call
/// it, so this history is only as continuous as this backend has been
/// running - a gap here may be GLPI being down or may be the dashboard
/// being down, and the chart cannot tell you which.
#[component]
pub fn PerfPhp(window: String) -> Element {
    let at = api::history_window(&window);

    rsx! {
        div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
            Link { to: Route::Perf {}, class: "link link-primary text-sm", "\u{2190} Perf" }
            Link {
                to: Route::PerfHistory { window: at.to_string() },
                class: "link link-primary text-sm",
                "MariaDB \u{2192}"
            }
            InfoButton {
                label: "What is this".to_string(),
                title: "How long GLPI has been taking to answer".to_string(),
                body: "Two of GLPI's own endpoints, timed once a minute and kept, so the \
                       figures on the perf page can be read as normal or not instead of \
                       just as numbers. status.php runs GLPI's health checks and index.php \
                       renders the login page - both unauthenticated and cheap, which is \
                       what makes them safe to call forever. They are a FLOOR, not a \
                       typical page: the overhead every request pays before doing anything \
                       particular, so a real list view costs this plus its own \
                       work.\n\n\
                       THESE ARE GAUGES, not counters. A response time is already a \
                       reading, so unlike the MariaDB counters nothing is differenced and \
                       a restart cannot corrupt a point.\n\n\
                       THIS SAMPLER ADDS LOAD TO WHAT IT MEASURES, which the perf page \
                       otherwise refuses to do, and that is a deliberate trade for having \
                       an overnight record at all. Two requests a minute, 2880 a day. It is \
                       the only sampler here with a dial: GLPI_INV_PHP_SAMPLE_SECONDS, 0 \
                       to stop.\n\n\
                       A GAP IS AMBIGUOUS HERE and is not on the MariaDB page. Those \
                       counters are sampled by an EVENT inside the database and survive \
                       this dashboard being stopped; nothing but this process can time \
                       GLPI, so a hole in these lines may be GLPI down or may be the \
                       dashboard down. The chart cannot distinguish them.\n\n\
                       FAILURES ARE A SEPARATE PANEL, drawn only when there has been at \
                       least one, and those samples are left out of the response time \
                       beside it: the duration of a refused connection is not a measure of \
                       speed, and averaging it in makes an outage look like things got \
                       faster."
                    .to_string(),
            }
        }

        div { class: "mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1 text-sm",
            span { class: "text-base-content/60", "Window:" }
            for w in api::HISTORY_WINDOWS.iter() {
                Link {
                    key: "{w}",
                    class: if *w == at { "link link-primary font-semibold" } else { "link" },
                    to: Route::PerfPhp { window: (*w).to_string() },
                    "{w}"
                }
            }
        }

        PhpHistoryPanel { window: at.to_string() }
    }
}

/// The PHP history, drawn by the same components as the MariaDB one.
///
/// Only the fetch differs - the backend answers both in one shape for
/// exactly this reason, so the two pages cannot drift apart in how a series
/// is rendered.
#[component]
fn PhpHistoryPanel(window: String) -> Element {
    // Same shape as StatusHistoryPanel's fetch below, and for the same
    // reason: the bucketing that makes a year drawable at sixty points is
    // the backend's, so each window is its own request.
    //
    // use_reactive! is what makes it RE-request. `window` is a plain prop,
    // not a signal, and use_resource only re-runs when a signal read inside
    // its closure changes - so cloning the prop in from outside gives the
    // resource nothing to depend on. Switching window renders this
    // component again with a new prop, but hooks persist across that, and
    // the future would never run a second time. use_reactive! declares the
    // dependency explicitly.
    let report = use_resource(use_reactive!(|window| {
        let window = window.clone();
        async move { api::fetch_php_history(&window).await }
    }));

    let numbers = use_signal(|| false);

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {},
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) if !r.configured => rsx! {
                div { class: "alert alert-info text-sm",
                    "This backend has no database URL, so it has nowhere to keep a timing. \
                     The readings on the perf page still work - they are taken live - but \
                     nothing is being recorded."
                }
            },
            Some(Ok(r)) if !r.sampling => rsx! {
                div { class: "alert alert-info text-sm",
                    "Nothing has been recorded. Either sampling is switched off with \
                     GLPI_INV_PHP_SAMPLE_SECONDS=0, or this backend has not been up long \
                     enough to take a sample - unlike the MariaDB history, nothing collects \
                     these while the dashboard is down."
                }
            },
            Some(Ok(r)) => {
                let h = r.history.clone().unwrap_or(api::StatusHistory {
                    hours: 0,
                    since: None,
                    until: None,
                    samples: 0,
                    span_seconds: None,
                    missing: Vec::new(),
                    series: Vec::new(),
                });
                rsx! { StatusHistoryBody { history: h, numbers } }
            }
        }
    }
}

/// The settings this page can change, and the ones it cannot.
///
/// It writes a Nix file and nothing else. nginx's running config is a store
/// path on a read-only mount and nginx has no runtime settings interface, so
/// unlike the MariaDB variables page there is no "apply it now" at any
/// price - the rebuild is the apply, and it is the reader's to run.
///
/// Three groups, and the third is the point. A directive already emitted by
/// one of the recommended* flags cannot be written here: nginx refuses to
/// start on a duplicate, so offering a field for it would hand the reader a
/// control whose only effect is a web server that will not come back.
#[component]
fn NginxTuning() -> Element {
    let report = use_resource(|| async move { api::fetch_nginx_tuning().await });
    // The last refusal, which is where the useful text is: which flag owns
    // the directive, or what the value should have looked like.
    let notice = use_signal(|| None::<String>);

    rsx! {
        match &*report.read_unchecked() {
            None | Some(Err(_)) => rsx! {},
            Some(Ok(r)) => {
                let r = r.clone();
                rsx! {
                    Panel {
                        title: "Settings".to_string(),
                        subtitle: if r.writable {
                            "editable - a rebuild applies them".to_string()
                        } else if r.configured {
                            "read-only in this backend".to_string()
                        } else {
                            "no tuning file configured".to_string()
                        },
                        info: "nginx's running config is a Nix store path, mode 444 on a \
                               read-only mount, and nginx has no runtime settings interface - \
                               no equivalent of SET GLOBAL. So nothing here is applied by this \
                               app. What a field does is write one line into a Nix file the \
                               system config imports; nixos-rebuild switch is what makes it \
                               real, and that is deliberately not a button.\n\n\
                               WHY SOME HAVE NO FIELD. A directive the generated config \
                               already contains cannot be added again - nginx refuses to \
                               start on a duplicate, so the field would produce a web server \
                               that does not come back. Those come from recommendedGzipSettings, \
                               recommendedOptimisation and recommendedTlsSettings, and the row \
                               names which. Changing one means turning its flag off and owning \
                               the whole block it generates, which is a bigger decision than a \
                               text field.\n\n\
                               THE OTHERS DIVIDE IN TWO. Five directives are absent from the \
                               generated config - measured, not assumed - so a tuning file may \
                               add them. Three more are present but have a NixOS option that \
                               REPLACES the generated line rather than repeating it.\n\n\
                               VALUES ARE REBUILT, NOT ECHOED. What reaches the file is \
                               assembled from the parts that matched a per-setting shape, so a \
                               value is either one of the forms that setting takes or it is \
                               refused by name."
                            .to_string(),

                        if !r.configured {
                            div { class: "mb-3 alert alert-info text-sm",
                                "No tuning file. Set GLPI_INV_NGINX_TUNING to a path the system \
                                 config imports - and git add it, because /etc/nixos is a git \
                                 repo and a flake cannot see an untracked file."
                            }
                        }
                        if let Some(n) = notice() {
                            div { class: "mb-3 alert alert-warning text-sm", "{n}" }
                        }

                        div { class: "overflow-x-auto",
                            table { class: "table table-xs",
                                thead {
                                    tr {
                                        th { "setting" }
                                        th { "wanted" }
                                        th { "goes to" }
                                    }
                                }
                                tbody {
                                    for t in r.settings.iter() {
                                        tr { key: "{t.name}",
                                            td { class: "font-mono text-xs whitespace-nowrap",
                                                // Per row, because the panel's
                                                // own modal can only describe
                                                // the three groups in general
                                                // and what a reader wants is
                                                // which one THIS setting is in.
                                                div { class: "flex items-center gap-1.5",
                                                    span { "{t.name}" }
                                                    InfoButton {
                                                        title: t.name.clone(),
                                                        body: tuning_body(t),
                                                    }
                                                }
                                            }
                                            td {
                                                if t.group == "generated" {
                                                    span { class: "text-xs text-base-content/50", "set by the flag" }
                                                } else {
                                                    TuningField {
                                                        name: t.name.clone(),
                                                        wanted: t.wanted.clone().unwrap_or_default(),
                                                        accepts: t.accepts.clone(),
                                                        enabled: r.writable,
                                                        notice,
                                                        report,
                                                    }
                                                }
                                            }
                                            td {
                                                class: if t.group == "generated" { "text-xs text-base-content/50" } else { "text-xs text-base-content/70" },
                                                if t.group == "generated" {
                                                    "{t.via}"
                                                } else {
                                                    "services.nginx.{t.via}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(path) = r.path.as_ref() {
                            p { class: "mt-2 text-xs text-base-content/60",
                                "Written to {path}. Nothing above is in effect until:"
                                code { class: "ml-1", "sudo nixos-rebuild switch" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What this dashboard costs to answer its own requests.
///
/// THE EXCEPTION TO THIS PAGE'S RULE, and the rule still holds everywhere
/// else. This page measures GLPI because an instrument reporting on itself
/// invites tuning the wrong thing - this process had used 0.01 CPU seconds
/// across its four runtime threads after nearly nine hours, and nothing in
/// this panel is a reason to tune it.
///
/// What it is for is the decomposition. A route's time here INCLUDES what it
/// waited on, so /api/rules at ~250ms is almost entirely GLPI's PHP while
/// /api/mariadb/attributes at ~2ms is a local query - two orders of
/// magnitude apart, in one table, saying which half of a page load belongs
/// to which system. The panels above make that comparison for GLPI as a
/// whole; this one makes it per route.
#[component]
fn SelfTiming() -> Element {
    let report = use_resource(|| async move { api::fetch_self_timing().await });

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {},
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) if r.routes.is_empty() => rsx! {},
            Some(Ok(r)) => rsx! { SelfTimingPanel { report: r.clone() } },
        }
    }
}

#[component]
fn SelfTimingPanel(report: api::SelfTimingReport) -> Element {
    let total: u64 = report.routes.iter().map(|r| r.count).sum();
    // The split the panel exists to show. Not computed from a list of which
    // routes call GLPI - that would be a second place to keep true - but
    // from the figures themselves: anything an order of magnitude past a
    // local query is waiting on something.
    let upstream = report.routes.iter().filter(|r| r.median_ms >= 50.0).count();

    rsx! {
        Panel {
            title: "What this dashboard costs to answer".to_string(),
            subtitle: format!(
                "{total} requests over {}",
                fmt_uptime(report.uptime_seconds)
            ),
            info: "Every other figure on this page measures GLPI. This one measures this \
                   backend, which the page otherwise deliberately avoids - an instrument \
                   reporting its own latency invites tuning the wrong thing, and this \
                   process is not the bottleneck in anything: its four runtime threads had \
                   used 0.01 CPU seconds between them after nearly nine hours of \
                   uptime.\n\n\
                   WHAT MAKES IT WORTH SHOWING, against the expectation that it would read \
                   zero: a route's time INCLUDES whatever it waited on. Measured on this \
                   install, /api/agents answers in about 440ms and /api/mariadb/plugins in \
                   about 0.8ms - two to three orders of magnitude apart, because the first \
                   spends its time inside GLPI's PHP and the second inside a local socket \
                   query. That split is the panel. The readings above compare GLPI against \
                   its database for the install as a whole; this is the same comparison \
                   per route, which is the part that says which half of any given page \
                   load belongs to which system.\n\n\
                   IT IS STILL NOT A REASON TO TUNE THIS PROCESS. The slowest route in the \
                   table costs less than a quarter of one GLPI page request, and the rows \
                   that look slow are slow because of what they are waiting for. A median \
                   at or above 50ms is coloured for exactly that reason - it marks a route \
                   that is waiting on something upstream, not one that is computing.\n\n\
                   THE CLOCK runs from after routing to the response being ready, so it \
                   excludes time on the wire, and it is attached to the api routes only - \
                   serving the wasm bundle and its assets is a file read and says nothing \
                   about this process.\n\n\
                   ROWS ARE MATCHED ROUTES, not request paths, so /api/logs/{file} stays \
                   one row however many files were tailed. Keying by the path would let a \
                   parameterised route grow a key per request, which is a leak driven by \
                   whatever a client asks for.\n\n\
                   THE FIGURES ARE A WINDOW of the last 200 requests per route, because a \
                   percentile over every request since boot describes a server that no \
                   longer exists; the count beside them is not windowed and is every \
                   request since this process started. Percentiles are nearest-rank, so \
                   every number shown is a request that actually happened rather than an \
                   interpolation between two. One oddity worth knowing: this panel's own \
                   fetch is a request, so /api/self-timing appears in its own table."
                .to_string(),

            div { class: "overflow-x-auto",
                table { class: "table table-xs",
                    thead {
                        tr {
                            th { "route" }
                            th { class: "text-right", "n" }
                            th { class: "text-right", "median" }
                            th { class: "text-right", "p95" }
                            th { class: "text-right", "max" }
                        }
                    }
                    tbody {
                        for r in report.routes.iter() {
                            tr { key: "{r.path}",
                                td { class: "font-mono text-xs", "{r.path}" }
                                td { class: "text-right text-xs text-base-content/60", "{r.count}" }
                                // Coloured on the one distinction that
                                // matters here: a route past ~50ms is
                                // waiting on something upstream, and that
                                // is not a fault of this process.
                                td {
                                    class: if r.median_ms >= 50.0 { "text-right font-mono text-xs text-warning" } else { "text-right font-mono text-xs" },
                                    "{r.median_ms:.1}"
                                }
                                td { class: "text-right font-mono text-xs text-base-content/70", "{r.p95_ms:.1}" }
                                td { class: "text-right font-mono text-xs text-base-content/70", "{r.max_ms:.1}" }
                            }
                        }
                    }
                }
            }
            p { class: "mt-2 text-xs text-base-content/60",
                if upstream > 0 {
                    "{upstream} of {report.routes.len()} routes sit above 50ms, and every one \
                     of those is waiting on GLPI or on a query rather than computing \
                     anything. Milliseconds are the whole table: the slowest route here \
                     costs less than a quarter of one GLPI page request."
                } else {
                    "Nothing here is above 50ms, so no route is waiting on anything \
                     measurable. That is the expected reading when only local routes have \
                     been called."
                }
            }
        }
    }
}

/// The history's span in words, when the window asks for materially more
/// than exists - otherwise None, and nothing is said.
///
/// The threshold is nine tenths rather than equality: a day window against
/// a day of samples is always a few minutes short of its own name, and
/// announcing that on every page view would be noise rather than a finding.
fn short_window(span_seconds: Option<u64>, window_hours: u32) -> Option<String> {
    let span = span_seconds?;
    let asked = window_hours as u64 * 3600;
    (asked > 0 && span * 10 < asked * 9).then(|| fmt_span(span))
}

/// A duration a sentence can hold: "3 hours", "1 day 13 hours".
fn fmt_span(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3600, (secs % 3600) / 60);
    match (d, h) {
        (0, 0) => format!("{m} minute{}", plural(m)),
        (0, _) => format!("{h} hour{}", plural(h)),
        (_, 0) => format!("{d} day{}", plural(d)),
        _ => format!("{d} day{} {h} hour{}", plural(d), plural(h)),
    }
}

fn plural(n: u64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Seconds as something a subtitle can hold.
fn fmt_uptime(secs: u64) -> String {
    match secs {
        s if s < 90 => format!("{s}s"),
        s if s < 5400 => format!("{}m", s / 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

/// The one part of this page that is not a reading taken now.
///
/// Everything else here measures the install at the moment the page opened,
/// which is an instrument creating the load it reports on. These panels are
/// the server's own counters, sampled every minute by a MariaDB EVENT into
/// `glpi_inv.status_samples` and read back - so an overnight spike is still
/// there in the morning, which no amount of measuring now can reach.
///
/// SMALL MULTIPLES rather than sixteen series on one chart, and that is not a
/// layout preference. Sixteen series needs sixteen distinguishable hues,
/// which do not exist - past eight, a generated hue is indistinguishable from
/// an existing one for a colourblind reader. One series per panel needs no
/// categorical palette at all: the title names the line, so there is no
/// legend and no identity carried by colour.
#[component]
fn StatusHistoryPanel(window: String) -> Element {
    // Re-fetched per window rather than filtered client-side: the bucketing
    // that makes a year drawable at sixty points is the backend's, and asking
    // it for a month of minutes to average them here would be the payload
    // this app spends its time avoiding.
    // use_reactive! for the same reason as the PHP panel above: `window` is
    // a plain prop rather than a signal, so without it the resource has no
    // dependency to re-run on and the first window fetched is the only one
    // ever shown.
    let report = use_resource(use_reactive!(|window| {
        let window = window.clone();
        async move { api::fetch_status_history(&window).await }
    }));

    // The table view the accessibility pass requires: the chart is the
    // overview, and this is where an exact figure comes from.
    let numbers = use_signal(|| false);

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {},
            Some(Err(_)) => rsx! {},
            Some(Ok(r)) if !r.configured => rsx! {},
            Some(Ok(r)) if !r.sampling => rsx! {
                Panel {
                    title: "History".to_string(),
                    subtitle: "nothing is sampling".to_string(),
                    info: "These panels read glpi_inv.status_samples, which a MariaDB EVENT \
                           fills once a minute. The backend creates both at startup and \
                           turns the event scheduler on, so reaching this state means that \
                           failed - most likely the account does not hold CREATE on the \
                           server, or this is a server the app only reads. The backend log \
                           says which. Note what it creates: glpi_inv, this app's OWN \
                           schema. It writes nothing to glpi, which is the line it does \
                           not cross whatever it needs somewhere to store readings."
                        .to_string(),
                    div { class: "text-sm text-base-content/70",
                        "No sampler. The backend tried to create one at startup and could \
                         not, so there is no history to draw and this page is back to \
                         measuring only the moment it was opened."
                    }
                }
            },
            Some(Ok(r)) => {
                let h = r.history.clone().unwrap_or(api::StatusHistory {
                    hours: 0,
                    since: None,
                    until: None,
                    samples: 0,
                    span_seconds: None,
                    missing: Vec::new(),
                    series: Vec::new(),
                });
                // Said once, above the charts, whenever the window names
                // more time than exists. Without it, picking week, month
                // and year in turn draws three identical charts with
                // nothing on the page admitting why - which is exactly how
                // it reads as the window control being broken.
                //
                // Keyed on the SPAN, not on retention. Retention is only
                // the binding limit once the sampler has been running
                // longer than it keeps; before that the limit is simply how
                // long it has been running, and blaming retention would be
                // the wrong answer for the first fortnight.
                let short = short_window(h.span_seconds, h.hours);
                rsx! {
                    if let Some(span) = short {
                        div { class: "mb-3 text-xs text-base-content/60",
                            "There is {span} of history, so this window draws all of it. \
                             Every window longer than that draws the same chart - only \
                             the ones shorter than it differ."
                            if r.kept_days > 0 && h.hours > r.kept_days * 24 {
                                " Even once there is more, the sampler keeps {r.kept_days} \
                                 days, which is this window's real ceiling."
                            }
                        }
                    }
                    StatusHistoryBody { history: h, numbers }
                }
            }
        }
    }
}

#[component]
fn StatusHistoryBody(history: api::StatusHistory, numbers: Signal<bool>) -> Element {
    let span = match (history.since.as_ref(), history.until.as_ref()) {
        // What there IS, not what was asked for. A sampler started ten
        // minutes ago has ten minutes of history, and claiming a 24 hour
        // window would be the panel's one chance to mislead.
        (Some(a), Some(b)) => format!("{a} to {b}"),
        _ => "no samples in the window".to_string(),
    };

    rsx! {
        Panel {
            title: "History".to_string(),
            subtitle: format!("{} samples, {span}", history.samples),
            info: "The server's own status counters, sampled once a minute by a MariaDB \
                   EVENT and read back here. This is the only thing on this page that \
                   survives the moment it was taken. RATES, NOT TOTALS. Almost every \
                   counter MariaDB publishes is cumulative since startup, so its raw value \
                   only ever rises and says nothing about any particular minute. What is \
                   drawn is the difference between consecutive samples, per minute. The \
                   two thread counts are gauges - already a reading - and are drawn as \
                   they came. A RESTART RESETS EVERY COUNTER to zero, which makes the \
                   difference across that boundary negative. Those intervals are dropped \
                   rather than drawn as zero: no reading exists for them, and a zero would \
                   claim the server went quiet at the exact moment it was restarting. EACH \
                   PANEL HAS ITS OWN Y SCALE, topped at its own peak, so the shapes are \
                   comparable and the heights are not - a full-height line in a panel \
                   peaking at 2 is not busier than a half-height one peaking at 900. The \
                   peak is printed on each panel for that reason. WHERE IT LIVES: \
                   glpi_inv.status_samples, this app's own schema and not GLPI's, pruned \
                   to 14 days by the same event that writes it. The backend creates the \
                   schema, the table and the event at startup if they are absent and \
                   leaves existing samples alone, so a restored datadir or a new machine \
                   starts sampling on its own rather than needing SQL run by hand."
                .to_string(),
            // In the body rather than as a Panel `action`, which would park
            // it at the far right of the header. Panel's header is
            // justify-between and shared by every page, so centring it there
            // would mean centring every other panel's controls too.
            div { class: "mb-3 flex justify-center",
                button {
                    class: "btn btn-xs",
                    onclick: move |_| {
                        let mut n = numbers;
                        let was = n();
                        n.set(!was);
                    },
                    if numbers() { "Show charts" } else { "Show numbers" }
                }
            }

            if !history.missing.is_empty() {
                div { class: "mb-3 text-xs text-warning",
                    "Expected but never sampled: {history.missing.join(\", \")}. The event\u{2019}s \
                     counter list and this app\u{2019}s have drifted apart."
                }
            }

            if history.series.is_empty() {
                div { class: "text-sm text-base-content/70",
                    "The table exists but holds nothing in the last {history.hours} hours. \
                     If the sampler was only just started, the first points arrive within \
                     a minute."
                }
            } else if numbers() {
                StatusNumbers { series: history.series.clone() }
            } else {
                // Two across rather than four. The plots are twice the size
                // they were on /perf, and at four columns that would leave
                // each one narrower than before rather than larger.
                div { class: "grid gap-x-6 gap-y-5 lg:grid-cols-2",
                    for s in history.series.iter() {
                        Sparkline { key: "{s.name}", series: s.clone() }
                    }
                }
            }
        }
    }
}

/// The table behind the charts.
///
/// Not a dump of every point - that is thousands of rows nobody reads. It is
/// the figures a reader would otherwise try to get by squinting at a 56-pixel
/// panel: what it is now, what its worst was, and how much was measured.
#[component]
fn StatusNumbers(series: Vec<api::StatusSeries>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-xs",
                thead {
                    tr {
                        th { "counter" }
                        th { class: "text-right", "now" }
                        th { class: "text-right", "peak" }
                        th { "unit" }
                        th { "readings" }
                        th { "server name" }
                    }
                }
                tbody {
                    for s in series.iter() {
                        tr { key: "{s.name}",
                            td { "{s.label}" }
                            td { class: "text-right font-mono text-xs", "{fmt_reading(s.latest)}" }
                            td { class: "text-right font-mono text-xs", "{fmt_reading(s.peak)}" }
                            td { class: "text-xs text-base-content/60", "{s.unit}" }
                            td { class: "text-xs text-base-content/60", "{s.readings}" }
                            td { class: "font-mono text-xs text-base-content/50", "{s.name}" }
                        }
                    }
                }
            }
        }
    }
}

/// One counter's panel: a title, its current reading, and the line.
///
/// The y scale is zero to this series' own peak. Zero-based deliberately -
/// a line chart whose baseline is not zero exaggerates every wobble into a
/// cliff, and these are counters where "flat at nearly nothing" is the
/// normal and important case.
#[component]
fn Sparkline(series: api::StatusSeries) -> Element {
    // Which point the pointer is nearest, and how wide the plot rendered.
    // The width is measured rather than assumed because the svg scales with
    // the column, and the pointer arrives in element pixels.
    let mut at = use_signal(|| None::<usize>);
    let mut width = use_signal(|| 0.0_f64);

    // Twice the plot these started at. W is only the coordinate space - the
    // svg stretches to its column either way - but H is real pixels, and it
    // is the one that decides whether a change of a few per cent in a
    // counter is a visible wiggle or a flat line.
    const W: f64 = 480.0;
    const H: f64 = 112.0;

    let n = series.points.len();
    // A single point is a reading, not a trend: one point draws no line, so
    // it is drawn as a dot and says so rather than rendering an empty box.
    let top = if series.peak > 0.0 { series.peak } else { 1.0 };

    let x_at = |i: usize| -> f64 {
        if n <= 1 {
            W / 2.0
        } else {
            i as f64 * W / (n - 1) as f64
        }
    };
    let y_at = |v: f64| -> f64 { H - (v / top).clamp(0.0, 1.0) * (H - 2.0) - 1.0 };

    let path = series
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "{}{:.2} {:.2}",
                if i == 0 { "M" } else { "L" },
                x_at(i),
                y_at(p.value)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    // The x axis's end ticks, formatted as a PAIR - see axis_ticks for why
    // neither can be formatted without looking at the other.
    let (first_at, last_at) = axis_ticks(
        series.points.first().map(|p| p.at.as_str()).unwrap_or(""),
        series.points.last().map(|p| p.at.as_str()).unwrap_or(""),
    );

    let hovered = at().and_then(|i| series.points.get(i).map(|p| (i, p.clone())));
    // The hovered reading replaces the current one in the header rather than
    // appearing beside it: two numbers in a 240px panel is the thing that
    // makes a sparkline unreadable.
    let (shown_value, shown_when) = match &hovered {
        Some((_, p)) => (p.value, Some(p.at.clone())),
        None => (series.latest, None),
    };

    rsx! {
        div { class: "min-w-0",
            div { class: "flex items-baseline justify-between gap-2",
                div { class: "flex min-w-0 items-center gap-1.5",
                    span { class: "truncate text-[1.17rem] font-medium leading-tight text-base-content/80", "{series.label}" }
                    // Per panel, because the thing a reader needs explained
                    // is this counter - what it counts, and when a rise in
                    // it means anything. One button on the panel above
                    // would have to carry sixteen answers.
                    match &series.note {
                        Some(note) => rsx! {
                            InfoButton {
                                title: series.label.clone(),
                                // One body, no tab strip. The project's rule
                                // (scripts/check-info-tabs.py) is that a
                                // section hides text the reader could
                                // already see; paragraphs split on a blank
                                // line and show unclicked.
                                body: format!(
                                    "{note}\n\nThe server calls this {}, and the line is {}. \
                                     Where it is a counter it is drawn as the difference \
                                     between consecutive samples rather than the raw total, \
                                     which only ever rises. The y axis runs from zero to this \
                                     counter\u{2019}s own peak in the window, so shapes are \
                                     comparable between panels and heights are not.",
                                    series.name,
                                    series.unit,
                                ),
                            }
                        },
                        // Not a silent omission: a counter arriving from a
                        // backend this build has no note for is a drift
                        // between the two lists, and saying nothing would
                        // hide it.
                        None => rsx! {
                            span {
                                class: "text-xs text-warning",
                                title: "This build has no note on this counter.",
                                "?"
                            }
                        },
                    }
                }
                span { class: "font-mono text-[1.17rem] leading-tight text-base-content", "{fmt_reading(shown_value)}" }
            }
            // The hover readout. The peak used to sit here and is now the
            // y axis's top tick, where a reader looking for the scale will
            // actually look for it.
            //
            // THE PANEL'S TYPE SCALE, all four sizes arbitrary values rather
            // than Tailwind steps, because what matters is the ratios
            // between them and the steps do not land on them:
            //
            //   1.17rem  counter name, current value   (was 0.875)
            //   0.87rem  this hover readout            (was 0.65)
            //   0.8rem   y axis name, ticks, x axis    (was 0.6)
            //
            // The smallest were doubled first, which left them larger than
            // the name and the value and inverted the panel - the ticks
            // read as the headline and the counter as the caption. The name
            // and value are scaled by the same factor to put that back, so
            // the three ranks hold the 1.35 : 1.0 : 0.92 relation they had
            // at the original size. Change one and the others have to move -
            // which is why the later pass that took a third off did it to
            // all three at once (x2/3, rounded to 2dp), leaving the ratios
            // intact and the whole panel at 1.33x where it started.
            div { class: "mb-1 truncate text-[0.87rem] text-base-content/50",
                match &shown_when {
                    Some(w) => w.clone(),
                    None => "latest".to_string(),
                }
            }

            div { class: "flex items-stretch gap-1",
                // Y AXIS NAME. Rotated so it reads bottom-to-top, which is
                // the convention for a vertical axis title. In HTML and not
                // in the svg: preserveAspectRatio is none, so the plot
                // scales x and y by different factors and any text inside
                // it would be stretched by whatever the column width
                // happens to be. Strokes survive that via
                // non-scaling-stroke; glyphs have no equivalent.
                div {
                    class: "flex shrink-0 items-center [writing-mode:vertical-rl] rotate-180 text-[0.8rem] leading-none text-base-content/50",
                    "{series.unit}"
                }
                // Y AXIS TICKS. Two, at the ends of the scale, because the
                // scale is zero-based and topped at this series' own peak -
                // so those two numbers describe it completely and anything
                // between them is arithmetic the reader can do.
                div {
                    class: "flex shrink-0 flex-col justify-between text-right text-[0.8rem] leading-none text-base-content/40 tabular-nums",
                    style: "height: {H}px",
                    span { "{fmt_reading(series.peak)}" }
                    span { "0" }
                }

                div { class: "min-w-0 flex-1",
                    svg {
                        class: "w-full",
                        height: "{H}",
                        view_box: "0 0 {W} {H}",
                        preserve_aspect_ratio: "none",
                        onmounted: move |e| async move {
                            if let Ok(r) = e.get_client_rect().await {
                                width.set(r.size.width);
                            }
                        },
                        onmousemove: move |e| {
                            let w = width();
                            if w <= 0.0 || n == 0 {
                                return;
                            }
                            let frac = (e.element_coordinates().x / w).clamp(0.0, 1.0);
                            at.set(Some(((frac * (n.saturating_sub(1)) as f64).round() as usize).min(n - 1)));
                        },
                        onmouseleave: move |_| at.set(None),

                        // Baseline only. One hairline, one step off the surface - a
                        // grid behind a 56px panel is noise, and the peak is printed
                        // above instead.
                        line {
                            x1: "0",
                            y1: "{H - 1.0}",
                            x2: "{W}",
                            y2: "{H - 1.0}",
                            stroke: "var(--viz-grid)",
                            stroke_width: "1",
                            // The svg scales non-uniformly to fill its column, which
                            // would stretch strokes with it. This keeps every stroke
                            // at its stated width in screen pixels.
                            vector_effect: "non-scaling-stroke",
                        }

                        // One point is a reading, not a trend: there is no line to
                        // draw, and the end marker below is the whole chart.
                        if n > 1 {
                            path {
                                d: "{path}",
                                fill: "none",
                                stroke: "var(--viz-line)",
                                stroke_width: "2",
                                stroke_linejoin: "round",
                                stroke_linecap: "round",
                                vector_effect: "non-scaling-stroke",
                            }
                        }

                        // Markers are vertical TICKS, not dots, and that is forced
                        // by the scaling. preserveAspectRatio is none so the plot
                        // fills whatever column it lands in, which scales x and y by
                        // different factors - and under that, a circle renders as an
                        // ellipse whose eccentricity depends on the viewport. A
                        // vertical segment has no horizontal extent to distort, and
                        // non-scaling-stroke holds its width at 2px, so it is the
                        // one marker shape that is the same in every column width.
                        if let Some(p) = series.points.last() {
                            line {
                                x1: "{x_at(n - 1):.2}",
                                y1: "{(y_at(p.value) - 3.0).max(0.0):.2}",
                                x2: "{x_at(n - 1):.2}",
                                y2: "{(y_at(p.value) + 3.0).min(H):.2}",
                                stroke: "var(--viz-line)",
                                stroke_width: "3",
                                stroke_linecap: "round",
                                vector_effect: "non-scaling-stroke",
                            }
                        }

                        if let Some((i, p)) = &hovered {
                            // Crosshair first, so the value tick sits on top of it.
                            line {
                                x1: "{x_at(*i):.2}",
                                y1: "0",
                                x2: "{x_at(*i):.2}",
                                y2: "{H}",
                                stroke: "var(--viz-grid)",
                                stroke_width: "1",
                                vector_effect: "non-scaling-stroke",
                            }
                            line {
                                x1: "{x_at(*i):.2}",
                                y1: "{(y_at(p.value) - 4.0).max(0.0):.2}",
                                x2: "{x_at(*i):.2}",
                                y2: "{(y_at(p.value) + 4.0).min(H):.2}",
                                stroke: "var(--viz-line)",
                                stroke_width: "4",
                                stroke_linecap: "round",
                                vector_effect: "non-scaling-stroke",
                            }
                        }
                    }
                }
            }

            // X AXIS: its two ticks are the ends of the window, and its
            // name sits between them.
            div {
                class: "mt-0.5 flex items-baseline justify-between gap-1 text-[0.8rem] leading-none text-base-content/40 tabular-nums",
                span { "{first_at}" }
                span { class: "text-base-content/50", "time" }
                span { "{last_at}" }
            }
        }
    }
}

/// The two x-axis tick labels, from the first and last stamps.
///
/// A PAIR, because neither can be formatted correctly on its own. These were
/// clock time only, on the reasoning that the date was already in the panel
/// header and repeating it in every column bought nothing. That is wrong for
/// the window this draws: 24 hours rolling crosses midnight nearly always,
/// so a span of 2026-11-21 05:49 to 2026-11-22 05:24 rendered as
/// "05:49 ... 05:24" - twenty-five minutes BACKWARDS, on an axis running a
/// full day forwards. It read as a broken plot and was a broken label.
///
/// So the date appears whenever the two ends fall on different days, and on
/// BOTH ticks when it appears: one labelled "11-21 18:48" beside a bare
/// "05:24" moves the ambiguity rather than removing it.
///
/// Sliced rather than parsed - the backend writes these in a fixed format and
/// the only thing wanted is a substring. Anything not that shape comes back
/// whole, so a changed format shows up as an over-long tick rather than as a
/// blank axis.
fn axis_ticks(first: &str, last: &str) -> (String, String) {
    let same_day = match (first.get(..10), last.get(..10)) {
        (Some(a), Some(b)) => a == b,
        // One of them is not a stamp this understands. Show everything
        // rather than guess which part is safe to drop.
        _ => false,
    };
    (tick(first, same_day), tick(last, same_day))
}

/// One tick: `HH:MM`, or `MM-DD HH:MM` when the window spans more than a day.
fn tick(at: &str, same_day: bool) -> String {
    let (Some(date), Some(time)) = (at.get(5..10), at.get(11..16)) else {
        return at.to_string();
    };
    if same_day {
        time.to_string()
    } else {
        format!("{date} {time}")
    }
}

/// A reading, at a width a 240px panel can hold.
///
/// Counters here span "0.02 queries a minute" to "thousands of buffer pool
/// reads", and a fixed number of decimals is unreadable at one end or lossy
/// at the other.
fn fmt_reading(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.0}", v)
    } else if v >= 10.0 {
        format!("{:.1}", v)
    } else if v > 0.0 {
        format!("{:.2}", v)
    } else {
        "0".to_string()
    }
}

/// What the server itself thinks was slow.
///
/// The only thing on this page that is a RECORD rather than a reading. Every
/// other figure here is taken now, against an idle install, by an instrument
/// that had to create the load it measured. This one is whatever actually ran
/// - GLPI's own cron, a browser session, an agent submitting an inventory -
/// caught at the moment it was slow, which is the moment nothing is watching.
///
/// Two fetches, and the split is the point. The settings are four variables
/// and ride with the page. The entries are a full scan of `mysql.slow_log`,
/// which is a CSV table and carries no index, so they are a button: a page
/// that scanned a growing log on every open would become the thing this page
/// exists to notice.
#[component]
fn SlowLog() -> Element {
    let settings = use_resource(|| async move { api::fetch_slow_log().await });

    // The result of the button, and whether it is in flight. Not a
    // use_resource: that would run on mount, which is exactly what the split
    // above is avoiding.
    let run = use_signal(|| None::<Result<api::SlowLogEntriesReport, api::ApiError>>);
    let busy = use_signal(|| false);

    rsx! {
        match &*settings.read_unchecked() {
            None => rsx! {},
            // Quiet, like RuleLoad: this page is about the timings, and a
            // database it cannot reach should not read as the measurements
            // having failed.
            Some(Err(_)) => rsx! {},
            Some(Ok(r)) if !r.configured => rsx! {},
            Some(Ok(r)) => {
                let st = r.settings.clone().unwrap_or(api::SlowLogSettings {
                    enabled: false,
                    threshold_seconds: 0.0,
                    log_output: String::new(),
                    to_table: false,
                    to_file: false,
                    file: String::new(),
                });
                rsx! { SlowLogPanel { settings: st, run, busy } }
            }
        }
    }
}

/// The tile, its button, and what the button found.
#[component]
fn SlowLogPanel(
    settings: api::SlowLogSettings,
    run: Signal<Option<Result<api::SlowLogEntriesReport, api::ApiError>>>,
    busy: Signal<bool>,
) -> Element {
    let st = settings.clone();

    let subtitle = if !st.enabled {
        "off - the server is recording nothing".to_string()
    } else {
        format!(
            "on, over {}s \u{2192} {}",
            trim_seconds(st.threshold_seconds),
            st.log_output
        )
    };

    rsx! {
        Panel {
            title: "What the server logged as slow".to_string(),
            subtitle,
            info: "Every other figure on this page is taken now, by an instrument that had                    to create the load it measured. This one is a record: whatever actually                    ran and took longer than long_query_time, caught at the moment it was                    slow. READ rows examined AGAINST rows sent. The duration says a query                    was slow; that ratio says why - fifty thousand rows read to return                    three is a missing index, and no amount of server tuning fixes it.                    WHERE IT COMES FROM. log_output is a SET, so the server can write both                    a file and a table. This reads mysql.slow_log, the table, because the                    file lives in a datadir no ordinary account can enter. With log_output                    at FILE alone the table is empty BY DESIGN, and the button says that                    rather than reporting nothing slow. WHAT IT IS NOT is a digest. It                    shows the slowest twenty, not the costliest pattern - a query running                    200ms ten thousand times outranks one 3s outlier and will not appear                    here at all. Past a few thousand rows the answer is pt-query-digest                    over the file copy. The table is also a CSV one, which cannot be                    indexed and is never rotated, so every run below is a full scan and                    TRUNCATE is the way to shorten it."
                .to_string(),

            if !st.enabled {
                div { class: "text-sm text-base-content/70",
                    "The server is logging nothing, so there is nothing to read.                      slow_query_log is OFF - it is a dynamic variable, so the variables                      page can turn it on without a restart."
                }
            } else {
                div { class: "flex flex-wrap items-center gap-3",
                    button {
                        class: "btn btn-sm btn-primary",
                        disabled: busy(),
                        onclick: move |_| {
                            let mut run = run;
                            let mut busy = busy;
                            busy.set(true);
                            run.set(None);
                            spawn(async move {
                                let r = api::fetch_slow_log_entries().await;
                                run.set(Some(r));
                                busy.set(false);
                            });
                        },
                        if busy() {
                            span { class: "loading loading-spinner loading-xs mr-2" }
                            "Scanning\u{2026}"
                        } else {
                            "Show the slowest queries"
                        }
                    }
                    // Said on the button's own line rather than only in the
                    // modal: the cost is the reason this is a button.
                    span { class: "text-xs text-base-content/60",
                        "Reads mysql.slow_log, worst first. A full scan - the table has no index."
                    }
                }

                if st.to_file {
                    p { class: "mt-2 text-xs text-base-content/60",
                        "A file copy is also being written, to {st.file} - relative names are                          relative to the datadir, which is mode 0700. That copy is what                          pt-query-digest would read."
                    }
                }

                match &*run.read() {
                    None => rsx! {},
                    Some(Err(e)) => rsx! {
                        div { class: "mt-3", FetchError { error: e.clone() } }
                    },
                    Some(Ok(r)) if !r.to_table => rsx! {
                        div { class: "mt-3 alert alert-info text-sm",
                            "log_output does not include TABLE, so mysql.slow_log is empty                              whatever the server is logging. That is not an absence of slow                              queries - the record is going to the file only."
                        }
                    },
                    Some(Ok(r)) => rsx! {
                        SlowLogRows {
                            entries: r.entries.clone().unwrap_or(api::SlowLogEntries {
                                total: 0,
                                entries: Vec::new(),
                            }),
                            limit: r.limit,
                            threshold: st.threshold_seconds,
                        }
                    },
                }
            }
        }
    }
}

/// What the scan found.
#[component]
fn SlowLogRows(entries: api::SlowLogEntries, limit: u32, threshold: f64) -> Element {
    if entries.entries.is_empty() {
        return rsx! {
            div { class: "mt-3 text-sm text-base-content/70",
                "The log is empty: nothing has taken longer than {trim_seconds(threshold)}s since                  logging was turned on. On a quiet install that is the expected answer, and it                  says the threshold is catching nothing rather than that the server is fast -                  lower it to see anything at all."
            }
        };
    }

    rsx! {
        div { class: "mt-3 overflow-x-auto",
            table { class: "table table-xs",
                thead {
                    tr {
                        th { "when" }
                        th { class: "text-right", "seconds" }
                        th { class: "text-right", "lock" }
                        th { class: "text-right", "sent" }
                        th { class: "text-right", "examined" }
                        th { "db" }
                        th { "query" }
                    }
                }
                tbody {
                    for (i , e) in entries.entries.iter().enumerate() {
                        tr { key: "{i}-{e.start_time}",
                            td { class: "whitespace-nowrap text-xs text-base-content/60", "{e.start_time}" }
                            td { class: "text-right font-mono text-xs", "{e.seconds:.3}" }
                            td {
                                class: if e.lock_seconds > 0.0 { "text-right font-mono text-xs text-warning" } else { "text-right font-mono text-xs text-base-content/40" },
                                "{e.lock_seconds:.3}"
                            }
                            td { class: "text-right font-mono text-xs", "{e.rows_sent}" }
                            // The one number worth colouring. A query reading
                            // far more than it returns is a missing index,
                            // and that is a different fix from a slow server.
                            td {
                                class: if e.rows_examined > 1000 && e.rows_examined > e.rows_sent.saturating_mul(100) { "text-right font-mono text-xs text-warning font-semibold" } else { "text-right font-mono text-xs" },
                                "{e.rows_examined}"
                            }
                            td { class: "text-xs", "{e.db}" }
                            td { class: "font-mono text-xs", "{e.sql_text}" }
                        }
                    }
                }
            }
        }
        p { class: "mt-2 text-xs text-base-content/60",
            if entries.total > limit as u64 {
                "The slowest {limit} of {entries.total} logged. Past a few thousand the                  question stops being which single query was worst and becomes which                  pattern costs the most, which is a digest tool rather than a list."
            } else {
                "All {entries.total} logged. Queries are truncated at 400 characters."
            }
        }
    }
}

/// 0.5 rather than 0.500000, and 10 rather than 10.000000.
///
/// The server reports long_query_time as a double with six decimals, which
/// reads as a precision the setting does not have.
fn trim_seconds(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// What a rule chain will cost, counted rather than timed.
///
/// The one thing on this page that is not a stopwatch reading, and it is here
/// because no stopwatch can reach it: GLPI's own profiler is instrumented in
/// 174 places and none of them is in RuleCollection, Rule or RuleTicket, so
/// rule processing has no timing anywhere - not in a log, not in a counter,
/// not in the debug panel. It is undifferentiated PHP inside whichever request
/// created the ticket. What CAN be read is how much work a ticket will ask
/// for, which is the leading indicator the timing would have been.
#[component]
fn RuleLoad() -> Element {
    let report = use_resource(|| async move { api::fetch_rules().await });

    rsx! {
        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-sm text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Counting GLPI\u{2019}s rules\u{2026}"
                }
            },
            // Quietly absent rather than an error block: this page's subject
            // is the timings, and a profile that cannot read rules should not
            // make it look as though the measurements failed.
            Some(Err(_)) => rsx! {},
            Some(Ok(r)) if !r.available => rsx! {},
            Some(Ok(r)) => rsx! { RuleLoadPanel { report: r.clone() } },
        }
    }
}

/// GLPI's own right name for a rule type, for naming what a 403 is missing.
///
/// Read off each rule class's `$rightname` - RuleAsset.php:39 for the one this
/// install refuses - rather than derived from the type, because they do not
/// follow from it: RuleRight's right is `rule_ldap`, not `rule_right`. Unknown
/// types fall back to the type itself, which is still better than a page that
/// says a permission is missing without saying which.
fn right_for(rule_type: &str) -> &str {
    match rule_type {
        "Asset" => "rule_asset",
        "Ticket" => "rule_ticket",
        "Change" => "rule_change",
        "Problem" => "rule_problem",
        "Right" => "rule_ldap",
        other => other,
    }
}

/// The two figures, and what makes a count the signal here.
#[component]
fn RuleLoadPanel(report: api::RulesReport) -> Element {
    // Every collection GLPI evaluates in full, not only the ITIL three. The
    // set is the backend's and comes off the collection classes: these are
    // the ones that do not stop at the first match, so their size is a linear
    // cost. Import rules are deliberately absent - they stop, so adding them
    // here would inflate a figure that is about work actually done.
    let full: Vec<&api::RuleCollection> = ["Ticket", "Change", "Problem", "Right"]
        .iter()
        .filter_map(|t| report.collection(t))
        .collect();
    // Active only. An inactive rule is a row that fires on nothing, and
    // counting it would overstate the work this figure exists to state.
    let active: Vec<&api::RuleSummary> = full
        .iter()
        .flat_map(|c| c.rules.iter())
        .filter(|r| r.is_active)
        .collect();
    let criteria: usize = active.iter().map(|r| r.criteria).sum();
    // Named, so the figure cannot be read as being about tickets alone. Only
    // the ones that actually have an active rule - a collection contributing
    // nothing is not part of the cost.
    let carrying = full
        .iter()
        .filter(|c| c.active > 0)
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .join(", ");

    let import = report.collection("ImportAsset");

    rsx! {
        Panel {
            title: "What a rule chain will cost".to_string(),
            subtitle: "Counted, because nothing times it".to_string(),
            info: "Some of GLPI's rule collections are evaluated in full and the rest \
                   stop at the first match, and only the first kind has a cost that \
                   grows with the list. RuleCollection leaves stop_on_first_match false, \
                   and five collections do not override it: tickets, changes, problems, \
                   assets and authorizations. In those, every active rule is tested on \
                   every write the collection watches - a ticket create or update, an \
                   asset add or update, a login - and every match applies its actions. \
                   Each collection has its own trigger, so the figures are a per-event \
                   worst case rather than one number paid by everything. The work is \
                   spent inside the request doing the write, on top of the PHP floor the \
                   readings above measure. WHAT AT MOST MEANS. The criteria figure is a \
                   ceiling rather than a count of what runs. Rule::checkCriterias \
                   short-circuits in both directions - with AND matching it stops at the \
                   first criterion that fails, with OR matching at the first that \
                   succeeds - so a rule carrying six criteria usually tests one or two. \
                   The number is the worst case, which is the right thing to watch and \
                   the wrong thing to multiply. WHAT IT CANNOT TELL YOU is what a \
                   criterion costs. Some compare a field already in memory and are \
                   measured in microseconds; others reach for related data - both ticket \
                   rules this install ships test _locations_id_of_item, the location of \
                   the affected item, which is a lookup. Twenty criteria might be noise \
                   or might be a handful of queries, and a count cannot distinguish \
                   them. Turning this figure into milliseconds would mean inventing a \
                   cost per criterion that does not exist. WHAT IT IS FOR, then, is \
                   excluding rules as a suspect. At zero, rule processing cannot be \
                   contributing anything to the latency of a write - no measurement \
                   needed and nothing to tune. Against a floor of some twenty \
                   milliseconds of PHP before GLPI does anything at all, a small figure \
                   says look elsewhere; a figure climbing into the hundreds says look \
                   here first, and only then is it worth timing an actual create. Where \
                   a collection is refused, as the asset rules are for this account, the \
                   figure is short by whatever is active in it and says so rather than \
                   reading as a zero. WHY A COUNT AND NOT A TIMING. GLPI attributes no \
                   time to rule processing anywhere: its profiler is instrumented in 174 \
                   places and none of them is in RuleCollection, Rule or RuleTicket. The \
                   database cannot see it either, because the cost is many fast queries \
                   plus PHP rather than one slow statement. The input is the only honest \
                   instrument available. The escape hatch for a chain that does get long \
                   is a rule action rather than a setting - GLPI offers Skip remaining \
                   rules on exactly the collections that do not stop at the first match, \
                   which is the same five."
                .to_string(),

            div { class: "flex flex-wrap items-baseline gap-x-8 gap-y-2",
                div {
                    div { class: "text-2xl font-semibold", "{active.len()}" }
                    div { class: "text-xs text-base-content/60",
                        "active rules evaluated in full"
                    }
                }
                div {
                    div { class: "text-2xl font-semibold", "{criteria}" }
                    div { class: "text-xs text-base-content/60",
                        "criteria per matching event, at most"
                    }
                }
            }

            p { class: "mt-3 max-w-prose text-sm",
                if active.is_empty() {
                    "Nothing is active in any of them, so no write pays for this mechanism \
                     today. The figure is worth watching rather than reading once: it is \
                     the only warning available before the cost is felt."
                } else {
                    "Each of those rules is tested on every write its collection watches, \
                     with no stopping at the first match, so this is work the write pays \
                     before anything it asked for. Not one event either - a ticket create \
                     or update, an asset add or update, a login - each collection has its \
                     own trigger and only its own rules run."
                }
            }

            if !carrying.is_empty() {
                p { class: "mt-2 max-w-prose text-xs text-base-content/60",
                    "Carried by: {carrying}."
                }
            }

            if let Some(c) = import {
                p { class: "mt-2 max-w-prose text-sm text-base-content/70",
                    "For contrast, the {c.total} import rules do not scale this way: that \
                     collection stops at the first match, so GLPI walks it in rank order \
                     and quits as soon as one claims the submission. A long import list \
                     costs what it takes to reach the match, not the whole list."
                }
            }

            for t in report.refused.iter() {
                div { key: "{t}", class: "mt-3 alert alert-warning py-2 text-sm",
                    div {
                        span { class: "font-medium", "{t} rules are not counted above. " }
                        "GLPI answers this collection with 403 for the account this app \
                         uses: the route is there and the profile lacks READ on "
                        span { class: "font-mono text-xs", "{right_for(t)}" }
                        ". The rules exist and run on the install - "
                        if t == "Asset" {
                            "these fire on every asset add and update, which includes the \
                             inventory writes this app watches - "
                        }
                        "so this reading is short by however many of them are active. \
                         Granting that right to the API profile is what would close it; \
                         nothing here guesses the count from elsewhere."
                    }
                }
            }

            p { class: "mt-3 text-xs text-base-content/60",
                "The rules themselves, per collection and in rank order, are on "
                Link {
                    class: "link link-primary",
                    to: Route::Db {
                        tab: "rules".to_string(),
                        sub: String::new(),
                    },
                    "db \u{00b7} rules"
                }
                "."
            }
        }
    }
}

/// One figure in a strip, with its explanation attached to it.
///
/// This page's panels are strips of figures rather than tables, and a figure
/// in a strip is exactly the kind of thing a reader guesses at - is it a
/// limit or a reading, is it per worker or in total. So each carries its own
/// modal, the same rule `StatTile` follows for the dashboard tiles. Not
/// `StatTile` itself: these read value-above-label inside a panel that
/// already has a border, where a daisyUI stat cell brings its own.
#[component]
fn Figure(
    value: String,
    label: String,
    body: String,
    /// Colour the figure only when it means something is wrong - `StatTile`'s
    /// rule, for the same reason: a red zero trains the reader to ignore the
    /// colour, and then the one that matters does not register.
    #[props(default = false)]
    alarm: bool,
) -> Element {
    // Cloned before the macro rather than inside it: the label is also
    // formatted into the markup below, and the prop would move it.
    let title = label.clone();

    rsx! {
        div {
            div {
                class: if alarm { "text-2xl font-semibold text-error" } else { "text-2xl font-semibold" },
                "{value}"
            }
            // The button sits on the label line, not beside the number: the
            // figure should stay the thing the eye lands on.
            div { class: "flex items-center gap-1.5 text-xs text-base-content/60",
                span { "{label}" }
                InfoButton { title, body }
            }
        }
    }
}

/// What a directive does, past the one line already printed beside it.
///
/// Keyed by name, not by position: the list of directives is the app's to
/// send and its order is not promised. A name with no entry here gets no
/// button rather than an empty one - a modal is a promise of more than the
/// row already says, and an unkept one is worse than none.
const NGINX_DIRECTIVES: [(&str, &str); 16] = [
    (
        "worker_processes",
        "auto resolves to one per core at startup, which is why the figure above can differ \
         from what the file asks for. Each worker is single-threaded and event-driven, so \
         one of them handles thousands of connections and more than one per core buys \
         nothing but context switches. It is read at startup only: a reload will not move \
         it.",
    ),
    (
        "worker_connections",
        "The ceiling on simultaneous connections one worker will hold, idle keepalive ones \
         included - which is what keepalive_timeout trades against. Raising it without \
         raising worker_rlimit_nofile swaps a connection limit for \"too many open files\", \
         because every connection is also a descriptor.",
    ),
    (
        "worker_rlimit_nofile",
        "Every connection, every open file and every log handle is one descriptor, so this \
         has to clear worker_connections with room to spare - twice it is the usual rule \
         for something proxying. Left unset, nginx inherits whatever limit the service \
         manager gave it, which is not a number the capacity figures above know about.",
    ),
    (
        "http2",
        "Multiplexing removes head-of-line blocking between a page's assets, so the gain \
         tracks latency: largest over a slow link, near nothing on this host. Every browser \
         requires TLS for it, so turning it on without a certificate changes nothing one \
         will do. Older nginx spelled it listen ... http2, which is why guides disagree.",
    ),
    (
        "open_file_cache",
        "Keeps descriptors and stat results for a time you choose, so a repeat request skips \
         the syscalls rather than looking the file up again. What it costs is staleness: a \
         file replaced inside that window is still served from the old descriptor. On a \
         document root that is a store path nothing is replaced in place, which makes it \
         cheap here.",
    ),
    (
        "gzip_types",
        "text/html is always compressed and is never in this list - naming it is the common \
         mistake. Everything else is opt in by MIME type, so whether a response is \
         compressed depends on the Content-Type nginx gave it, which is the type printed on \
         each timing card above.",
    ),
    (
        "gzip",
        "Spends processor time to buy bandwidth, so it wins wherever the link is slower than \
         the processor - which is every link that leaves this host. It does nothing for \
         images or anything already compressed, and nothing at all for a type gzip_types \
         does not name.",
    ),
    (
        "gzip_comp_level",
        "1 to 9, and the curve flattens early on text: most of the saving is already there \
         at level 1 and the rest is paid per response, every response. gzip_static avoids \
         the trade entirely for files that can be compressed once, ahead of time.",
    ),
    (
        "keepalive_timeout",
        "The idle seconds before nginx closes a connection it could have reused. Long holds \
         a worker connection open for nothing; short pays the setup again. Whether the \
         current value is buying anything is a measurement rather than a guess - it is the \
         requests-per-connection figure, and it needs a stub_status location to exist.",
    ),
    (
        "sendfile",
        "The file goes kernel to socket without passing through nginx's own memory, saving a \
         copy per response. It applies to files on disk only: nothing coming back from \
         php-fpm takes that path, so it has no effect on a GLPI page.",
    ),
    (
        "tcp_nopush",
        "Holds a response back until a full packet's worth is ready, so a sendfile response \
         is not dribbled out in fragments. It needs sendfile on to do anything at all, and \
         it adds nothing to a proxied response.",
    ),
    (
        "client_max_body_size",
        "nginx refuses the request before PHP is started, so PHP's own upload limits are \
         never reached and nothing in GLPI's log accounts for the loss. The agent reports a \
         failure and GLPI records nothing. A machine with a long software list is where this \
         is met first.",
    ),
    (
        "fastcgi_read_timeout",
        "The clock runs between two successive reads from php-fpm, not across the whole \
         request. On a timeout the PHP worker is still working and will finish, so a long \
         import looks failed in the browser and succeeds underneath it. Set it against \
         PHP's own max_execution_time rather than by feel.",
    ),
    (
        "ssl_protocols",
        "1.3 finishes its handshake in one round trip where 1.2 needs two, which is what \
         puts it on a performance page. Offering only 1.3 locks out old clients; 1.0 and \
         1.1 are what there is to remove, and this build does not offer them anyway.",
    ),
    (
        "ssl_session_cache",
        "shared:SSL:10m is ten megabytes shared by every worker - roughly forty thousand \
         sessions, not ten minutes; the lifetime is ssl_session_timeout. Without a shared \
         cache each worker remembers alone, and a client landing on a different worker \
         starts the handshake over.",
    ),
    (
        "server_tokens",
        "off drops the version from the Server header and from nginx's own error pages, \
         leaving \"nginx\". It does not hide that nginx is there. The version this page \
         shows is read from the binary rather than from a header, so turning it off does \
         not blind the page.",
    ),
];

/// What a build flag buys, past the line already beside it.
const NGINX_MODULES: [(&str, &str); 4] = [
    (
        "--with-http_stub_status_module",
        "Compiled in here, so the counters already exist and what is missing is only a place \
         to read them: a location containing stub_status, restricted to localhost. Seven \
         numbers is all it offers - no breakdown by vhost, path or status code, because \
         nginx keeps none.",
    ),
    (
        "--with-http_v2_module",
        "Present, so http2 on is a config change rather than a different nginx. Where it is \
         absent the directive is not ignored, it is a startup error - which is the thing \
         worth knowing before editing the settings below.",
    ),
    (
        "--with-http_v3_module",
        "QUIC runs over UDP and carries its own encryption, so a firewall rule for 443/tcp \
         does not cover it and the TLS reasoning on this page does not transfer. Nothing \
         here uses it; it is listed because its presence is what decides whether the option \
         exists at all.",
    ),
    (
        "--with-http_gzip_static_module",
        "Looks for file.gz beside file and sends that instead, so compression is paid once \
         ahead of time rather than per response. It needs the .gz to be there: a document \
         root built without them is served exactly as before, with nothing to say why.",
    ),
];

/// What a timing is, past the line already under it.
///
/// Keyed on the probe's own name, which is also what `NginxBody` matches on
/// to pair the static reading against the PHP one.
const NGINX_PROBES: [(&str, &str); 3] = [
    (
        "static file",
        "The URL is chosen when the reading is taken, from a list of assets GLPI ships - the \
         first that answers 200 is the one timed. Five requests, keepalive off, body \
         included; the median is the figure and the best is beside it because the gap \
         between them is scheduling noise rather than anything about nginx.",
    ),
    (
        "through php-fpm",
        "status.php, which is GLPI's lightest endpoint - so this is close to the floor for a \
         PHP response and not a typical page. Take the static reading off it and what is \
         left is the FastCGI round trip plus GLPI's bootstrap. A real page adds its own \
         queries on top of all of it.",
    ),
    (
        "a path that is not there",
        "The 404 is deliberate and is not nginx refusing anything: GLPI's vhost ends in \
         try_files $uri /index.php, so a path that is not a file on disk is answered by PHP. \
         The bytes are a fully rendered error page. One wrong asset reference therefore buys \
         a GLPI bootstrap on every page view that includes it.",
    ),
];

/// The body for one of the tables above, or none if nothing was written for
/// that name.
fn lookup(table: &[(&str, &'static str)], name: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

/// Why this setting has the control it has, in its own terms.
///
/// Built from the row rather than written out sixteen times, because what a
/// reader cannot get from the table is which of the three groups THIS
/// setting is in and what that costs - and the group, the route and the
/// accepted shape are all in the row already.
fn tuning_body(t: &api::NginxTunable) -> String {
    match t.group.as_str() {
        "generated" => format!(
            "No field, and not because this one is dangerous: {} already emits this \
             directive, nginx refuses to start on a duplicate, and a second line is all a \
             field here could write. Changing it means turning that flag off and owning \
             every directive it generates, which is a decision rather than an edit.",
            t.via
        ),
        "option" => format!(
            "The generated config does set this, but services.nginx.{} REPLACES that line \
             instead of adding a second one, so it is safe to write. Accepts {}. Nothing \
             happens until the rebuild - this app writes the file and stops there.",
            t.via, t.accepts
        ),
        _ => format!(
            "Absent from the generated config - measured, not assumed - so the tuning file \
             may add it outright, through services.nginx.{}. Accepts {}; an empty box \
             removes the line rather than writing an empty value. Nothing happens until the \
             rebuild.",
            t.via, t.accepts
        ),
    }
}

/// The figures beside the nginx link.
///
/// The two timings rather than the version: what a reader wants to know
/// before clicking is whether the web server is costing anything, and the
/// static reading against the PHP one answers that in two numbers.
#[component]
fn NginxLinkSummary() -> Element {
    let report = use_resource(|| async move { api::fetch_nginx().await });

    rsx! {
        match &*report.read_unchecked() {
            Some(Ok(r)) if r.running => {
                let fig = r
                    .probes
                    .iter()
                    .filter_map(|p| p.median_ms.map(|ms| format!("{} {:.1}ms", p.what, ms)))
                    .collect::<Vec<_>>()
                    .join(" \u{b7} ");
                let version = r.version.clone().unwrap_or_else(|| "running".to_string());
                rsx! {
                    span { class: "text-sm text-base-content/60", "{version} \u{b7} {fig}" }
                }
            }
            Some(Ok(_)) => rsx! {
                span { class: "text-sm text-base-content/60", "no nginx process" }
            },
            _ => rsx! {},
        }
    }
}

/// The web server's own page.
///
/// Sibling of /perf/php and /perf/history, and the only one of the three with
/// no window: nothing here is sampled over time, because nginx keeps no
/// counters this app can reach. What it has instead is a comparison - what a
/// file costs against what a PHP page costs, through the same server - and
/// the settings that decide what a request pays before PHP sees it.
#[component]
pub fn PerfNginx() -> Element {
    let report = use_resource(|| async move { api::fetch_nginx().await });

    rsx! {
        div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
            Link { to: Route::Perf {}, class: "link link-primary text-sm", "\u{2190} Perf" }
            InfoButton {
                label: "What is this".to_string(),
                title: "The layer in front of PHP, read from outside it".to_string(),
                body: "nginx publishes runtime counters through a stub_status location that \
                       somebody has to add, and when it is absent there is no second source: \
                       the access log is the only other record of a request and on a packaged \
                       install it belongs to nginx's own user rather than to this service. So \
                       this page reads what an ordinary user can read - the running process, \
                       the config file it was started with, and two timings it takes itself. \
                       THE TWO TIMINGS ARE THE POINT. A static file and a PHP endpoint, over \
                       the same server: the first is nginx alone, the second is nginx plus the \
                       FastCGI round trip plus GLPI's bootstrap. Their difference is what PHP \
                       costs, measured from outside PHP, which is the one reading here that \
                       does not depend on GLPI reporting anything about itself. WHY THE STATIC \
                       FILE IS CHOSEN AND NOT FIXED: GLPI's vhost ends in try_files $uri \
                       /index.php, so a missing path is served BY PHP - timing a 404 would \
                       measure a GLPI bootstrap and label it nginx. Several known assets are \
                       tried and the first that answers 200 is the one timed; if none does, \
                       the probe says so rather than reporting the fastest failure."
                    .to_string(),
            }
        }

        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-sm text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Reading nginx\u{2026}"
                }
            },
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) => rsx! { NginxBody { report: r.clone() } },
        }

        div { class: "mt-4", NginxTuning {} }
    }
}

/// The readings, the settings, and what is missing.
#[component]
fn NginxBody(report: api::NginxReport) -> Element {
    // The pair the page exists for, when both were taken: nginx alone against
    // nginx plus everything behind it. By name, because a third probe landed
    // between them and an index would have quietly compared the wrong two.
    let overhead = match (
        report.probe("static file").and_then(|p| p.median_ms),
        report.probe("through php-fpm").and_then(|p| p.median_ms),
    ) {
        (Some(a), Some(b)) if a > 0.0 && b > a => Some((a, b, b - a)),
        _ => None,
    };
    // The third reading, which is a finding rather than a measurement of
    // nginx: what a path nginx cannot serve costs once try_files has handed
    // it to PHP.
    let missing = report
        .probe("a path that is not there")
        .and_then(|p| p.median_ms.map(|ms| (ms, p.bytes.unwrap_or(0))));

    rsx! {
        // Capacity first and across the width: it is the only figure here that
        // says what the server can take rather than what it is doing, and it
        // is the one a reader checks before a load test.
        if let Some(c) = report.capacity.clone() {
            div { class: "mb-4",
                Panel {
                    title: "What it can take".to_string(),
                    subtitle: match c.concurrent_requests {
                        Some(n) => format!("about {n} proxied requests in flight"),
                        None => "connection ceiling unknown".to_string(),
                    },
                    info: "The arithmetic, which is easy to get wrong in one direction: \
                           capacity is worker_processes times worker_connections, and a \
                           PROXIED request uses two of them - one to the client and one to \
                           php-fpm - so the number of GLPI requests in flight is half the \
                           total. Halved here rather than left to the reader, because the \
                           halving is the part everyone forgets. Where the config does not \
                           set a directive, nginx's own default is used and named below: an \
                           unset directive still has a value, and a blank reading would read \
                           as unlimited. somaxconn is the kernel's ceiling on a listen \
                           backlog - a backlog above it is silently clamped, so the two are \
                           worth seeing together."
                        .to_string(),

                    // Each figure carries its own modal rather than leaning
                    // on the panel's: the arithmetic above explains how they
                    // relate, and what a reader still has to guess at is
                    // what any one of them is on its own.
                    div { class: "flex flex-wrap items-baseline gap-x-8 gap-y-2",
                        Figure {
                            value: c.workers.to_string(),
                            label: if c.workers_auto {
                                "workers (auto)".to_string()
                            } else {
                                "worker process(es)".to_string()
                            },
                            body: "Processes nginx forked to serve connections, counted from \
                                   the running process table rather than read from the \
                                   config - so this is what is serving now, not what the \
                                   file asks for. Each one is single-threaded and \
                                   event-driven, which is why a single worker handles \
                                   thousands of connections at once."
                                .to_string(),
                        }
                        Figure {
                            value: match c.connections_per_worker {
                                Some(n) => n.to_string(),
                                None => "?".to_string(),
                            },
                            label: "connections per worker".to_string(),
                            body: "worker_connections: the ceiling on how many connections one \
                                   worker will hold at once, idle keepalive ones included. It \
                                   is per worker and not per server, so the total is this \
                                   times the figure beside it. Where the config does not set \
                                   it, nginx's own default is in force and is named under \
                                   these figures."
                                .to_string(),
                        }
                        Figure {
                            value: match c.concurrent_requests {
                                Some(n) => n.to_string(),
                                None => "?".to_string(),
                            },
                            label: "proxied requests, at most".to_string(),
                            body: "Workers times connections per worker, divided by two. A \
                                   static file needs one connection and would get the whole \
                                   figure; this is the GLPI number, which is the one worth \
                                   having. And it is a ceiling nginx enforces rather than a \
                                   throughput estimate: php-fpm's own worker count is usually \
                                   the smaller limit, and that is on the PHP page."
                                .to_string(),
                        }
                        Figure {
                            value: match c.somaxconn {
                                Some(n) => n.to_string(),
                                None => "?".to_string(),
                            },
                            label: "somaxconn".to_string(),
                            body: "The kernel's ceiling on how many connections may queue on a \
                                   listening socket before nginx accepts them. nginx's own \
                                   backlog is clamped to it silently, so a config asking for \
                                   more is neither an error nor in effect. It decides what \
                                   survives a burst, not what throughput is."
                                .to_string(),
                        }
                    }

                    if !c.defaults_used.is_empty() {
                        {
                            let defaults = c.defaults_used.join(", ");
                            rsx! {
                                p { class: "mt-3 max-w-prose text-xs text-base-content/60",
                                    "From nginx's defaults rather than from the config: "
                                    span { class: "font-mono", "{defaults}" }
                                    ". Worth knowing before changing either - a value nobody \
                                     chose is a different thing from one somebody did."
                                }
                            }
                        }
                    }
                }
            }
        }

        // The counters, when they exist at all. Full width above the rest for
        // the same reason: accepts against handled is the one reading here
        // that nothing else in this app can substitute for.
        if let Some(st) = report.status.clone() {
            div { class: "mb-4", NginxCounters { status: st } }
        }

        // Stacked, each across the full width. The timings are three rows and
        // the settings are sixteen, so side by side meant one short panel
        // beside a tall one and a column of white space under the short one.
        // Full width also lets each panel lay its own content out
        // horizontally, which is where the room actually goes.
        div { class: "space-y-4",
            Panel {
                title: "What a request costs before PHP".to_string(),
                subtitle: "Taken now, five samples each".to_string(),
                info: "Both timed including the body, with connection reuse off, because a \
                       browser arriving at GLPI does not have a pooled connection either. \
                       The static reading is the whole of what nginx costs: accept, resolve a \
                       path, read a file, write it out."
                    .to_string(),
                // Three cards across rather than three table rows: the rows
                // were a narrow column of text with the whole right-hand side
                // of a full-width panel empty beside them, and the three
                // readings are meant to be compared with each other - side by
                // side is how a comparison reads.
                div { class: "grid gap-3 sm:grid-cols-2 lg:grid-cols-3",
                    for p in report.probes.iter() {
                        div {
                            key: "{p.url}",
                            class: "flex flex-col rounded-box border border-base-300 p-3",
                            div { class: "flex flex-wrap items-baseline justify-between gap-x-2",
                                // The name and its modal together, so the
                                // button belongs to this reading rather than
                                // to the row of three.
                                div { class: "flex items-center gap-1.5",
                                    span { class: "font-medium", "{p.what}" }
                                    if let Some(b) = lookup(&NGINX_PROBES, &p.what) {
                                        InfoButton {
                                            title: p.what.clone(),
                                            body: b.to_string(),
                                        }
                                    }
                                }
                                if let Some(code) = p.status {
                                    span {
                                        class: if code == 200 {
                                            "font-mono text-xs text-base-content/50"
                                        } else {
                                            // Not an error - the 404 is
                                            // deliberate - but it is the
                                            // reason that row is slow, so it
                                            // should not read as an aside.
                                            "font-mono text-xs text-warning"
                                        },
                                        "HTTP {code}"
                                    }
                                }
                            }

                            if let Some(e) = p.error.clone() {
                                div { class: "mt-2 text-xs text-error", "{e}" }
                            } else {
                                div { class: "mt-1 font-mono text-2xl",
                                    if let Some(ms) = p.median_ms {
                                        "{ms:.2} ms"
                                    } else {
                                        "no reading"
                                    }
                                }
                                div { class: "text-xs text-base-content/60",
                                    if let Some(ms) = p.best_ms {
                                        "best {ms:.2}"
                                    }
                                    if let Some(n) = p.bytes {
                                        " \u{b7} {n} B"
                                    }
                                    // The type is what decides whether
                                    // gzip_types matched the response, so it
                                    // belongs beside the encoding rather than
                                    // in a column of its own.
                                    match p.encoding.clone() {
                                        Some(enc) => rsx! { " \u{b7} {enc}" },
                                        None => rsx! {
                                            span { class: "text-warning", " \u{b7} not compressed" }
                                        },
                                    }
                                }
                                if let Some(ct) = p.content_type.clone() {
                                    div { class: "font-mono text-xs text-base-content/40", "{ct}" }
                                }
                            }

                            // Pushed to the bottom so the figures line up
                            // across the three cards whatever the detail
                            // wraps to.
                            p { class: "mt-2 grow text-xs text-base-content/60", "{p.detail}" }
                            div { class: "mt-1 truncate font-mono text-xs text-base-content/40",
                                "{p.url}"
                            }
                        }
                    }
                }

                // The two readings-about-the-readings, side by side: both are
                // short prose, and stacked they left the same right-hand gap
                // the table did.
                div { class: "mt-4 grid gap-x-6 gap-y-3 border-t border-base-300 pt-3 lg:grid-cols-2",
                    if let Some((static_ms, php_ms, diff)) = overhead {
                        div {
                            p { class: "max-w-prose text-sm",
                            "nginx answers a file in "
                            span { class: "font-semibold", "{static_ms:.2} ms" }
                            " and a PHP page in "
                            span { class: "font-semibold", "{php_ms:.1} ms" }
                            ", so about "
                            span { class: "font-semibold", "{diff:.1} ms" }
                            " of that request is PHP and GLPI rather than the web server. \
                             The web server is not what to tune while that holds - and it \
                             is the same shape of comparison the DB page makes underneath."
                        }
                    }
                }

                if let Some((ms, bytes)) = missing {
                    div {
                        p { class: "max-w-prose text-sm",
                            span { class: "font-medium", "A path that is not there costs " }
                            span { class: "font-semibold", "{ms:.1} ms" }
                            " and "
                            span { class: "font-semibold", "{bytes} bytes" }
                            ". That is not nginx refusing it: GLPI's vhost ends in \
                             try_files $uri /index.php, so anything that is not a file on \
                             disk is routed to PHP, which renders a full error page. One \
                             mistyped asset reference in a theme or plugin therefore costs \
                             a GLPI bootstrap on every page view that includes it - and it \
                             shows up in nginx's access log as a 404, not as a slow page."
                        }
                    }
                }
                }
            }

            Panel {
                title: "How it is set up".to_string(),
                subtitle: match report.version.clone() {
                    Some(v) => format!("nginx {v}, {} worker(s)", report.workers),
                    None => format!("{} worker(s)", report.workers),
                },
                info: "Ten directives, read as the first word on a line in the config the \
                       master process was started with - not a parse of the whole file. \
                       nginx's grammar has includes, maps and nesting, and a half-parser \
                       would be wrong in ways nobody reading this page could see. A blank \
                       value means the directive does not appear, so nginx uses its own \
                       default."
                    .to_string(),

                if !report.running {
                    div { class: "alert alert-warning text-sm",
                        "No nginx master process is running. Either something else is \
                         serving GLPI, or it is down - and if it were down, nothing on this \
                         page would have answered."
                    }
                } else if !report.config_readable {
                    div { class: "alert alert-info text-sm",
                        "nginx is running but its config is not readable by this service, \
                         so there is nothing to show below. "
                        if let Some(path) = report.config_path.clone() {
                            "It was started with "
                            span { class: "font-mono text-xs", "{path}" }
                            "."
                        } else {
                            "Its command line does not say which file it used."
                        }
                    }
                } else {
                    // Across the width rather than down it: sixteen directives
                    // as table rows is a column of mostly empty space on any
                    // screen wide enough to read this page on, and the
                    // directives are independent of each other - nothing is
                    // being compared down the column, so nothing is lost by
                    // wrapping them.
                    div { class: "grid gap-3 sm:grid-cols-2 xl:grid-cols-3",
                        for st in report.settings.iter() {
                            div {
                                key: "{st.name}",
                                class: "rounded-box border border-base-300 p-3",
                                div { class: "flex flex-wrap items-baseline justify-between gap-x-3",
                                    // The directive's own modal, beside its
                                    // name: the line under the card says
                                    // what it is for in one sentence, and
                                    // what it leaves out is what the value
                                    // trades against.
                                    div { class: "flex items-center gap-1.5",
                                        span { class: "font-mono text-xs", "{st.name}" }
                                        if let Some(b) = lookup(&NGINX_DIRECTIVES, &st.name) {
                                            InfoButton {
                                                title: st.name.clone(),
                                                body: b.to_string(),
                                            }
                                        }
                                    }
                                    span { class: "font-mono text-sm",
                                        if let Some(v) = st.value.clone() {
                                            "{v}"
                                        } else {
                                            span { class: "text-base-content/40", "not set" }
                                        }
                                    }
                                }
                                p { class: "mt-1 text-xs text-base-content/60", "{st.note}" }
                            }
                        }
                    }

                    if let Some(path) = report.config_path.clone() {
                        p { class: "mt-3 font-mono text-xs text-base-content/50", "{path}" }
                    }
                }

                if !report.modules.is_empty() {
                    div { class: "mt-4 border-t border-base-300 pt-3",
                        div { class: "mb-2 text-xs font-medium text-base-content/70",
                            "What this build can do"
                        }
                        ul { class: "grid gap-1 text-xs sm:grid-cols-2",
                            for m in report.modules.iter() {
                                li { key: "{m.flag}", class: "flex gap-2",
                                    span {
                                        class: if m.present {
                                            "font-mono text-success"
                                        } else {
                                            "font-mono text-base-content/40"
                                        },
                                        if m.present { "yes" } else { "no" }
                                    }
                                    span {
                                        span { class: "font-mono", "{m.flag}" }
                                        span { class: "text-base-content/60", " - {m.note}" }
                                    }
                                    if let Some(b) = lookup(&NGINX_MODULES, &m.flag) {
                                        InfoButton { title: m.flag.clone(), body: b.to_string() }
                                    }
                                }
                            }
                        }
                        if let Some(b) = report.binary.clone() {
                            p { class: "mt-2 font-mono text-xs text-base-content/40", "{b}" }
                        }
                    }
                }

                // Last, because it is the thing to DO rather than a reading.
                if report.running && !report.stub_status {
                    div { class: "mt-3 text-xs text-base-content/60",
                        "No stub_status location, so nginx keeps no counters this page can \
                         read - no request rate, no active connections, no accept backlog. \
                         Adding one to the vhost, bound to localhost, is what would turn \
                         this page from two timings into a history like the other two."
                    }
                }
            }
        }
    }
}
/// nginx's own counters, and the two comparisons that matter in them.
///
/// Its own component because it renders only when a stub_status location
/// exists - the minority case, and the page says how to create one when it
/// does not.
#[component]
fn NginxCounters(status: api::NginxStatus) -> Element {
    let dropped = status.dropped.unwrap_or(0);
    let thin_keepalive = status.per_connection.is_some_and(|r| r < 1.5);

    rsx! {
        Panel {
            title: "What it has been doing".to_string(),
            subtitle: "Counters since the master started, not a rate now".to_string(),
            info: "stub_status is the only thing nginx counts, and it counts since the \
                   master process started - so a rate taken from it is an average over that \
                   whole period rather than what is happening at this moment. Two \
                   comparisons carry almost all of the value. ACCEPTS AGAINST HANDLED: if \
                   they differ, nginx DROPPED connections, which happens when it runs out \
                   of worker connections or file descriptors, and nothing else on this \
                   install records that it happened. REQUESTS PER CONNECTION: at one, every \
                   request paid its own connection setup and keepalive is buying nothing - \
                   over TLS that is a handshake each time. Reading and Writing are \
                   instantaneous rather than cumulative, and Writing includes time spent \
                   waiting on php-fpm, which is why it rises when PHP is the bottleneck \
                   rather than nginx."
                .to_string(),

            if let Some(e) = status.error.clone() {
                div { class: "alert alert-warning text-sm",
                    "The config has a stub_status location but it could not be read: {e}"
                }
            } else {
                div { class: "flex flex-wrap items-baseline gap-x-8 gap-y-2",
                    Figure {
                        value: match status.active {
                            Some(n) => n.to_string(),
                            None => "?".to_string(),
                        },
                        label: "active connections".to_string(),
                        body: "Connections open at the instant this page read the counter, \
                               idle keepalive ones included. The only figure in this panel \
                               that is a reading of now rather than a total since the master \
                               started - and it is one reading, so a value close to the \
                               ceiling above is worth taking again before acting on it."
                            .to_string(),
                    }
                    Figure {
                        value: dropped.to_string(),
                        alarm: dropped > 0,
                        label: "dropped (accepts \u{2212} handled)".to_string(),
                        body: "Subtracted here, because nginx publishes the two totals and \
                               never the difference. It counts since the master process \
                               started, so a figure above zero may be from a load test last \
                               month rather than from this morning: the uptime is what dates \
                               it, and a restart is what clears it."
                            .to_string(),
                    }
                    Figure {
                        value: match status.per_connection {
                            Some(n) => format!("{n:.1}"),
                            None => "?".to_string(),
                        },
                        label: "requests per connection".to_string(),
                        body: "Requests divided by handled connections. Above one, setup is \
                               being amortised across several requests; at one, each request \
                               opened and closed its own. HTTP/2 raises it on its own, \
                               because a browser multiplexes a whole page over one \
                               connection - so a low figure with http2 on points at the \
                               clients rather than at the setting."
                            .to_string(),
                    }
                    Figure {
                        value: match status.requests {
                            Some(n) => n.to_string(),
                            None => "?".to_string(),
                        },
                        label: "requests served".to_string(),
                        body: "Every request since the master process started, static and \
                               proxied counted together, with no split by vhost, path or \
                               status code - stub_status keeps one total and nothing else. \
                               Over the uptime it gives an average rate, which is not the \
                               rate now."
                            .to_string(),
                    }
                }

                // items-center here, which the plain row did not need: these
                // three now carry buttons taller than their own text.
                div { class: "mt-3 flex flex-wrap items-center gap-x-6 gap-y-1 text-xs text-base-content/60",
                    if let Some(n) = status.reading {
                        div { class: "flex items-center gap-1.5",
                            span { "reading {n}" }
                            InfoButton {
                                title: "reading".to_string(),
                                body: "Connections nginx is reading a request from right now - \
                                       the header, or the body of an upload. A figure that \
                                       stays high points at slow clients or large submissions \
                                       rather than at anything nginx is doing."
                                    .to_string(),
                            }
                        }
                    }
                    if let Some(n) = status.writing {
                        div { class: "flex items-center gap-1.5",
                            span { "writing {n}" }
                            InfoButton {
                                title: "writing".to_string(),
                                // "in flight", not the obvious word for it:
                                // output.css is generated by scanning these
                                // files for anything class-shaped, and that
                                // word pulled daisyUI's whole component of
                                // the same name into the stylesheet - 1.7 KB
                                // of rules this app never uses.
                                body: "Connections nginx is writing a response to. A proxied \
                                       request counts here for the whole time php-fpm is \
                                       thinking and not only while bytes move, so this is \
                                       nearer to a count of requests in flight than to \
                                       anything about the network."
                                    .to_string(),
                            }
                        }
                    }
                    if let Some(n) = status.waiting {
                        div { class: "flex items-center gap-1.5",
                            span { "waiting {n}" }
                            InfoButton {
                                title: "waiting".to_string(),
                                body: "Idle keepalive connections, held open for a request that \
                                       may never arrive. Each costs one worker connection and \
                                       nothing else, so a large number beside a healthy \
                                       ceiling above is keepalive working rather than a \
                                       problem."
                                    .to_string(),
                            }
                        }
                    }
                    span { class: "font-mono text-base-content/40", "{status.url}" }
                }

                if dropped > 0 {
                    p { class: "mt-3 max-w-prose text-sm text-error",
                        "nginx dropped {dropped} connection(s). That is a connection or \
                         file-descriptor limit rather than a slow page, and the capacity \
                         figures above are where to look first."
                    }
                } else if thin_keepalive {
                    p { class: "mt-3 max-w-prose text-sm",
                        "About one request per connection, so keepalive is buying nothing: \
                         every request is paying its own setup, and over TLS that is a \
                         handshake each time."
                    }
                }
            }
        }
    }
}
