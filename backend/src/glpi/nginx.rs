//! The web server in front of GLPI, as far as it can be read without root.
//!
//! WHY THIS IS CONFIGURATION AND NOT COUNTERS. nginx publishes runtime
//! metrics through `stub_status`, which is a `location` somebody has to add -
//! and when it is absent there is no second source: the access log is the
//! only other record of a request, and on a packaged install it belongs to
//! nginx's own user, not to this service. So this module reads what it can
//! read as an ordinary user - the running process, the config file it was
//! started with, and two timings it takes itself - and says plainly which
//! half is missing rather than drawing an empty chart.
//!
//! THE TWO TIMINGS ARE THE POINT. A static file and a PHP endpoint, both
//! through nginx: the first is nginx alone, the second is nginx plus
//! php-fpm plus GLPI's bootstrap. The difference is what PHP costs, measured
//! from outside PHP, which is the one reading on the perf page that does not
//! depend on GLPI being willing to report anything.
use std::time::{Duration, Instant};

use serde::Serialize;

/// Directives worth reading off a GLPI vhost, with why each one is here.
///
/// A fixed list rather than a parse of the whole file: nginx's grammar has
/// includes, maps and nesting, and a half-parser would be wrong in ways a
/// reader could not see. Each entry is matched as the first word on a line,
/// so `gzip on;` matches `gzip` and `gzip_types ...` does not.
const DIRECTIVES: [(&str, &str); 10] = [
    (
        "gzip",
        "Compression. Off means every HTML and JSON response travels whole, which is \
         the cheapest large win available on a slow link.",
    ),
    (
        "gzip_comp_level",
        "How hard it compresses. Above 6 costs CPU for very little size on text.",
    ),
    (
        "keepalive_timeout",
        "How long a connection is held open for reuse. Zero means a TCP and TLS \
         handshake per request, which dominates everything else on a high-latency link.",
    ),
    (
        "sendfile",
        "Static files copied kernel-to-socket rather than through nginx's buffers.",
    ),
    (
        "tcp_nopush",
        "Full packets rather than partial ones. Pairs with sendfile.",
    ),
    (
        "client_max_body_size",
        "The ceiling on an upload, and on an inventory submission arriving over HTTP: \
         an agent sending more than this gets a 413 and the report never lands.",
    ),
    (
        "fastcgi_read_timeout",
        "How long nginx waits for php-fpm. Shorter than PHP's own limit turns a slow \
         page into a 504 while the worker is still running.",
    ),
    (
        "ssl_protocols",
        "Which TLS versions are offered. 1.3 saves a round trip on every new \
         connection, so it is a latency setting as much as a security one.",
    ),
    (
        "ssl_session_cache",
        "Resumed TLS sessions skip the handshake. Absent means every visit pays it.",
    ),
    (
        "server_tokens",
        "Whether the version is advertised. Not performance - read here because this \
         is the file it lives in.",
    ),
];

/// One directive as the running config has it.
#[derive(Debug, Clone, Serialize)]
pub struct NginxSetting {
    pub name: String,
    /// None when the directive does not appear, which is a reading: nginx then
    /// uses its own default, and the note says what that costs.
    pub value: Option<String>,
    pub note: String,
}

/// One timing taken through nginx.
#[derive(Debug, Clone, Serialize)]
pub struct NginxProbe {
    pub what: String,
    pub detail: String,
    pub url: String,
    /// None when the request failed; `error` then says why. A failure is not a
    /// slow reading and must not be averaged with one.
    pub median_ms: Option<f64>,
    pub best_ms: Option<f64>,
    pub status: Option<u16>,
    pub error: Option<String>,
}

/// What can be established about the web server from outside it.
#[derive(Debug, Clone, Serialize)]
pub struct NginxReport {
    /// False when no nginx master process is running under any user.
    pub running: bool,
    /// From the binary path on the master's command line, which on a
    /// store-path install carries the version. None elsewhere - `nginx -v`
    /// would need the binary to be on this service's PATH.
    pub version: Option<String>,
    /// The config file the master was started with, from its own argv rather
    /// than from a guess at the conventional location.
    pub config_path: Option<String>,
    /// Whether that file is readable by this service. False is common and not
    /// a fault: the settings below are then empty and the page says why.
    pub config_readable: bool,
    pub workers: usize,
    /// Whether a `stub_status` location exists anywhere in the config. The
    /// difference between this page having live counters and not.
    pub stub_status: bool,
    pub settings: Vec<NginxSetting>,
    pub probes: Vec<NginxProbe>,
}

