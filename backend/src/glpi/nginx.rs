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
const DIRECTIVES: [(&str, &str); 16] = [
    (
        "worker_processes",
        "One per core is right; more adds context switching and no throughput, because \
         nginx is event-driven rather than process-per-request.",
    ),
    (
        "worker_connections",
        "Per worker, and a proxied request uses TWO - one to the client, one to \
         php-fpm. Capacity is workers times this, halved.",
    ),
    (
        "worker_rlimit_nofile",
        "File descriptors per worker. Below the connection count the symptom is \
         \"too many open files\" rather than a connection limit.",
    ),
    (
        "http2",
        "One connection multiplexing many small assets, which is the shape of a GLPI \
         page. A directive since nginx 1.25.1, not a listen parameter.",
    ),
    (
        "open_file_cache",
        "The static-file equivalent of opcache: without it every request stats and \
         opens the file again.",
    ),
    (
        "gzip_types",
        "Which types are compressed. application/json has to be named or GLPI's API \
         responses travel whole while the HTML around them is compressed.",
    ),
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
    /// What the server said it was sending. Read rather than assumed: whether
    /// a response is compressed depends on its type matching gzip_types, and
    /// the type is the server's answer rather than the path's extension.
    pub content_type: Option<String>,
    /// `Content-Encoding`, with the request made as a browser makes it -
    /// `Accept-Encoding: gzip`. None means nginx sent it whole, which for a
    /// text or JSON response is a finding: it is the cheapest large saving
    /// available and it is decided by gzip_types.
    pub encoding: Option<String>,
    /// Body bytes actually received. Compressed when `encoding` says so, so
    /// the pair together say what the wire carried.
    pub bytes: Option<usize>,
}

/// One build-time module the guide's readings depend on.
#[derive(Debug, Clone, Serialize)]
pub struct NginxModule {
    pub flag: String,
    pub present: bool,
    pub note: String,
}

/// The modules worth knowing about before advising anything.
///
/// Read from the binary rather than assumed: `stub_status` is the difference
/// between this page having counters and not, and telling someone to add a
/// location for a module their build lacks wastes an afternoon.
const MODULES: [(&str, &str); 4] = [
    (
        "--with-http_stub_status_module",
        "The only counters nginx has: active connections, accepts against handled, \
         reading/writing/waiting. Without it there is nothing to add a location for.",
    ),
    (
        "--with-http_v2_module",
        "HTTP/2. One connection carrying many small assets, which is the shape of a \
         GLPI page.",
    ),
    (
        "--with-http_v3_module",
        "HTTP/3 over QUIC. Present in newer builds; its handshake behaves differently \
         enough that TLS reasoning does not transfer.",
    ),
    (
        "--with-http_gzip_static_module",
        "Serves a precompressed .gz beside a file instead of compressing per request.",
    ),
];

/// nginx's only counters, when a `stub_status` location exists.
///
/// Cumulative since the master started, like MariaDB's, so a rate from them is
/// an average over that period. The two that answer questions nothing else
/// can: `accepts` against `handled` - a difference means connections were
/// DROPPED, from a connection or descriptor limit - and `requests` over
/// `accepts`, which is how many requests a connection carried and therefore
/// whether keepalive is doing anything.
#[derive(Debug, Clone, Serialize)]
pub struct NginxStatus {
    pub url: String,
    pub active: Option<u64>,
    pub accepts: Option<u64>,
    pub handled: Option<u64>,
    pub requests: Option<u64>,
    /// accepts - handled. Non-zero is the finding this endpoint exists for.
    pub dropped: Option<u64>,
    /// requests / accepts. At 1 every request paid a connection setup.
    pub per_connection: Option<f64>,
    pub reading: Option<u64>,
    pub writing: Option<u64>,
    pub waiting: Option<u64>,
    /// Why there are no counters, when there are none.
    pub error: Option<String>,
}

/// The connection ceiling, worked out rather than quoted.
///
/// The guide's arithmetic: capacity is `worker_processes × worker_connections`,
/// and a proxied request uses two of them - one to the client and one to
/// php-fpm - so the number of GLPI requests in flight is half of it. Worth
/// computing here rather than leaving two numbers side by side, because the
/// halving is the part everyone forgets.
#[derive(Debug, Clone, Serialize)]
pub struct NginxCapacity {
    /// From the config, or the observed process count when it says `auto`.
    pub workers: usize,
    /// Whether that came from `auto` rather than from a number.
    pub workers_auto: bool,
    /// Inputs taken from nginx's own defaults because the config does not set
    /// them. Named rather than silently used: "512 because nobody chose" and
    /// "512 because somebody did" are different facts, and the first is the
    /// one worth knowing before advising a change.
    pub defaults_used: Vec<String>,
    pub connections_per_worker: Option<usize>,
    /// Total connections, and the proxied-request figure that is half of it.
    pub connections: Option<usize>,
    pub concurrent_requests: Option<usize>,
    /// `net.core.somaxconn`: the kernel ceiling a `listen ... backlog=` above
    /// it is silently clamped to.
    pub somaxconn: Option<usize>,
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
    /// Empty when the binary could not be run - a packaged install where the
    /// path on the master's command line is not executable by this service.
    /// Distinct from every module being absent, which would be a finding.
    pub modules: Vec<NginxModule>,
    /// The binary the master is running, for the version and the modules.
    pub binary: Option<String>,
    pub capacity: NginxCapacity,
    /// None when no `stub_status` location was found in the config, which is
    /// the usual case and is why `stub_status` above is reported on its own.
    pub status: Option<NginxStatus>,
}

