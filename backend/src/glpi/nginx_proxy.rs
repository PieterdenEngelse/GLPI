//! Whether this nginx proxies anything, and in which direction.
//!
//! A different question from the one the perf page asks about the same
//! server. That page asks what a request costs; this asks where a request
//! goes. nginx can answer from the filesystem, hand the request to an
//! application over a gateway protocol, or pass it to another HTTP server and
//! relay that server's answer back. Only the last is proxying.
//!
//! Read from the config the running master was started with - found the same
//! way the perf page finds it, from the master's own argv - because proxying
//! is a configuration and not a runtime state. nginx publishes no counter for
//! it, and `stub_status` would not say which of its locations proxies even
//! where it exists.
use serde::Serialize;

/// The directives that hand a request to something other than the filesystem.
///
/// All of them are called a pass and only `proxy_pass` is proxying. They are
/// read together because a reverse-proxy count of zero is misread on its own:
/// on a PHP install something certainly leaves nginx, and a reader who is not
/// shown what would conclude the page is broken rather than that the thing
/// leaving is not a proxied request.
const PASS_DIRECTIVES: [(&str, &str); 6] = [
    (
        "proxy_pass",
        "HTTP or HTTPS to another server, with nginx relaying that server's answer back to \
         the client. This is the directive that makes nginx a proxy; nothing else in this \
         list does.",
    ),
    (
        "fastcgi_pass",
        "The FastCGI protocol to an application pool - php-fpm on this install. Binary \
         records over a socket rather than HTTP, and the pool is part of the install rather \
         than a server being fronted, so this is a gateway and not a proxy.",
    ),
    (
        "uwsgi_pass",
        "The uwsgi protocol, which is FastCGI's equivalent for Python applications. A \
         gateway for the same reason.",
    ),
    (
        "scgi_pass",
        "SCGI, the simplest of the gateway protocols. Rare now and present here only so \
         that finding one would not be reported as nothing.",
    ),
    (
        "grpc_pass",
        "gRPC, which is HTTP/2 underneath and so the closest of these to proxying - but it \
         addresses a service method rather than fronting a site, and a client cannot tell \
         nginx where to send it.",
    ),
    (
        "memcached_pass",
        "Answers from memcached directly, with no application involved. A cache read rather \
         than a request handed on.",
    ),
];

/// One pass directive as the config has it.
#[derive(Debug, Clone, Serialize)]
pub struct Pass {
    pub directive: String,
    /// What it passes to, as written: an upstream name, a socket path or a
    /// URL. Left as written rather than resolved - a bare name here may be an
    /// `upstream` block, and whether it is one is visible beside it.
    pub target: String,
    /// The top-level block it sits in, `http` or `stream`. A `stream` pass
    /// moves bytes at the TCP or UDP level and never sees a request, which
    /// makes it proxying in a sense the HTTP question does not cover - so it
    /// is counted apart rather than folded into either tile.
    pub context: String,
    /// Whether the target is built from a variable. A forward proxy has to do
    /// this, because the host comes from the request and is not known when
    /// the config is written. A reverse proxy MAY do it, so this is the shape
    /// of a forward proxy and not proof of one.
    pub dynamic: bool,
    pub note: String,
}

/// A piece of config that does nothing unless nginx proxies.
///
/// Reported present-and-unused rather than left out. Every one of these is in
/// this install's generated config, put there by the NixOS nginx module
/// rather than by anyone configuring a proxy, so a reader who greps for
/// "proxy" finds them and is owed the distinction between a setting that is
/// in force and one that is waiting for a `proxy_pass` that never comes.
#[derive(Debug, Clone, Serialize)]
pub struct Support {
    pub what: String,
    pub present: bool,
    /// Whether anything in the config actually reaches it.
    pub used: bool,
    pub note: String,
}