/// Static files to try, in order, for the nginx-alone timing.
///
/// Several because the one that exists is a GLPI version detail, and a
/// hardcoded single path would quietly turn this reading into a PHP one the
/// first time GLPI renamed an asset. All three are served from GLPI's public
/// root by the `location /` block, so a 200 from any of them is nginx reading
/// a file and nothing else.
const STATIC_CANDIDATES: [&str; 3] = ["/pics/favicon.ico", "/js/common.js", "/lib/base.js"];

/// The config path out of a rewritten process title.
fn config_from_title(title: &str) -> Option<String> {
    let rest = title.split(" -c ").nth(1)?;
    let path = rest.split_whitespace().next()?;
    (!path.is_empty()).then(|| path.to_string())
}

/// How many times each probe is taken. Five, and the median reported beside
/// the best, for the same reason the GLPI timings use five: the first request
/// pays a cold connection and is not the normal case.
const SAMPLES: usize = 5;

/// Everything readable, plus the two timings.
pub async fn report(glpi_url: &str) -> NginxReport {
    // Its own client, built like the perf module's and for the same reason:
    // fresh connections, because that is what a browser arriving at GLPI
    // does, and a pooled one would skip the handshake no real visit skips.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(0)
        .build()
        .unwrap_or_default();

    let (running, version, config_path, workers) = process_facts();

    let config = config_path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let config_readable = config.is_some();

    let settings = config.as_deref().map(directives).unwrap_or_default();
    let stub_status = config.as_deref().is_some_and(|c| c.contains("stub_status"));

    // Static first, because it is the one nginx answers alone - but only if
    // the file is really there. GLPI's vhost ends in
    // `try_files $uri /index.php$is_args$args`, so a missing path is served
    // BY PHP: timing a 404 would measure a GLPI bootstrap and label it nginx,
    // which is worse than having no reading. So the candidates are tried
    // until one answers 200, and if none does the probe reports that rather
    // than falling back to whatever was fastest.
    let base = glpi_url.trim_end_matches('/');
    let mut static_probe = None;
    for path in STATIC_CANDIDATES {
        let p = probe(
            &client,
            &format!("{base}{path}"),
            "static file",
            "nginx alone - served from disk with no PHP involved",
        )
        .await;
        if p.status == Some(200) {
            static_probe = Some(p);
            break;
        }
        // Kept as the fallback REPORT, not as a reading: if nothing answers
        // 200 the page should see the 404 and its path.
        static_probe = static_probe.or(Some(p));
    }

    let probes = vec![
        static_probe.expect("STATIC_CANDIDATES is never empty"),
        probe(
            &client,
            &format!("{}/status.php", glpi_url.trim_end_matches('/')),
            "through php-fpm",
            "nginx plus the FastCGI round trip plus GLPI's bootstrap",
        )
        .await,
    ];

    NginxReport {
        running,
        version,
        config_path,
        config_readable,
        workers,
        stub_status,
        settings,
        probes,
    }
}

/// The master's version and config path, and how many workers there are.
///
/// Read from /proc rather than by running `nginx -V`: the binary is not on
/// this service's PATH on a packaged install, and `cmdline` is world readable
/// where the process itself is not.
fn process_facts() -> (bool, Option<String>, Option<String>, usize) {
    let mut running = false;
    let mut version = None;
    let mut config_path = None;
    let mut workers = 0;

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return (false, None, None, 0);
    };

    for entry in entries.flatten() {
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        // argv is NUL separated, and the last element carries a trailing NUL.
        let args: Vec<String> = raw
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        let Some(argv0) = args.first() else { continue };
        if !argv0.contains("nginx") {
            continue;
        }

        // The master is the one started with a config; workers inherit an
        // argv that says so. Counting "nginx: worker process" by its argv0
        // rather than by parentage keeps this to one pass over /proc.
        if argv0.contains("worker") || args.iter().any(|a| a == "worker") {
            workers += 1;
            continue;
        }

        running = true;
        // Two shapes, and the second is the one that actually occurs. A
        // master that has started rewrites its whole command line into one
        // argv entry - "nginx: master process /path/to/nginx -c /path/conf" -
        // so the separate "-c" element only exists for the brief moment
        // before that. Both are read, argv first.
        config_path = args
            .iter()
            .position(|a| a == "-c")
            .and_then(|pos| args.get(pos + 1).cloned())
            .or_else(|| config_from_title(&args.join(" ")));
        // /nix/store/<hash>-nginx-1.30.5/bin/nginx -> 1.30.5. Absent on a
        // distribution path, where the version is not in the filename.
        version = argv0
            .split('/')
            .find_map(|part| part.strip_prefix("nginx-").map(str::to_string))
            .or_else(|| {
                argv0
                    .split("nginx-")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .map(str::to_string)
            });
    }

    (running, version, config_path, workers)
}

