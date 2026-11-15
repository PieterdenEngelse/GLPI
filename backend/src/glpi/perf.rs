//! How long GLPI takes to answer, and how much of that is the database.
//!
//! ABOUT GLPI, NOT ABOUT THIS APP. Everything here is a measurement of the
//! install this dashboard watches: PHP answering its own endpoints, and
//! MariaDB answering a query of the shape GLPI's pages run. This app's own
//! latency is not on it - it is the instrument, and an instrument that
//! reports itself invites the reader to tune the wrong thing.
//!
//! WHY MEASURE AT ALL RATHER THAN READ A CONFIGURATION. The question "is
//! GLPI slow, and if so where" is not answered by any file: the same
//! php.ini is fast on an empty install and slow on a large one, and the
//! database that takes 0.3 ms today takes longer when it stops fitting in
//! memory. A reading taken now is the only honest answer, and taking it
//! again later is how the change is seen.
//!
//! WHAT THE NUMBERS MEAN, and what they do not. Both endpoints are
//! unauthenticated and small: status.php runs GLPI's own health checks, and
//! index.php renders the login page. Neither is a heavy page, so these are a
//! FLOOR on what a request costs - the per-request overhead every page pays
//! before doing anything particular. A list view with a filter costs that
//! plus its own work. The floor is the useful number anyway, because it is
//! the part no query optimisation can remove.
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

/// How many times each endpoint is hit.
///
/// Five, and the median is reported beside the fastest: the first request
/// after an idle period pays for connection setup and whatever PHP has let
/// fall out of opcache, so a single sample reports a cold start as though it
/// were the normal case.
const SAMPLES: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    /// What was asked, as a path - the host is reported once, by the page.
    pub what: String,
    /// What it is, in a few words, because `/status.php` does not say.
    pub detail: String,
    pub best_ms: f64,
    pub median_ms: f64,
    pub worst_ms: f64,
    /// Present when the request failed, in which case the times are of the
    /// failures and worth nothing - said rather than silently averaged in.
    pub error: Option<String>,
}

/// One derived reading about the running server.
///
/// `value` is formatted here rather than in the page, because the formatting
/// is part of the reading - a hit rate is a percentage, a rate is per
/// second, and a count is a count. `detail` carries the raw figures it came
/// from, so nothing on the page is a number the reader has to take on faith.
///
/// NO VERDICTS. None of these says "good" or "bad": a buffer pool hit rate
/// of 99% is excellent on one workload and a symptom on another, and a
/// threshold written here would be this app's opinion wearing the server's
/// clothes. The page states what was measured and what it is made of.
#[derive(Debug, Clone, Serialize)]
pub struct Gauge {
    pub label: String,
    pub value: String,
    pub detail: String,
}

/// One of GLPI's own scheduled tasks, with what it has actually cost.
///
/// GLPI measures this itself - `glpi_crontasklogs.elapsed` is seconds, written
/// by the task when it finishes - so this is the application's own account of
/// its background work rather than anything inferred from outside.
#[derive(Debug, Clone, Serialize)]
pub struct CronTask {
    pub name: String,
    /// Seconds between runs, as GLPI schedules it.
    pub frequency: i64,
    /// GLPI's own state column: 0 disabled, 1 waiting, 2 running.
    pub state: i64,
    pub last_run: Option<String>,
    pub avg_seconds: f64,
    pub max_seconds: f64,
    pub runs: i64,
}

/// One table, by what it costs on disk.
#[derive(Debug, Clone, Serialize)]
pub struct TableSize {
    pub name: String,
    /// InnoDB's estimate, not a COUNT - exact counts on every table would be
    /// a table scan each, and the question here is scale rather than a
    /// figure anyone will quote.
    pub rows: i64,
    pub kib: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfReport {
    pub glpi_url: String,
    /// PHP's version, as the server volunteers it in X-Powered-By. Absent
    /// when the header is suppressed, which is a hardening choice and not a
    /// fault - the page says so rather than showing a blank.
    pub php: Option<String>,
    pub http: Vec<Timing>,
    /// A query of the shape GLPI's own pages run.
    pub query: Option<Timing>,
    pub samples: usize,
    /// How the engine is behaving, as opposed to how it is configured - the
    /// settings are on the DB page, and these are what they produced.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub engine: Vec<Gauge>,
    /// GLPI's background work, by what it costs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cron: Vec<CronTask>,
    /// The largest tables, and what the whole database weighs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<TableSize>,
    pub total_kib: i64,
    pub total_tables: i64,
}

fn stats(mut times: Vec<Duration>) -> (f64, f64, f64) {
    times.sort();
    let ms = |d: &Duration| d.as_secs_f64() * 1000.0;
    (
        ms(times.first().unwrap_or(&Duration::ZERO)),
        ms(&times[times.len() / 2]),
        ms(times.last().unwrap_or(&Duration::ZERO)),
    )
}

/// Time one GLPI endpoint, and report PHP's version if it is volunteered.
async fn time_http(
    client: &reqwest::Client,
    base: &str,
    path: &str,
    detail: &str,
) -> (Timing, Option<String>) {
    let url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let mut times = Vec::with_capacity(SAMPLES);
    let mut error = None;
    let mut php = None;

    for _ in 0..SAMPLES {
        let started = Instant::now();
        match client.get(&url).send().await {
            Ok(r) => {
                if php.is_none() {
                    php = r
                        .headers()
                        .get("x-powered-by")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                }
                // The body has to be drained for the timing to include it;
                // stopping at the headers would time the first byte and call
                // it the request.
                let _ = r.bytes().await;
            }
            Err(e) => error = Some(e.to_string()),
        }
        times.push(started.elapsed());
    }

    let (best_ms, median_ms, worst_ms) = stats(times);
    (
        Timing {
            what: path.to_string(),
            detail: detail.to_string(),
            best_ms,
            median_ms,
            worst_ms,
            error,
        },
        php,
    )
}

/// Time GLPI's own endpoints, and a query of the kind its pages run.
pub async fn measure(glpi_url: &str, db: Option<&super::refused::Db>) -> Result<PerfReport> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        // Fresh connections, because that is what a browser arriving at GLPI
        // does; a pooled one would report a number no page ever sees.
        .pool_max_idle_per_host(0)
        .build()
        .context("building the HTTP client")?;

    let mut http = Vec::new();
    let mut php = None;

    for (path, detail) in [
        (
            "/status.php",
            "GLPI's own health check - the cheapest thing it serves",
        ),
        (
            "/index.php",
            "The login page: a real render, with GLPI's bootstrap behind it",
        ),
    ] {
        let (timing, found) = time_http(&client, glpi_url, path, detail).await;
        php = php.or(found);
        http.push(timing);
    }

    let (query, engine, cron, tables, total_kib, total_tables) = match db {
        Some(db) => {
            let (engine, cron, tables, total_kib, total_tables) = db.perf_readings().await?;
            (
                Some(db.time_query(SAMPLES).await?),
                engine,
                cron,
                tables,
                total_kib,
                total_tables,
            )
        }
        None => (None, Vec::new(), Vec::new(), Vec::new(), 0, 0),
    };

    Ok(PerfReport {
        glpi_url: glpi_url.to_string(),
        php,
        http,
        query,
        samples: SAMPLES,
        engine,
        cron,
        tables,
        total_kib,
        total_tables,
    })
}