/// What the config says about proxying.
#[derive(Debug, Clone, Serialize)]
pub struct ProxyReport {
    /// False when no nginx master is running under any user. The rest is then
    /// empty: the config this reads is the one a master names on its command
    /// line, and with no master there is no file to be sure of.
    pub running: bool,
    pub config_path: Option<String>,
    /// False is common and not a fault - the settings are then empty and the
    /// page says which, rather than reporting an absence of proxying it did
    /// not establish.
    pub config_readable: bool,
    /// Every pass directive found, proxying or not.
    pub passes: Vec<Pass>,
    /// `http`-context `proxy_pass` with a fixed target: a reverse proxy, one
    /// per location that has one.
    pub reverse: usize,
    /// `http`-context `proxy_pass` whose target comes from the request. The
    /// forward-proxy shape; `resolver` below is the other half of it.
    pub forward: usize,
    /// `stream`-context passes: TCP or UDP forwarding, which is neither tile.
    pub stream: usize,
    /// Passes that are gateways rather than proxying - what leaves nginx on
    /// an install with no proxy at all.
    pub gateways: usize,
    /// `upstream` block names. A reverse proxy does not need one, so this is
    /// not a second count of the same thing: it says whether the targets are
    /// pools that can be balanced and failed over, or single servers.
    pub upstreams: Vec<String>,
    /// `resolver`, which a forward proxy cannot work without: the host it
    /// must reach is in the request and has to be looked up at request time.
    pub resolver: Option<String>,
    /// From the binary's configure line. None when the binary could not be
    /// run, which is not the same as the module being absent.
    pub proxy_module: Option<bool>,
    pub stream_module: Option<bool>,
    /// `CONNECT`, which a forward proxy needs for anything over HTTPS. No
    /// stock nginx has it - it is a third-party patch - so this is the fact
    /// that decides whether a forward proxy is configurable here at all.
    pub connect_module: Option<bool>,
    pub support: Vec<Support>,
    /// `include` lines this did not follow. A pass inside an included file
    /// would not be counted, so they are named rather than letting a zero
    /// stand for a config this only read part of.
    pub includes: Vec<String>,
}

/// Third-party module names that add forward proxying, as they appear on a
/// configure line. Two spellings because the widely used patch and its forks
/// do not agree on one.
const CONNECT_FLAGS: [&str; 2] = ["proxy_connect", "ngx_http_proxy_connect_module"];

/// Everything readable about proxying, which is the config and the binary.
pub fn report() -> ProxyReport {
    let (running, _version, config_path, _workers, binary) = super::nginx::process_facts();

    let config = config_path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let config_readable = config.is_some();
    let text = config.unwrap_or_default();

    let passes = passes(&text);
    let http_proxy = |p: &&Pass| p.directive == "proxy_pass" && p.context == "http";
    let reverse = passes
        .iter()
        .filter(|p| http_proxy(p) && !p.dynamic)
        .count();
    let forward = passes.iter().filter(|p| http_proxy(p) && p.dynamic).count();
    let stream = passes.iter().filter(|p| p.context == "stream").count();
    let gateways = passes
        .iter()
        .filter(|p| p.directive != "proxy_pass")
        .count();

    // The configure line, read once and asked three questions. None when the
    // binary is not executable by this service, which is the packaged-install
    // case and is why all three module fields are Option.
    let configure = binary.as_deref().and_then(super::nginx::configure_line);
    let (proxy_module, stream_module, connect_module) = match configure.as_deref() {
        // The proxy module is built unless somebody asked for it to be left
        // out, so its flag is the NEGATIVE one. Looking for
        // --with-http_proxy_module would report every stock build as lacking
        // the module it certainly has.
        Some(line) => (
            Some(!line.contains("--without-http_proxy_module")),
            Some(line.contains("--with-stream")),
            Some(CONNECT_FLAGS.iter().any(|f| line.contains(f))),
        ),
        None => (None, None, None),
    };

    let any_http_proxy = reverse + forward > 0;
    let support = support(&text, any_http_proxy);

    ProxyReport {
        running,
        config_path,
        config_readable,
        passes,
        reverse,
        forward,
        stream,
        gateways,
        upstreams: upstreams(&text),
        resolver: value_of(&text, "resolver"),
        proxy_module,
        stream_module,
        connect_module,
        support,
        includes: includes(&text),
    }
}

/// Every pass directive in the config, with the top-level block it sits in.
///
/// Brace depth rather than a parser, for the reason the perf page's directive
/// reader gives for being line-based: the only structure this needs to know
/// about is which top-level block a line is inside, because that is what
/// decides whether a pass is an HTTP request handed on or a TCP connection
/// forwarded. Everything else about the line is in the line.
fn passes(config: &str) -> Vec<Pass> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut context = String::new();

    for line in config.lines() {
        let line = line.trim();
        // A whole-line comment, which is the only kind the generated config
        // has. Not split on '#' mid-line: that would truncate a value that
        // legitimately contains one, and losing a target is worse than
        // counting a commented-out directive that is not there.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // The block being opened at the top level names the context for
        // everything under it, until it closes.
        if depth == 0 && line.contains('{') {
            if let Some(name) = line.split_whitespace().next() {
                context = name.to_string();
            }
        }

        for (directive, note) in PASS_DIRECTIVES {
            let Some(rest) = line.strip_prefix(directive) else {
                continue;
            };
            // The name must be the whole first word. Without this,
            // `proxy_pass` matches nothing wrong today but `fastcgi_pass`
            // would match a hypothetical `fastcgi_pass_header`, and the
            // reader of this file should not have to check which directives
            // happen to be prefixes of which.
            if !rest.starts_with(char::is_whitespace) {
                continue;
            }
            let target = rest.trim().trim_end_matches(';').trim().to_string();
            out.push(Pass {
                directive: directive.to_string(),
                dynamic: target.contains('$'),
                target,
                context: context.clone(),
                note: note.to_string(),
            });
            break;
        }

        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
    }

    out
}