/// Static files to try, in order, for the nginx-alone timing.
///
/// Several because the one that exists is a GLPI version detail, and a
/// hardcoded single path would quietly turn this reading into a PHP one the
/// first time GLPI renamed an asset. All three are served from GLPI's public
/// root by the `location /` block, so a 200 from any of them is nginx reading
/// a file and nothing else.
const STATIC_CANDIDATES: [&str; 3] = ["/pics/favicon.ico", "/js/common.js", "/lib/base.js"];

/// The executable out of a rewritten process title.
///
/// "nginx: master process /path/to/nginx -c /path/conf" -> "/path/to/nginx".
/// The first absolute-looking word, because the prefix is nginx's own wording
/// and is not a path.
fn binary_from_title(title: &str) -> Option<String> {
    title
        .split_whitespace()
        .find(|w| w.starts_with('/') && w.contains("nginx"))
        .map(str::to_string)
}

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

    let (running, version, config_path, workers, binary) = process_facts();

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

    // A path that cannot exist, deliberately. GLPI's vhost ends in
    // `try_files $uri /index.php$is_args$args`, so this is answered by PHP
    // rather than by nginx - and the cost of that is the point: a mistyped or
    // removed asset reference does not cost a 404, it costs a GLPI bootstrap.
    // Showing it beside the other two turns that mechanism from a sentence
    // into a reading.
    //
    // The path is fixed rather than random so the 404 it leaves in the access
    // log is recognisable as this app's probe rather than as a real broken
    // link somebody should chase.
    let missing = probe(
        &client,
        &format!("{base}/glpi-inv-probe-missing-asset"),
        "a path that is not there",
        "nginx has no such file, so try_files hands it to PHP - the cost of a \
         mistyped asset reference",
    )
    .await;

    let probes = vec![
        static_probe.expect("STATIC_CANDIDATES is never empty"),
        missing,
        probe(
            &client,
            &format!("{}/status.php", glpi_url.trim_end_matches('/')),
            "through php-fpm",
            "nginx plus the FastCGI round trip plus GLPI's bootstrap",
        )
        .await,
    ];

    let modules = binary.as_deref().map(module_flags).unwrap_or_default();
    let capacity = capacity(&settings, workers);

    // Only when the config says there is one. Guessing at /nginx_status would
    // put a 404 in somebody's log on every page open, and a 404 from
    // try_files is a GLPI bootstrap - this page measures that cost on purpose
    // once, and should not pay it twice by accident.
    let status = match config.as_deref().and_then(status_location) {
        Some(path) => Some(fetch_status(&client, base, &path).await),
        None => None,
    };

    NginxReport {
        running,
        version,
        config_path,
        config_readable,
        workers,
        stub_status,
        settings,
        probes,
        modules,
        binary,
        capacity,
        status,
    }
}

/// The path of the `location` block that contains `stub_status`.
///
/// Scanned rather than assumed: the conventional names are /nginx_status and
/// /status, and an install that chose a third would otherwise have counters
/// this page could not find. Tracks the most recent `location` seen and
/// returns it when `stub_status` turns up before the next one - which is what
/// "inside that block" means without parsing braces.
fn status_location(config: &str) -> Option<String> {
    let mut current: Option<String> = None;
    for line in config.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("location") {
            // `location = /x {`, `location /x {`, `location ~ ^/x$ {`. Only
            // the literal forms can be requested back, so a regex location is
            // taken as "not findable" rather than guessed at.
            current = rest
                .trim_start()
                .trim_start_matches('=')
                .split_whitespace()
                .next()
                .filter(|p| p.starts_with('/'))
                .map(str::to_string);
        } else if line.starts_with("stub_status") {
            return current;
        }
    }
    None
}