/// Each wanted directive's first occurrence, or None where it is absent.
///
/// First match, not every match: a GLPI install has one vhost, and a config
/// with several would need the server block picked rather than the directive
/// counted - which is the parser this deliberately is not.
fn directives(config: &str) -> Vec<NginxSetting> {
    DIRECTIVES
        .iter()
        .map(|(name, note)| {
            let value = config.lines().find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix(name)?;
                // The name must be the whole first word: `gzip` must not match
                // `gzip_types`, which is a different setting and a long one.
                if !rest.starts_with(char::is_whitespace) {
                    return None;
                }
                let value = rest.trim().trim_end_matches(';').trim();
                (!value.is_empty()).then(|| value.to_string())
            });
            NginxSetting {
                name: (*name).to_string(),
                value,
                note: (*note).to_string(),
            }
        })
        .collect()
}

/// One URL, timed `SAMPLES` times.
async fn probe(client: &reqwest::Client, url: &str, what: &str, detail: &str) -> NginxProbe {
    let mut times = Vec::new();
    let mut status = None;
    let mut error = None;

    for _ in 0..SAMPLES {
        let start = Instant::now();
        match client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(res) => {
                status = Some(res.status().as_u16());
                // Drained before the clock stops: stopping at the headers
                // would time the first byte and call it the request.
                let _ = res.bytes().await;
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    NginxProbe {
        what: what.to_string(),
        detail: detail.to_string(),
        url: url.to_string(),
        median_ms: times.get(times.len() / 2).copied(),
        best_ms: times.first().copied(),
        status,
        error,
    }
}

/// Nothing here needs a database, so the only thing worth asserting is the
/// parsing - and the parsing is where a plausible-looking wrong answer would
/// come from.
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "http {\n\tgzip on;\n\tgzip_types text/plain application/json;\n\
                          \tgzip_comp_level 5;\n\tkeepalive_timeout 65;\n\tsendfile on;\n\
                          \tserver {\n\t\tlisten 443 ssl;\n\t}\n}\n";

    #[test]
    fn a_directive_is_read_with_its_value() {
        let got = directives(SAMPLE);
        let gzip = got.iter().find(|s| s.name == "gzip").unwrap();
        assert_eq!(gzip.value.as_deref(), Some("on"));
    }

    #[test]
    fn a_longer_directive_with_the_same_prefix_is_not_mistaken_for_it() {
        // The bug this test exists for: `gzip` matching `gzip_types` would
        // report the whole MIME list as the value of gzip.
        let got = directives(SAMPLE);
        let gzip = got.iter().find(|s| s.name == "gzip").unwrap();
        assert_eq!(gzip.value.as_deref(), Some("on"));
        let level = got.iter().find(|s| s.name == "gzip_comp_level").unwrap();
        assert_eq!(level.value.as_deref(), Some("5"));
    }

    #[test]
    fn an_absent_directive_is_none_rather_than_empty() {
        let got = directives(SAMPLE);
        let ssl = got.iter().find(|s| s.name == "ssl_session_cache").unwrap();
        assert!(ssl.value.is_none());
    }

    #[test]
    fn the_config_path_is_read_from_a_rewritten_process_title() {
        // The shape /proc actually holds once nginx has started: one argv
        // entry, the real command line inside it.
        let title = "nginx: master process /nix/store/abc-nginx-1.30.5/bin/nginx \
                     -c /nix/store/def-nginx.conf";
        assert_eq!(
            config_from_title(title).as_deref(),
            Some("/nix/store/def-nginx.conf")
        );
    }

    #[test]
    fn a_title_without_a_config_yields_nothing_rather_than_a_guess() {
        assert!(config_from_title("nginx: worker process").is_none());
    }

    #[test]
    fn stub_status_is_detected_by_its_name() {
        assert!(!SAMPLE.contains("stub_status"));
        assert!("location /status { stub_status; }".contains("stub_status"));
    }
}