/// The `upstream` block names, in the order the file has them.
fn upstreams(config: &str) -> Vec<String> {
    config
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("upstream")?;
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let name = rest.trim().trim_end_matches('{').trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

/// The `include` lines, as written.
///
/// Reported rather than followed. Following them would mean resolving globs
/// against a directory this service may not be able to read, and a partial
/// answer presented as a whole one is the failure this page is most exposed
/// to: every count here would be wrong by exactly what the include held.
fn includes(config: &str) -> Vec<String> {
    config
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') {
                return None;
            }
            let rest = line.strip_prefix("include")?;
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let path = rest.trim().trim_end_matches(';').trim();
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

/// The first value of a directive, by the whole-first-word rule.
fn value_of(config: &str, name: &str) -> Option<String> {
    config.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let rest = line.strip_prefix(name)?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let value = rest.trim().trim_end_matches(';').trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// How many lines are that directive.
fn count_of(config: &str, name: &str) -> usize {
    config
        .lines()
        .filter(|line| {
            let line = line.trim();
            if line.starts_with('#') {
                return false;
            }
            line.strip_prefix(name)
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        })
        .count()
}

/// The config with its whole-line comments dropped.
///
/// Only for the checks that search the text rather than read a directive -
/// the line-based readers above skip a comment as they go. A comment is
/// documentation of what a config COULD do and this page's whole job is
/// saying what it does.
fn uncommented(config: &str) -> String {
    config
        .lines()
        .filter(|line| !line.trim().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The four pieces of proxy support, and whether anything reaches them.
fn support(config: &str, any_http_proxy: bool) -> Vec<Support> {
    // The map that nginx's own websocket guide tells everyone to write. The
    // NixOS module emits it unconditionally, so it is here on an install with
    // nothing proxied at all. Its own declaration mentions the variable once,
    // which is why a second mention is what counts as a use: that second one
    // can only be a `proxy_set_header Connection` reading it.
    //
    // Counted over the code and not the file. The generated config labels
    // that map with "# $connection_upgrade is used for websocket proxying",
    // and counting that comment made this report the map as in force on an
    // install where nothing reads it - found by running the endpoint against
    // this machine, after a test on a hand-written config had passed.
    let upgrade_mentions = uncommented(config).matches("connection_upgrade").count();
    let set_headers = count_of(config, "proxy_set_header");

    vec![
        Support {
            what: "gzip_proxied".to_string(),
            present: value_of(config, "gzip_proxied").is_some(),
            used: any_http_proxy,
            note: "Whether to compress a response that came from an upstream. nginx \
                   normally will not, because a cache between it and the client may hold \
                   the compressed copy and serve it to a client that cannot read it. With \
                   nothing proxied there is no upstream response to decide about."
                .to_string(),
        },
        Support {
            what: "$connection_upgrade map".to_string(),
            present: upgrade_mentions > 0,
            used: upgrade_mentions > 1,
            note: "Turns a client's Upgrade header into the Connection value a websocket \
                   needs, for passing to an upstream. A map costs nothing to declare and \
                   does nothing until a proxy_set_header reads it, which is the difference \
                   between the two figures here."
                .to_string(),
        },
        Support {
            what: "proxy_set_header".to_string(),
            present: set_headers > 0,
            used: set_headers > 0 && any_http_proxy,
            note: "What the upstream is told about the original request - Host, \
                   X-Forwarded-For, X-Real-IP. Inherited by a location from its server \
                   block, so one of these can sit far from the proxy_pass it applies to, \
                   and applies to nothing where there is no proxy_pass at all."
                .to_string(),
        },
        Support {
            what: "proxy_cache_path".to_string(),
            present: value_of(config, "proxy_cache_path").is_some(),
            // The path declares the store; `proxy_cache` is what switches it
            // on for a location. Declared and never switched on is a real
            // state - disk reserved, nothing cached.
            used: count_of(config, "proxy_cache") > 0,
            note: "Where proxied responses are kept on disk, and for how long. Declaring \
                   the store does not cache anything: a location has to name it with \
                   proxy_cache before a single response is written there."
                .to_string(),
        },
    ]
}

/// The parsing is the whole risk here: a wrong count would look exactly like
/// a right one, and the thing being counted is absent on this install - which
/// is the case a test can confirm and a reading cannot.
#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of this machine's generated config, cut down: one vhost, one
    /// FastCGI gateway, no proxying, and the two inert proxy settings the
    /// NixOS module emits regardless.
    const GLPI: &str = "http {\n\tgzip_proxied expired no-cache;\n\
                        \tmap $http_upgrade $connection_upgrade {\n\t\tdefault upgrade;\n\t}\n\
                        \tserver {\n\t\tlisten 127.0.0.1:8080;\n\
                        \t\tlocation ~ ^/index\\.php$ {\n\
                        \t\t\tfastcgi_pass unix:/run/phpfpm/glpi.sock;\n\t\t}\n\t}\n}\n";

    /// A reverse proxy with a pool, and a stream forwarder beside it.
    const PROXIED: &str = "http {\n\tupstream app {\n\t\tserver 127.0.0.1:3000;\n\t}\n\
                           \tserver {\n\t\tlocation / {\n\t\t\tproxy_pass http://app;\n\
                           \t\t\tproxy_set_header Host $host;\n\t\t}\n\t}\n}\n\
                           stream {\n\tserver {\n\t\tproxy_pass 10.0.0.5:5432;\n\t}\n}\n";

    #[test]
    fn a_fastcgi_gateway_is_not_counted_as_a_proxy() {
        // The finding this page exists to state plainly: something does leave
        // nginx on this install, and it is not a proxied request.
        let got = passes(GLPI);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].directive, "fastcgi_pass");
        assert_eq!(got[0].target, "unix:/run/phpfpm/glpi.sock");
        assert_eq!(got[0].context, "http");
        assert!(!got[0].dynamic);
    }

    #[test]
    fn a_stream_pass_is_not_counted_in_the_http_block() {
        let got = passes(PROXIED);
        let http: Vec<_> = got.iter().filter(|p| p.context == "http").collect();
        let stream: Vec<_> = got.iter().filter(|p| p.context == "stream").collect();
        assert_eq!(http.len(), 1);
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].target, "10.0.0.5:5432");
    }

    #[test]
    fn a_variable_target_is_the_forward_proxy_shape() {
        let conf = "http {\n\tserver {\n\t\tresolver 1.1.1.1;\n\
                    \t\tlocation / {\n\t\t\tproxy_pass http://$http_host$request_uri;\n\
                    \t\t}\n\t}\n}\n";
        let got = passes(conf);
        assert_eq!(got.len(), 1);
        assert!(got[0].dynamic);
        assert_eq!(value_of(conf, "resolver").as_deref(), Some("1.1.1.1"));
    }

    #[test]
    fn an_upstream_block_is_read_by_name() {
        assert_eq!(upstreams(PROXIED), vec!["app".to_string()]);
        assert!(upstreams(GLPI).is_empty());
    }

    #[test]
    fn a_commented_out_pass_is_not_a_pass() {
        // The config this reads is generated, so a commented directive is
        // unlikely - but counting one would turn "no proxy here" into "a
        // proxy somebody disabled", which are different answers.
        let conf = "http {\n\tserver {\n\t\t# proxy_pass http://old;\n\t}\n}\n";
        assert!(passes(conf).is_empty());
    }

    #[test]
    fn the_support_pieces_are_present_and_unused_on_this_config() {
        let got = support(GLPI, false);
        let gzip = got.iter().find(|s| s.what == "gzip_proxied").unwrap();
        assert!(gzip.present && !gzip.used);
        let map = got
            .iter()
            .find(|s| s.what == "$connection_upgrade map")
            .unwrap();
        // Declared once, read never: present and unused, which is the whole
        // reason this struct has two booleans instead of one.
        assert!(map.present && !map.used);
    }

    #[test]
    fn the_comment_labelling_the_map_is_not_a_use_of_it() {
        // The bug this test exists for, and it was the real config that
        // found it: the NixOS module labels the map with a comment naming
        // the variable, so counting mentions over the whole file saw two and
        // reported a websocket proxy on an install that has none.
        let conf = "http {\n\t# $connection_upgrade is used for websocket proxying\n\
                    \tmap $http_upgrade $connection_upgrade {\n\t\tdefault upgrade;\n\t}\n}\n";
        let got = support(conf, false);
        let map = got
            .iter()
            .find(|s| s.what == "$connection_upgrade map")
            .unwrap();
        assert!(map.present && !map.used);
    }

    #[test]
    fn a_map_that_is_read_counts_as_used() {
        let conf = "http {\n\tmap $http_upgrade $connection_upgrade {\n\t\tdefault upgrade;\n\t}\n\
                    \tserver {\n\t\tproxy_set_header Connection $connection_upgrade;\n\t}\n}\n";
        let got = support(conf, true);
        let map = got
            .iter()
            .find(|s| s.what == "$connection_upgrade map")
            .unwrap();
        assert!(map.used);
    }

    #[test]
    fn an_include_is_reported_rather_than_followed() {
        let conf = "http {\n\tinclude /nix/store/abc-mailcap/etc/nginx/mime.types;\n}\n";
        assert_eq!(
            includes(conf),
            vec!["/nix/store/abc-mailcap/etc/nginx/mime.types".to_string()]
        );
    }
}
