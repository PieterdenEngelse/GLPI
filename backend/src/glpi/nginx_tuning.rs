//! The settings the nginx page can change, and the file it changes them in.
//!
//! WHY A FILE AND NOT THE SERVER. nginx's running config is a Nix store
//! path (mode 444, on a read-only mount), and nginx has no runtime settings
//! interface at all, no equivalent of `SET GLOBAL`. So unlike the MariaDB
//! variables page there is no "apply it now" at any price. The only way in
//! is a different config, which means a rebuild.
//!
//! So this writes ONE Nix file that the system config imports, and the user
//! runs the rebuild. The app never applies anything, and the page says so.
//!
//! THREE GROUPS, and which group a setting is in is a fact about the
//! generated config rather than a preference:
//!
//! - **Append** - the directive is absent from the generated config, so a
//!   tuning file may add it. Measured, not assumed: `worker_processes`,
//!   `worker_connections`, `worker_rlimit_nofile`, `open_file_cache` and
//!   `fastcgi_read_timeout` appear zero times in the config this machine is
//!   running.
//! - **Option** - the directive is already emitted, but NixOS has a dedicated
//!   option that REPLACES it. Setting the option changes the value; appending
//!   the directive would duplicate it.
//! - **Generated** - already emitted by one of the `recommended*` flags, with
//!   no option to override it. Not editable here, and the page names the flag
//!   rather than offering a control that would break the server.
//!
//! THE DUPLICATE IS NOT A WARNING. nginx refuses to start on a repeated
//! `sendfile` or `keepalive_timeout`, so emitting a Generated directive into
//! this file produces a config that fails at rebuild time. That is the whole
//! reason the third group exists instead of letting everything be edited.
use std::fmt::Write as _;

