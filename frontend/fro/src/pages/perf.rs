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
use crate::components::{FetchError, InfoButton, Panel};
use dioxus::prelude::*;

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
                    div { class: "grid gap-3 sm:grid-cols-2 lg:grid-cols-3",
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
