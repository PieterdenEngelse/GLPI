//! How long this backend takes to answer its own `/api/*` requests.
//!
//! WHAT THIS IS NOT. The perf page measures GLPI; this measures the
//! instrument. That page's own doc warns that an instrument reporting on
//! itself invites tuning the wrong thing, and the warning stands - nothing
//! here is a reason to tune this process. Its four tokio workers had
//! consumed 0.01 CPU seconds between them after nearly nine hours, and the
//! runtime is not what any of these numbers are about.
//!
//! WHAT IT IS, and why it turned out worth having. A route's end-to-end time
//! INCLUDES whatever it waited on. `/api/agents` calls GLPI, so its figure is
//! mostly GLPI's PHP; `/api/mariadb/plugins` only queries the local socket,
//! so its figure is mostly the query. Side by side they decompose a page
//! load into "the part upstream owns" and "the part this box owns", which is
//! the same comparison the rest of the perf page makes and could not make
//! per-route.
//!
//! KEYED BY THE MATCHED ROUTE, never the request path. `/api/logs/{file}`
//! must be one row however many files are tailed - keying by URI turns a
//! parameterised route into unbounded keys, which is a memory leak driven by
//! whatever a client happens to request.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

/// Samples kept per route.
///
/// Bounded because this is a process that runs for weeks: an unbounded
/// vector per route is a slow leak, and a percentile over every request
/// since boot describes a server that no longer exists. 200 is a window
/// wide enough for a p95 to mean something and small enough that the whole
/// store stays a few hundred kilobytes with every route populated.
const KEEP: usize = 200;

/// Durations for one route, newest last.
#[derive(Default)]
struct Samples {
    /// Every request since start, including the ones dropped from `recent`.
    /// A count is cheap and is the one figure a window cannot give.
    count: u64,
    recent: Vec<f64>,
}

/// Per-route timings for this process.
#[derive(Default)]
pub struct Timings {
    routes: Mutex<HashMap<String, Samples>>,
}

impl Timings {
    /// Records one answered request.
    ///
    /// A poisoned lock is ignored rather than propagated: this is
    /// instrumentation, and a panic here would take down request handling to
    /// protect a statistic.
    pub fn record(&self, route: &str, took: Duration) {
        let Ok(mut routes) = self.routes.lock() else {
            return;
        };
        let entry = routes.entry(route.to_string()).or_default();
        entry.count += 1;
        if entry.recent.len() == KEEP {
            entry.recent.remove(0);
        }
        entry.recent.push(took.as_secs_f64() * 1000.0);
    }

    /// A reading of every route seen so far, slowest median first.
    pub fn report(&self) -> Vec<RouteTiming> {
        let Ok(routes) = self.routes.lock() else {
            return Vec::new();
        };

        let mut out: Vec<RouteTiming> = routes
            .iter()
            .filter_map(|(path, s)| {
                if s.recent.is_empty() {
                    return None;
                }
                let mut sorted = s.recent.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                Some(RouteTiming {
                    path: path.clone(),
                    count: s.count,
                    sampled: sorted.len(),
                    median_ms: percentile(&sorted, 0.50),
                    p95_ms: percentile(&sorted, 0.95),
                    max_ms: *sorted.last().unwrap_or(&0.0),
                    last_ms: *s.recent.last().unwrap_or(&0.0),
                })
            })
            .collect();

        // Slowest first, because the question is which route costs the most.
        // Path is the tiebreak so the order cannot depend on HashMap
        // iteration, which is randomised per process.
        out.sort_by(|a, b| {
            b.median_ms
                .partial_cmp(&a.median_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.path.cmp(&b.path))
        });
        out
    }
}

/// Nearest-rank percentile over an ascending slice.
///
/// Nearest-rank rather than interpolated: every value returned is a request
/// that actually happened, which is the right property for a latency figure
/// someone will go looking for in a log.
fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}

/// One route's timings.
#[derive(Serialize)]
pub struct RouteTiming {
    /// The matched route, so `/api/logs/{file}` stays one row.
    pub path: String,
    /// Requests since this process started.
    pub count: u64,
    /// How many of those the figures below are over - at most KEEP.
    pub sampled: usize,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub last_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_route_reports_its_median_and_extremes() {
        let t = Timings::default();
        for v in [10, 20, 30, 40, 50] {
            t.record("/api/thing", ms(v));
        }
        let r = t.report();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].count, 5);
        assert!((r[0].median_ms - 30.0).abs() < 1.0, "{}", r[0].median_ms);
        assert!((r[0].max_ms - 50.0).abs() < 1.0);
        assert!((r[0].last_ms - 50.0).abs() < 1.0);
    }

    /// The window is what bounds memory on a process that runs for weeks,
    /// and the lifetime count has to survive it.
    #[test]
    fn samples_are_bounded_but_the_count_is_not() {
        let t = Timings::default();
        for _ in 0..(KEEP + 50) {
            t.record("/api/thing", ms(1));
        }
        let r = t.report();
        assert_eq!(r[0].count as usize, KEEP + 50);
        assert_eq!(r[0].sampled, KEEP);
    }

    /// Old samples leave, so a route that was slow and got fixed stops
    /// reporting the old figure.
    #[test]
    fn the_window_forgets() {
        let t = Timings::default();
        t.record("/api/thing", ms(9999));
        for _ in 0..KEEP {
            t.record("/api/thing", ms(1));
        }
        let r = t.report();
        assert!(
            r[0].max_ms < 100.0,
            "the 9999ms sample survived: {}",
            r[0].max_ms
        );
    }

    #[test]
    fn routes_come_back_slowest_first() {
        let t = Timings::default();
        t.record("/api/fast", ms(1));
        t.record("/api/slow", ms(500));
        t.record("/api/middling", ms(50));
        let r = t.report();
        let order: Vec<&str> = r.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(order, vec!["/api/slow", "/api/middling", "/api/fast"]);
    }

    /// A percentile has to be a value that actually occurred.
    #[test]
    fn the_p95_is_a_real_sample() {
        let sorted: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        assert_eq!(percentile(&sorted, 0.95), 95.0);
        assert_eq!(percentile(&sorted, 0.50), 50.0);
        assert_eq!(percentile(&[], 0.95), 0.0);
    }
}