/// Where a setting's value has to go to take effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// `services.nginx.<option>`, as a raw directive appended there.
    Append(&'static str),
    /// `services.nginx.<option> = <value>`, a typed option that replaces the
    /// generated line.
    Option_(&'static str),
    /// Emitted by this `recommended*` flag; nothing here can change it.
    Generated(&'static str),
}

/// What a value is allowed to look like.
///
/// Whitelists per kind rather than one permissive pattern: this text ends up
/// inside a Nix string and then inside an nginx config that runs as root, so
/// the question is not "does it look wrong" but "is it one of the shapes this
/// setting takes".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A positive integer, or the word `auto`.
    Count,
    /// A positive integer with an optional k/m/g suffix: `10m`, `512k`.
    Size,
    /// `on` or `off`.
    Toggle,
    /// A positive integer with an optional time suffix: `60`, `60s`, `5m`.
    Duration,
    /// nginx's `open_file_cache` form: `off`, or `max=N` with optional
    /// `inactive=<duration>`.
    FileCache,
    /// One or more `TLSv1`, `TLSv1.1`, `TLSv1.2`, `TLSv1.3`, space separated.
    TlsProtocols,
    /// nginx's `resolver` form: one or more IP addresses, with an optional
    /// `valid=<duration>` and `ipv6=on|off`.
    Resolver,
}

/// One tunable, with where it goes and what it accepts.
pub struct Tunable {
    pub name: &'static str,
    pub route: Route,
    pub kind: Kind,
}

/// Every setting the page knows how to change, or knows it cannot.
///
/// The Append five were each confirmed absent from this machine's generated
/// nginx.conf before being listed here. If a future NixOS starts emitting one
/// of them, it moves to Generated or Option_ - adding it here would be a
/// config that will not start.
pub const TUNABLES: [Tunable; 17] = [
    Tunable {
        name: "worker_processes",
        route: Route::Append("appendConfig"),
        kind: Kind::Count,
    },
    Tunable {
        name: "worker_rlimit_nofile",
        route: Route::Append("appendConfig"),
        kind: Kind::Count,
    },
    Tunable {
        name: "worker_connections",
        route: Route::Append("eventsConfig"),
        kind: Kind::Count,
    },
    Tunable {
        name: "open_file_cache",
        route: Route::Append("commonHttpConfig"),
        kind: Kind::FileCache,
    },
    Tunable {
        name: "fastcgi_read_timeout",
        route: Route::Append("commonHttpConfig"),
        kind: Kind::Duration,
    },
    // commonHttpConfig and not appendConfig, and the difference is the whole
    // reason this one can exist: `resolver` is only valid inside http,
    // server, location or stream, and appendConfig lands in MAIN context
    // after the http block - which nginx rejects outright. A control that
    // wrote it there would hand the reader a server that will not start.
    //
    // Absent from this machine's generated config, checked the same way the
    // other four were: the proxy page reads the running config and reports
    // no resolver at all.
    Tunable {
        name: "resolver",
        route: Route::Append("commonHttpConfig"),
        kind: Kind::Resolver,
    },
    Tunable {
        name: "client_max_body_size",
        route: Route::Option_("clientMaxBodySize"),
        kind: Kind::Size,
    },
    Tunable {
        name: "server_tokens",
        route: Route::Option_("serverTokens"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "ssl_protocols",
        route: Route::Option_("sslProtocols"),
        kind: Kind::TlsProtocols,
    },
    Tunable {
        name: "gzip",
        route: Route::Generated("recommendedGzipSettings"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "gzip_comp_level",
        route: Route::Generated("recommendedGzipSettings"),
        kind: Kind::Count,
    },
    Tunable {
        name: "gzip_types",
        route: Route::Generated("recommendedGzipSettings"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "keepalive_timeout",
        route: Route::Generated("recommendedOptimisation"),
        kind: Kind::Duration,
    },
    Tunable {
        name: "sendfile",
        route: Route::Generated("recommendedOptimisation"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "tcp_nopush",
        route: Route::Generated("recommendedOptimisation"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "ssl_session_cache",
        route: Route::Generated("recommendedTlsSettings"),
        kind: Kind::Toggle,
    },
    Tunable {
        name: "http2",
        route: Route::Generated("the vhost's listen block"),
        kind: Kind::Toggle,
    },
];

pub fn tunable(name: &str) -> Option<&'static Tunable> {
    TUNABLES.iter().find(|t| t.name == name)
}

/// -> the value as it may be written, or why it may not be.
///
/// Returns the CANONICAL form rather than the submitted text, so what reaches
/// the file is assembled here from parts that matched rather than passed
/// through. Nothing a caller sends survives verbatim.
pub fn check(kind: Kind, value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err("no value".into());
    }
    match kind {
        Kind::Count => {
            if v == "auto" {
                return Ok("auto".into());
            }
            number(v).map(|n| n.to_string())
        }
        Kind::Toggle => match v.to_ascii_lowercase().as_str() {
            "on" => Ok("on".into()),
            "off" => Ok("off".into()),
            _ => Err("takes on or off".into()),
        },
        Kind::Size => {
            let (digits, suffix) = split_suffix(v, &['k', 'm', 'g']);
            let n = number(digits)?;
            Ok(format!("{n}{suffix}"))
        }
        Kind::Duration => {
            let (digits, suffix) = split_suffix(v, &['s', 'm', 'h']);
            let n = number(digits)?;
            Ok(format!("{n}{suffix}"))
        }
        Kind::FileCache => {
            if v.eq_ignore_ascii_case("off") {
                return Ok("off".into());
            }
            // Rebuilt from the parts that matched, in a fixed order, so a
            // third token or a repeated one cannot ride along.
            let mut max = None;
            let mut inactive = None;
            for part in v.split_whitespace() {
                if let Some(n) = part.strip_prefix("max=") {
                    max = Some(number(n)?);
                } else if let Some(d) = part.strip_prefix("inactive=") {
                    let (digits, suffix) = split_suffix(d, &['s', 'm', 'h']);
                    inactive = Some(format!("{}{}", number(digits)?, suffix));
                } else {
                    return Err(format!("{part:?} is not max= or inactive="));
                }
            }
            let max = max.ok_or("needs max=N, or off")?;
            Ok(match inactive {
                Some(i) => format!("max={max} inactive={i}"),
                None => format!("max={max}"),
            })
        }
        Kind::Resolver => {
            // Rebuilt from the parts that matched, like FileCache above and
            // for the sharper version of the same reason: this value ends up
            // as a directive nginx consults for every name it has to look
            // up, and an address is the one thing here that decides where a
            // request actually goes.
            let mut addresses = Vec::new();
            let mut valid = None;
            let mut ipv6 = None;
            for part in v.split_whitespace() {
                if let Some(d) = part.strip_prefix("valid=") {
                    let (digits, suffix) = split_suffix(d, &['s', 'm', 'h']);
                    valid = Some(format!("{}{}", number(digits)?, suffix));
                } else if let Some(f) = part.strip_prefix("ipv6=") {
                    ipv6 = Some(match f.to_ascii_lowercase().as_str() {
                        "on" => "on",
                        "off" => "off",
                        _ => return Err("ipv6= takes on or off".into()),
                    });
                } else {
                    addresses.push(address(part)?);
                }
            }
            if addresses.is_empty() {
                return Err("needs at least one IP address".into());
            }
            let mut out = addresses.join(" ");
            // Fixed order, not submission order: nginx accepts the flags
            // anywhere, and a file whose lines differ only in how someone
            // typed them is a file nobody can diff.
            if let Some(t) = valid {
                let _ = write!(out, " valid={t}");
            }
            if let Some(f) = ipv6 {
                let _ = write!(out, " ipv6={f}");
            }
            Ok(out)
        }
        Kind::TlsProtocols => {
            let mut out = Vec::new();
            for part in v.split_whitespace() {
                match part {
                    "TLSv1" | "TLSv1.1" | "TLSv1.2" | "TLSv1.3" => out.push(part),
                    _ => return Err(format!("{part:?} is not a TLS version nginx names")),
                }
            }
            Ok(out.join(" "))
        }
    }
}

/// One resolver address, canonicalised by the standard library's parser.
///
/// Parsed and re-rendered rather than pattern-matched: `std` already knows
/// every shape an IP address takes, and a regex that merely looks right is
/// the thing this module refuses to rely on. A name is NOT accepted - nginx
/// would take one, but a resolver that has to resolve its own resolver is a
/// bootstrap problem the page cannot explain in a field.
fn address(s: &str) -> Result<String, String> {
    // A bracketed v6 literal, which is how nginx writes one when a port
    // follows. Ports are not accepted here, so the brackets are dropped and
    // the address is rendered bare - still valid, and one spelling instead
    // of two.
    let bare = s
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or(s);
    bare.parse::<std::net::IpAddr>()
        .map(|ip| ip.to_string())
        .map_err(|_| format!("{s:?} is not an IP address"))
}

fn number(s: &str) -> Result<u64, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{s:?} is not a whole number"));
    }
    s.parse::<u64>().map_err(|_| "number too large".to_string())
}

/// Splits a trailing unit off a number, lowercased, or returns it whole.
fn split_suffix<'a>(v: &'a str, units: &[char]) -> (&'a str, String) {
    let lower = v.to_ascii_lowercase();
    if let Some(last) = lower.chars().last() {
        if units.contains(&last) {
            return (&v[..v.len() - 1], last.to_string());
        }
    }
    (v, String::new())
}

/// The whole tuning file, from the settings that are set.
///
/// Rewritten whole on every change, like the MariaDB init file and for the
/// same reason: a file assembled by appending is a file nobody can reason
/// about after the third edit.
pub fn render(set: &[(String, String)]) -> String {
    let mut out = String::from(
        "# Written by glpi-inv. Do not hand-edit: this file is rewritten whole.\n\
         #\n\
         # nginx settings changed from the dashboard. Importing this file is what\n\
         # makes them real, and `nixos-rebuild switch` is what applies them - the\n\
         # app cannot, because nginx's config is a read-only store path and nginx\n\
         # has no runtime settings interface.\n\
         #\n\
         # Only settings absent from the generated config, or having a NixOS\n\
         # option that replaces them, can appear here. A directive already\n\
         # emitted by recommendedGzipSettings, recommendedOptimisation or\n\
         # recommendedTlsSettings would be a DUPLICATE, and nginx refuses to\n\
         # start on one - so those are not editable rather than being written\n\
         # and failing at rebuild.\n\
         { ... }:\n\
         {\n",
    );

    // Grouped by where they go, so the file reads as the three Nix options it
    // actually sets rather than as a list in submission order.
    for option in ["appendConfig", "eventsConfig", "commonHttpConfig"] {
        let lines: Vec<String> = set
            .iter()
            .filter_map(|(name, value)| match tunable(name)?.route {
                Route::Append(o) if o == option => Some(format!("    {name} {value};")),
                _ => None,
            })
            .collect();
        if !lines.is_empty() {
            let _ = write!(
                out,
                "  services.nginx.{option} = ''\n{}\n  '';\n",
                lines.join("\n")
            );
        }
    }

    for (name, value) in set {
        if let Some(Route::Option_(option)) = tunable(name).map(|t| t.route) {
            // serverTokens is the one boolean; the rest are strings. Getting
            // this wrong is a type error at rebuild, not at runtime.
            let rendered = if option == "serverTokens" {
                (value == "on").to_string()
            } else {
                format!("\"{value}\"")
            };
            let _ = writeln!(out, "  services.nginx.{option} = {rendered};");
        }
    }

    out.push_str("}\n");
    out
}

/// The tuning file on disk: what it currently asks for, and rewriting it.
///
/// A path rather than an open handle, like the MariaDB init file and for the
/// same reason - it is also whatever someone edited on the server since.
pub struct TuningFile {
    path: std::path::PathBuf,
}

impl TuningFile {
    pub fn new(path: &str) -> Self {
        Self {
            path: std::path::PathBuf::from(path),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// What the file currently sets, by reading back the directives it wrote.
    ///
    /// Parsed from this app's own output rather than kept in memory: the
    /// process restarts, and the file is the state. Unparseable content comes
    /// back as nothing set rather than as an error - the file is replaced
    /// whole on the next write anyway, and refusing to render the page
    /// because someone hand-edited it would be the wrong failure.
    pub fn current(&self) -> Vec<(String, String)> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            // `  worker_processes 4;` from the append blocks.
            if let Some(body) = line.strip_suffix(';') {
                if let Some((name, value)) = body.split_once(' ') {
                    if matches!(tunable(name).map(|t| t.route), Some(Route::Append(_))) {
                        out.push((name.to_string(), value.trim().to_string()));
                        continue;
                    }
                }
            }
            // `  services.nginx.clientMaxBodySize = "20m";` from the options.
            if let Some(rest) = line.strip_prefix("services.nginx.") {
                if let Some((option, value)) = rest.split_once(" = ") {
                    let value = value.trim_end_matches(';').trim().trim_matches('"');
                    // Matched on the option's NAME rather than by building
                    // a Route: Route::Option_ holds a &'static str, and
                    // `option` is borrowed from the file just read.
                    if let Some(t) = TUNABLES
                        .iter()
                        .find(|t| matches!(t.route, Route::Option_(o) if o == option))
                    {
                        // serverTokens is written as a Nix boolean and read
                        // back as the on/off the page speaks.
                        let value = match value {
                            "true" => "on".to_string(),
                            "false" => "off".to_string(),
                            other => other.to_string(),
                        };
                        out.push((t.name.to_string(), value));
                    }
                }
            }
        }
        out
    }

    /// Sets one value, or clears it when None, and rewrites the file.
    ///
    /// Written to a neighbour and renamed over the target, so a rebuild that
    /// runs mid-write reads either the old file or the new one and never half
    /// of one. The same reason `deploy` renames the backend into place.
    pub fn set(&self, name: &str, value: Option<&str>) -> Result<Vec<(String, String)>, String> {
        let t = tunable(name).ok_or_else(|| format!("{name} is not a setting this page knows"))?;
        if let Route::Generated(flag) = t.route {
            return Err(format!(
                "{name} is already set by {flag}. Writing it here would repeat the                  directive, and nginx refuses to start on a duplicate."
            ));
        }

        let mut set = self.current();
        set.retain(|(n, _)| n != name);
        if let Some(v) = value {
            set.push((name.to_string(), check(t.kind, v)?));
        }
        // Written in the order TUNABLES declares, so the file does not
        // reshuffle itself every time a value changes and a diff shows the
        // change rather than the churn.
        set.sort_by_key(|(n, _)| {
            TUNABLES
                .iter()
                .position(|t| t.name == *n)
                .unwrap_or(usize::MAX)
        });

        let text = render(&set);

        // Atomic where it can be, in place where it cannot.
        //
        // The rename is preferred: a rebuild reading the file mid-write then
        // sees the old one or the new one, never half of one. But writing a
        // NEIGHBOUR needs write permission on the DIRECTORY, and the natural
        // home for this file is /etc/nixos, which is root's. The file itself
        // is ours; its parent is not. Measured here - the first live write
        // failed with "Permission denied" on nginx-tuning.nix.new while the
        // target beside it was mode 644 pde.
        //
        // The fallback truncates and rewrites in place, needing only the
        // permission we have. What that costs is a window where a concurrent
        // rebuild could read a partial file - microseconds for under a
        // kilobyte, and it fails as a loud Nix parse error rather than
        // silently, which is the right failure for something nobody should
        // be doing at the same time anyway.
        let tmp = self.path.with_extension("nix.new");
        let atomic = std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, &self.path));
        if let Err(rename_err) = atomic {
            // Remove the neighbour if the WRITE succeeded and the rename
            // failed; a stray .nix.new in /etc/nixos is picked up by nothing
            // but sits there looking like a leftover.
            let _ = std::fs::remove_file(&tmp);
            std::fs::write(&self.path, &text).map_err(|e| {
                format!(
                    "writing {}: {e} (no atomic write either: {rename_err})",
                    self.path.display()
                )
            })?;
        }
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_takes_digits_or_auto() {
        assert_eq!(check(Kind::Count, " 4 ").unwrap(), "4");
        assert_eq!(check(Kind::Count, "auto").unwrap(), "auto");
        assert!(check(Kind::Count, "4; rm -rf /").is_err());
        assert!(check(Kind::Count, "-1").is_err());
        assert!(check(Kind::Count, "").is_err());
    }

    #[test]
    fn a_size_keeps_its_unit_and_nothing_else() {
        assert_eq!(check(Kind::Size, "10M").unwrap(), "10m");
        assert_eq!(check(Kind::Size, "512").unwrap(), "512");
        assert!(check(Kind::Size, "10mb").is_err());
        assert!(check(Kind::Size, "10m; worker_processes 99").is_err());
    }

    /// The value is REBUILT from matched parts, so an extra token cannot ride
    /// in behind two valid ones.
    #[test]
    fn a_file_cache_is_reassembled_from_what_matched() {
        assert_eq!(
            check(Kind::FileCache, "max=1000 inactive=20s").unwrap(),
            "max=1000 inactive=20s"
        );
        assert_eq!(check(Kind::FileCache, "off").unwrap(), "off");
        assert!(check(Kind::FileCache, "max=1000 valid=30s").is_err());
        assert!(check(Kind::FileCache, "inactive=20s").is_err());
    }

    #[test]
    fn tls_protocols_must_be_ones_nginx_names() {
        assert_eq!(
            check(Kind::TlsProtocols, "TLSv1.2 TLSv1.3").unwrap(),
            "TLSv1.2 TLSv1.3"
        );
        assert!(check(Kind::TlsProtocols, "TLSv1.2 SSLv3").is_err());
    }

    /// The group that would produce a config nginx will not start.
    #[test]
    fn the_recommended_directives_are_not_appendable() {
        for name in ["sendfile", "keepalive_timeout", "gzip_types", "http2"] {
            let t = tunable(name).unwrap();
            assert!(
                matches!(t.route, Route::Generated(_)),
                "{name} must stay Generated - appending it duplicates a directive \
                 and nginx refuses to start"
            );
        }
    }

    /// And the five that were measured absent stay appendable.
    #[test]
    fn the_absent_directives_are_appendable() {
        for name in [
            "worker_processes",
            "worker_connections",
            "worker_rlimit_nofile",
            "open_file_cache",
            "fastcgi_read_timeout",
        ] {
            assert!(
                matches!(tunable(name).unwrap().route, Route::Append(_)),
                "{name}"
            );
        }
    }

    #[test]
    fn rendering_groups_by_the_option_each_one_sets() {
        let set = vec![
            ("worker_processes".to_string(), "4".to_string()),
            ("worker_connections".to_string(), "2048".to_string()),
            ("client_max_body_size".to_string(), "20m".to_string()),
            ("server_tokens".to_string(), "off".to_string()),
        ];
        let out = render(&set);
        assert!(out.contains("services.nginx.appendConfig = ''\n    worker_processes 4;"));
        assert!(out.contains("services.nginx.eventsConfig = ''\n    worker_connections 2048;"));
        assert!(out.contains("services.nginx.clientMaxBodySize = \"20m\";"));
        // The one boolean option, not a quoted string.
        assert!(out.contains("services.nginx.serverTokens = false;"));
        assert!(out.ends_with("}\n"));
    }

    /// What is written has to be readable again, or the page shows blank
    /// fields over a file that is setting things.
    #[test]
    fn what_is_written_reads_back() {
        let dir = std::env::temp_dir().join(format!("nginx-tuning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = TuningFile::new(dir.join("t.nix").to_str().unwrap());

        f.set("worker_processes", Some("4")).unwrap();
        f.set("client_max_body_size", Some("20M")).unwrap();
        f.set("server_tokens", Some("off")).unwrap();
        let back = f.current();

        assert_eq!(
            back.iter()
                .find(|(n, _)| n == "worker_processes")
                .unwrap()
                .1,
            "4"
        );
        // Canonicalised on the way in, not echoed.
        assert_eq!(
            back.iter()
                .find(|(n, _)| n == "client_max_body_size")
                .unwrap()
                .1,
            "20m"
        );
        // Nix boolean on disk, on/off to the page.
        assert_eq!(
            back.iter().find(|(n, _)| n == "server_tokens").unwrap().1,
            "off"
        );

        f.set("worker_processes", None).unwrap();
        assert!(!f.current().iter().any(|(n, _)| n == "worker_processes"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The /etc/nixos case: the file is ours, the directory is not, so no
    /// neighbouring temp file can be created and the write lands in place.
    #[test]
    fn a_file_in_an_unwritable_directory_is_still_written() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("nginx-ro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.nix");
        std::fs::write(&path, "{ ... }:\n{\n}\n").unwrap();

        let mut perms = std::fs::metadata(&dir).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&dir, perms).unwrap();

        let f = TuningFile::new(path.to_str().unwrap());
        let result = f.set("worker_connections", Some("2048"));

        // Restored before asserting, or a failure leaves a dir nothing can
        // clean up.
        let mut p = std::fs::metadata(&dir).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&dir, p).unwrap();

        assert!(result.is_ok(), "{result:?}");
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("worker_connections 2048;"));
        assert!(!dir.join("t.nix.new").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_generated_directive_is_refused_with_its_reason() {
        let dir = std::env::temp_dir().join(format!("nginx-gen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = TuningFile::new(dir.join("t.nix").to_str().unwrap());
        let e = f.set("sendfile", Some("off")).unwrap_err();
        assert!(e.contains("recommendedOptimisation"), "{e}");
        assert!(e.contains("duplicate"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_resolver_is_rebuilt_in_a_fixed_order() {
        // Submitted flags-first, written addresses-first: the file has one
        // spelling whatever anyone typed.
        let got = check(Kind::Resolver, "ipv6=off 1.1.1.1 valid=30s 9.9.9.9").unwrap();
        assert_eq!(got, "1.1.1.1 9.9.9.9 valid=30s ipv6=off");
    }

    #[test]
    fn a_resolver_with_one_address_and_nothing_else_is_enough() {
        assert_eq!(check(Kind::Resolver, "127.0.0.53").unwrap(), "127.0.0.53");
    }

    #[test]
    fn a_bracketed_v6_resolver_is_written_bare() {
        // One spelling, and nginx takes it: the brackets exist to separate a
        // v6 address from a port, and no port is accepted here.
        assert_eq!(
            check(Kind::Resolver, "[2001:4860:4860::8888]").unwrap(),
            "2001:4860:4860::8888"
        );
    }

    #[test]
    fn a_resolver_hostname_is_refused_by_name() {
        // nginx would accept one. This does not: a resolver that has to
        // resolve its own resolver cannot be explained in a text field.
        let e = check(Kind::Resolver, "dns.example.com").unwrap_err();
        assert!(e.contains("not an IP address"), "{e}");
    }

    #[test]
    fn a_resolver_with_no_address_is_refused() {
        let e = check(Kind::Resolver, "valid=30s").unwrap_err();
        assert!(e.contains("at least one IP address"), "{e}");
    }

    #[test]
    fn a_resolver_flag_that_is_not_one_is_refused() {
        assert!(check(Kind::Resolver, "1.1.1.1 ipv6=maybe").is_err());
        assert!(check(Kind::Resolver, "1.1.1.1 status_zone=dns").is_err());
    }

    #[test]
    fn an_injected_resolver_value_is_not_a_value() {
        // The shape the whole check() design exists for: this text ends up
        // inside a Nix string inside a config that runs as root.
        assert!(check(Kind::Resolver, "1.1.1.1; rm -rf /").is_err());
        assert!(check(Kind::Resolver, "1.1.1.1\n  worker_processes 99").is_err());
    }

    #[test]
    fn the_resolver_goes_into_the_http_block_and_not_main_context() {
        // The failure this routing avoids: nginx rejects `resolver` in main
        // context, so appendConfig would produce a server that does not
        // start. The test is on the ROUTE rather than on the rendered line
        // because that is where the mistake would be made.
        let t = tunable("resolver").unwrap();
        assert_eq!(t.route, Route::Append("commonHttpConfig"));

        let out = render(&[("resolver".to_string(), "1.1.1.1 valid=30s".to_string())]);
        assert!(out.contains("commonHttpConfig"), "{out}");
        assert!(out.contains("resolver 1.1.1.1 valid=30s;"), "{out}");
        assert!(!out.contains("appendConfig"), "{out}");
    }

    #[test]
    fn an_empty_set_still_renders_a_valid_module() {
        let out = render(&[]);
        assert!(out.contains("{ ... }:"));
        assert!(out.trim_end().ends_with('}'));
        assert!(!out.contains("services.nginx"));
    }
}
