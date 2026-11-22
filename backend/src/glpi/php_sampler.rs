//! Times GLPI's own endpoints on a schedule, so the perf page can say what
//! normal has been rather than only what it is.
//!
//! WHY THIS ONE IS A TASK AND NOT AN EVENT. The status counters are sampled
//! by a MariaDB EVENT - the database reads itself, and this process could be
//! stopped for a week without losing a point. Nothing equivalent is possible
//! here: MariaDB cannot make an HTTP request, so the only thing that can time
//! GLPI is something that can call it. The cost of that is real and worth
//! stating plainly - when this process is down, this history has a hole, and
//! the MariaDB history does not.
//!
//! IT ADDS LOAD TO WHAT IT MEASURES, which is the thing the perf page
//! otherwise refuses to do. Two unauthenticated GETs per interval, both of
//! them the cheapest pages GLPI serves, 2880 a day at the default minute.
//! That is a deliberate trade for having an overnight record at all, and it
//! is the only sampler in this app with a dial: GLPI_INV_PHP_SAMPLE_SECONDS,
//! 0 to stop.
//!
//! ONE SAMPLE PER ENDPOINT PER TICK, not the several the /perf page takes.
//! That page reports a best and a median because it is answering "how fast
//! is this right now" and a lone sample there would report a cold start as
//! though it were typical. Here the averaging happens across time instead -
//! a bucket on the chart is already the mean of many minutes - so taking
//! three samples a minute would triple the load for a smoothing the chart
//! does anyway.
use std::time::{Duration, Instant};

use super::refused::Db;
use super::sampler::PHP_ENDPOINTS;

/// Per-request timeout.
///
/// Generous, because a slow answer is the reading - cutting it off at two
/// seconds would record the timeout rather than the latency, and a GLPI
/// taking eight seconds is exactly what someone would come to this page to
/// find. Short enough that a hung endpoint cannot stall the interval.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Starts the sampler, if one is wanted.
///
/// Spawned and forgotten: it owns no state this process reads back, and
/// everything it produces is in the database. A failure to record is logged
/// and the loop continues - the next minute is a fresh attempt, and a
/// sampler that exits on the first error would leave a hole nobody is told
/// about.
pub fn spawn(db: Db, glpi_url: String, every: Duration) {
    tokio::spawn(async move {
        let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("PHP sampler not started, no HTTP client: {e:#}");
                return;
            }
        };

        tracing::info!(
            "PHP sampler every {}s against {} endpoints",
            every.as_secs(),
            PHP_ENDPOINTS.len()
        );

        let mut ticker = tokio::time::interval(every);
        // Skip missed ticks rather than firing them back to back. The
        // default behaviour would burst after any pause - a suspended
        // laptop, a slow GLPI - which is the worst moment to add requests.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            for (path, _) in PHP_ENDPOINTS {
                let (ms, ok) = time_once(&client, &glpi_url, path).await;
                if let Err(e) = db.record_php_sample(path, ms, ok).await {
                    tracing::warn!("could not record the {path} timing: {e:#}");
                }
            }
        }
    });
}

/// One request, timed, with whether it actually answered.
///
/// The body is drained before the clock stops. Stopping at the headers times
/// the first byte and calls it the request, which for a rendered PHP page is
/// most of the work missed - the same reason glpi::perf drains it.
async fn time_once(client: &reqwest::Client, base: &str, path: &str) -> (f64, bool) {
    let url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let started = Instant::now();
    let ok = match client.get(&url).send().await {
        Ok(r) => {
            let status_ok = r.status().is_success();
            let drained = r.bytes().await.is_ok();
            status_ok && drained
        }
        Err(_) => false,
    };
    (started.elapsed().as_secs_f64() * 1000.0, ok)
}