/// GETs a stub_status endpoint and parses its five lines.
async fn fetch_status(client: &reqwest::Client, base: &str, path: &str) -> NginxStatus {
    let url = format!("{base}{path}");
    let body = match client.get(&url).send().await {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        Ok(res) => {
            return NginxStatus {
                url,
                error: Some(format!(
                    "the location exists in the config but answered {}",
                    res.status()
                )),
                ..NginxStatus::empty()
            }
        }
        Err(e) => {
            return NginxStatus {
                url,
                error: Some(e.to_string()),
                ..NginxStatus::empty()
            }
        }
    };

    let mut parsed = parse_status(&body);
    parsed.url = url;
    parsed
}

impl NginxStatus {
    fn empty() -> Self {
        Self {
            url: String::new(),
            active: None,
            accepts: None,
            handled: None,
            requests: None,
            dropped: None,
            per_connection: None,
            reading: None,
            writing: None,
            waiting: None,
            error: None,
        }
    }
}

/// The fixed five-line shape stub_status prints.
///
/// Parsed by the words around the numbers rather than by position, so a
/// future nginx adding a field does not shift every reading by one.
fn parse_status(body: &str) -> NginxStatus {
    let mut out = NginxStatus::empty();

    for line in body.lines() {
        let line = line.trim();
        let nums: Vec<u64> = line
            .split_whitespace()
            .filter_map(|w| w.parse::<u64>().ok())
            .collect();

        if let Some(rest) = line.strip_prefix("Active connections:") {
            out.active = rest.trim().parse().ok();
        } else if line.starts_with("Reading:") {
            // "Reading: 0 Writing: 1 Waiting: 2"
            let pick = |name: &str| {
                line.split(name)
                    .nth(1)
                    .and_then(|r| r.split_whitespace().next())
                    .and_then(|v| v.parse::<u64>().ok())
            };
            out.reading = pick("Reading:");
            out.writing = pick("Writing:");
            out.waiting = pick("Waiting:");
        } else if nums.len() == 3 && !line.contains(':') {
            // The bare "accepts handled requests" triple, which is the only
            // line of three numbers and no label.
            out.accepts = Some(nums[0]);
            out.handled = Some(nums[1]);
            out.requests = Some(nums[2]);
        }
    }

    out.dropped = match (out.accepts, out.handled) {
        (Some(a), Some(h)) => Some(a.saturating_sub(h)),
        _ => None,
    };
    out.per_connection = match (out.requests, out.accepts) {
        (Some(r), Some(a)) if a > 0 => Some(r as f64 / a as f64),
        _ => None,
    };
    out
}

/// The connection ceiling from the config, falling back to what is running.
///
/// `worker_processes auto` is the common setting and is not a number, so the
/// observed process count stands in for it - flagged, because "one per core
/// because that is what auto did" and "one because somebody wrote 1" are
/// different facts about an install.
fn capacity(settings: &[NginxSetting], observed_workers: usize) -> NginxCapacity {
    // nginx's own defaults, for the inputs the config leaves out. Both are
    // from nginx's documentation for the directives below; the figure is
    // useless without them, because an unset directive still has a value and
    // a blank reading reads as "unlimited" to anybody skimming.
    const DEFAULT_WORKER_CONNECTIONS: usize = 512;
    const DEFAULT_WORKER_PROCESSES: usize = 1;

    let value = |name: &str| {
        settings
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.value.clone())
    };
    let mut defaults_used = Vec::new();

    let configured = value("worker_processes");
    let workers_auto = configured.as_deref() == Some("auto");
    let workers = match configured.as_deref() {
        // auto is one per core, which this process cannot read portably - the
        // running count is the same number and is observed rather than
        // assumed.
        Some("auto") => observed_workers,
        Some(v) => v.parse::<usize>().unwrap_or(observed_workers),
        None => {
            defaults_used.push("worker_processes 1".to_string());
            // The observed count is still preferred when it disagrees: it is
            // what is actually running, and a reader chasing capacity cares
            // about that rather than about what the default says.
            if observed_workers > 0 {
                observed_workers
            } else {
                DEFAULT_WORKER_PROCESSES
            }
        }
    };

    let connections_per_worker = match value("worker_connections") {
        Some(v) => v.parse::<usize>().ok(),
        None => {
            defaults_used.push("worker_connections 512".to_string());
            Some(DEFAULT_WORKER_CONNECTIONS)
        }
    };
    let connections = connections_per_worker.map(|c| c.saturating_mul(workers));

    NginxCapacity {
        workers,
        workers_auto,
        defaults_used,
        connections_per_worker,
        connections,
        // Halved because a proxied request holds one connection to the client
        // and one to the upstream. Integer division, so an odd total rounds
        // down - the conservative direction for a ceiling.
        concurrent_requests: connections.map(|c| c / 2),
        somaxconn: std::fs::read_to_string("/proc/sys/net/core/somaxconn")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok()),
    }
}

/// Which of `MODULES` this build has, from `nginx -V`.
///
/// `-V` prints the configure line to stderr and exits; it starts no server and
/// reads no config, so running it is safe even while nginx is serving. An
/// unreadable or unexecutable binary yields an empty list rather than a list
/// of absences - "we could not ask" and "it is not there" are different
/// answers and the page renders them differently.
fn module_flags(binary: &str) -> Vec<NginxModule> {
    let Ok(out) = std::process::Command::new(binary).arg("-V").output() else {
        return Vec::new();
    };
    // The configure line goes to stderr. Both are read because that is a
    // version detail nobody should have to remember.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    if text.trim().is_empty() {
        return Vec::new();
    }
    MODULES
        .iter()
        .map(|(flag, note)| NginxModule {
            flag: (*flag).to_string(),
            present: text.contains(flag),
            note: (*note).to_string(),
        })
        .collect()
}

/// The master's version and config path, and how many workers there are.
///
/// Read from /proc rather than by running `nginx -V`: the binary is not on
/// this service's PATH on a packaged install, and `cmdline` is world readable
/// where the process itself is not.
fn process_facts() -> (bool, Option<String>, Option<String>, usize, Option<String>) {
    let mut running = false;
    let mut version = None;
    let mut config_path = None;
    let mut workers = 0;
    let mut binary = None;

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return (false, None, None, 0, None);
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
        let title = args.join(" ");
        config_path = args
            .iter()
            .position(|a| a == "-c")
            .and_then(|pos| args.get(pos + 1).cloned())
            .or_else(|| config_from_title(&title));
        // The executable, for `-V`. Same two shapes: argv0 is the path until
        // the master rewrites its title, after which the path sits inside it.
        binary = if argv0.starts_with('/') {
            Some(argv0.clone())
        } else {
            binary_from_title(&title)
        };
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

    (running, version, config_path, workers, binary)
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

    let mut content_type = None;
    let mut encoding = None;
    let mut bytes = None;

    for _ in 0..SAMPLES {
        let start = Instant::now();
        match client
            .get(url)
            // As a browser asks. Without this header nginx answers
            // uncompressed whatever gzip_types says, and the reading would
            // describe a request nobody makes.
            .header(reqwest::header::ACCEPT_ENCODING, "gzip")
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(res) => {
                status = Some(res.status().as_u16());
                let header = |name: reqwest::header::HeaderName| {
                    res.headers()
                        .get(name)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string)
                };
                content_type = header(reqwest::header::CONTENT_TYPE);
                encoding = header(reqwest::header::CONTENT_ENCODING);
                // Drained before the clock stops: stopping at the headers
                // would time the first byte and call it the request.
                if let Ok(body) = res.bytes().await {
                    bytes = Some(body.len());
                }
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
        content_type,
        encoding,
        bytes,
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
    fn the_binary_is_read_from_a_rewritten_process_title() {
        let title = "nginx: master process /nix/store/abc-nginx-1.30.5/bin/nginx \
                     -c /nix/store/def-nginx.conf";
        assert_eq!(
            binary_from_title(title).as_deref(),
            Some("/nix/store/abc-nginx-1.30.5/bin/nginx")
        );
    }

    #[test]
    fn a_title_without_a_config_yields_nothing_rather_than_a_guess() {
        assert!(config_from_title("nginx: worker process").is_none());
    }

    #[test]
    fn the_status_location_is_found_by_the_block_it_sits_in() {
        let conf = "http {\n  location / { try_files $uri /index.php; }\n\
                    \n  location = /nginx_status {\n    stub_status;\n  }\n}\n";
        assert_eq!(status_location(conf).as_deref(), Some("/nginx_status"));
    }

    #[test]
    fn no_status_location_means_none_rather_than_a_guess() {
        // The case that must not become a request: guessing /nginx_status
        // here would 404 through PHP on every page open.
        let conf = "http {\n  location / { try_files $uri /index.php; }\n}\n";
        assert!(status_location(conf).is_none());
    }

    #[test]
    fn the_status_body_is_parsed_by_label_not_by_position() {
        let body = "Active connections: 3 \nserver accepts handled requests\n\
                    \t 1024 1020 4096 \nReading: 1 Writing: 2 Waiting: 0 \n";
        let s = parse_status(body);
        assert_eq!(s.active, Some(3));
        assert_eq!(s.accepts, Some(1024));
        assert_eq!(s.handled, Some(1020));
        assert_eq!(s.requests, Some(4096));
        // The whole point of the endpoint: four connections were dropped.
        assert_eq!(s.dropped, Some(4));
        assert_eq!(s.reading, Some(1));
        assert_eq!(s.writing, Some(2));
        assert_eq!(s.waiting, Some(0));
        assert!((s.per_connection.unwrap() - 4.0).abs() < 0.01);
    }

    #[test]
    fn stub_status_is_detected_by_its_name() {
        assert!(!SAMPLE.contains("stub_status"));
        assert!("location /status { stub_status; }".contains("stub_status"));
    }
}
