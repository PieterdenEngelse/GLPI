//! When a GLPI session ends, where each setting that decides it lives, and
//! the one file this app can change.
//!
//! WHY THIS IS NOT ONE SETTING IN ONE PLACE. "Log out after N minutes" reads
//! like a GLPI preference and is not one. GLPI has no session-duration
//! setting at all: it relies on PHP's `session.gc_maxlifetime`, and its own
//! `session` cron task (`CronTask::cronSession`) then reads the same ini
//! value to delete `sess_*` files and to call
//! `SessionTracker::revokeSessionsByAge`. The only logout-adjacent figure
//! GLPI does own is `login_remember_time`, which is the persistent-cookie
//! lifetime and not a timeout. So the three settings here sit in three
//! different places, and which place a setting is in decides whether this
//! app may touch it.
//!
//! WHY A FILE AND NOT THE SERVER, the same answer as `nginx_tuning`: php.ini
//! is a Nix store path and PHP has no runtime settings interface - no
//! equivalent of `SET GLOBAL`. There is no "apply it now" at any price. This
//! writes one Nix file, the user runs the rebuild, and the page says so.
//!
//! WHICH PHP KEYS CAN BE WRITTEN FROM A SEPARATE FILE, measured on this
//! deployment rather than reasoned about. The phpfpm module builds the pool's
//! `-c` ini as `cat ${phpPackage}/etc/php.ini $phpOptionsPath` (nixpkgs
//! `services/web-servers/phpfpm/default.nix`), so pool `phpOptions` lands at
//! the END of that file - but PHP then scans `${phpPackage}/lib`, where
//! `php.buildEnv { extraConfig = ... }` puts its own php.ini, and a scanned
//! file is loaded last and wins. Appending all three of these to a copy of
//! this pool's real `-c` ini and reading them back through the pool's own
//! php binary:
//!
//!   session.gc_maxlifetime = 28800   ->  28800   (absent from extraConfig)
//!   memory_limit = 333M              ->  256M    (extraConfig sets it)
//!   session.cookie_httponly = 0      ->  On      (extraConfig sets it)
//!
//! So a key glpi.nix's `extraConfig` already sets is NOT settable from here,
//! and silently so - the file would be written, the rebuild would succeed,
//! and the value would not change. That is why `Owner::Package` exists
//! instead of letting everything be edited: the same reasoning as
//! `nginx_tuning`'s Generated group, with a quieter failure.
//!
//! COROLLARY WORTH KEEPING: do not "tidy" `session.gc_maxlifetime` into
//! glpi.nix's `extraConfig`. It belongs in neither place twice, and putting
//! it there turns the control on this page into one that writes a file
//! nothing reads.
use std::fmt::Write as _;

/// The phpfpm pool GLPI runs in, which is the pool this file configures.
///
/// A const and not configuration: it is the pool name in glpi.nix, and a
/// deployment that renamed the pool would get a file that parses, rebuilds
/// and changes nothing. The page reports the name for exactly that reason -
/// a wrong pool is visible rather than silent.
pub const POOL: &str = "glpi";

/// The file whose `extraConfig` owns the two cookie flags, named so the page
/// can say where to go instead of offering a control that cannot work.
pub const PACKAGE_INI_SOURCE: &str = "glpi.nix (services.phpfpm.pools.glpi.phpPackage)";

/// Where a setting has to be changed for the change to take effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// `services.phpfpm.pools.<POOL>.phpOptions` - this file. Writable.
    Pool,
    /// The php package's own ini, from `php.buildEnv { extraConfig = ... }`.
    /// Loaded after the pool ini, so it wins. Not writable from here.
    Package,
    /// A row in GLPI's `glpi_configs`, changed on GLPI's own setup screen.
    /// Not writable from here, and the path is where to go.
    ///
    /// Deliberately not a SQL write behind a flag. Every other write in this
    /// app changes something the SERVER owns; this would change something
    /// GLPI owns, behind GLPI's back, and GLPI caches its config in
    /// `$CFG_GLPI` per request and keeps its own history of changes to it.
    /// See CLAUDE.md: nothing here touches the `glpi` schema.
    Glpi { path: &'static str },
}

/// One logout setting: where it lives, what it accepts, and what it means.
pub struct Setting {
    pub name: &'static str,
    pub owner: Owner,
    /// Smallest value the page will write.
    ///
    /// Not cosmetic on `gc_maxlifetime`: PHP reads 0 as "eligible for
    /// collection immediately", and GLPI's own cron task reads the same 0 as
    /// "fall back to a week", so the two halves of one setting would disagree
    /// about what the operator asked for.
    pub min: u64,
    /// Largest value the page will write, for the same reason nginx's sizes
    /// are bounded: a field that accepts 2^64 seconds is not a control.
    pub max: u64,
    /// PHP's or GLPI's own default, shown when nothing sets the value.
    ///
    /// Labelled as a default in the UI and never as a reading: this app does
    /// not run `ini_get`, so an unset value is reported as "PHP's default"
    /// rather than as what the server would say.
    pub default: &'static str,
    /// One line on what the setting decides, for the row.
    pub what: &'static str,
}

/// Every setting that decides when a GLPI session ends.
///
/// Order is the order the page shows them: the one that actually logs people
/// out first, then the cookie it rides in, then the two flags on that cookie,
/// then GLPI's remember-me - which is last because it is the only one that
/// is not a timeout and the easiest to mistake for one.
pub const SETTINGS: &[Setting] = &[
    Setting {
        name: "session.gc_maxlifetime",
        owner: Owner::Pool,
        // A minute floor and a 30-day ceiling. Thirty days because this is
        // the value that keeps a stolen session alive, and a year-long idle
        // timeout is not a setting anyone means.
        min: 60,
        max: 2_592_000,
        default: "1440",
        what: "Idle seconds before a session may be collected. The real logout.",
    },
    Setting {
        name: "session.cookie_lifetime",
        owner: Owner::Pool,
        // 0 is meaningful here and is the default: the cookie dies with the
        // browser. So the floor is 0, unlike the setting above.
        min: 0,
        max: 2_592_000,
        default: "0",
        what: "Seconds the session cookie survives in the browser. 0 means until it closes.",
    },
    Setting {
        name: "session.cookie_httponly",
        owner: Owner::Package,
        min: 0,
        max: 1,
        default: "1",
        what: "Whether script can read the session cookie. Set to 1 by this deployment.",
    },
    Setting {
        name: "session.cookie_samesite",
        owner: Owner::Package,
        min: 0,
        max: 0,
        default: "Lax",
        what: "Cross-site sending rule for the cookie. Set to Lax by this deployment.",
    },
    Setting {
        name: "login_remember_time",
        // GLPI's own screen. Setup > General, under the "Auto Login" heading,
        // where it is called "Time to allow \"Remember Me\"".
        owner: Owner::Glpi {
            path: "/front/config.form.php",
        },
        min: 0,
        max: 5_184_000,
        default: "604800",
        what: "How long a \"Remember Me\" cookie stays valid. Not a timeout.",
    },
];

pub fn setting(name: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.name == name)
}

/// Whether a value is one this setting takes, canonicalised.
///
/// Whole seconds only, and bounded by the setting's own range. The result
/// goes into a Nix string and then into a php.ini that PHP-FPM runs, so the
/// question is not "does it look like a number" but "is it one" - hence a
/// digit check and a parse rather than a pattern. A canonical form is
/// returned, so `0060` is stored as `60` and the page reports what is in the
/// file rather than what was typed.
pub fn check(s: &Setting, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{value:?} is not a whole number of seconds"));
    }
    let n: u64 = value
        .parse()
        .map_err(|_| format!("{value:?} is too large to be seconds"))?;
    if n < s.min {
        return Err(format!(
            "{} must be at least {} seconds; {n} would {}",
            s.name,
            s.min,
            if s.name == "session.gc_maxlifetime" {
                "log everyone out faster than they can read a page"
            } else {
                "be below what this setting accepts"
            }
        ));
    }
    if n > s.max {
        return Err(format!(
            "{} must be at most {} seconds ({} days); {n} is longer than this page will write",
            s.name,
            s.max,
            s.max / 86_400
        ));
    }
    Ok(n.to_string())
}

/// GLPI's `login_remember_time`, out of a v1 `getGlpiConfig` response.
///
/// v1 and not SQL: `getGlpiConfig` returns `$CFG_GLPI` through
/// `getSafeConfig()`, which strips only the eight `Config::$undisclosedFields`
/// secrets, and this key is not one of them - so the API answers it, and the
/// project rule is that SQL is for what the API cannot answer.
///
/// GLPI serves config values as strings about as often as numbers, so both
/// are accepted. `None` means the key was absent, which is reported as
/// unknown rather than as the default - a GLPI that stopped publishing it
/// should not be rendered as a GLPI with a 7-day cookie.
pub fn remember_time(config: &serde_json::Value) -> Option<i64> {
    let v = config
        .get("cfg_glpi")
        .unwrap_or(config)
        .get("login_remember_time")?;
    v.as_i64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// The whole tuning file, from the settings that are set.
///
/// Rewritten whole on every change, like the nginx tuning file and the
/// MariaDB init file: a file assembled by appending is a file nobody can
/// reason about after the third edit.
///
/// `phpOptions` is `types.lines` in the phpfpm module, so this definition
/// MERGES with the one glpi.nix makes rather than colliding with it. That is
/// what lets this be a separate file at all - a `types.str` option would be
/// a duplicate-definition error at rebuild.
pub fn render(set: &[(String, String)]) -> String {
    let mut out = String::from(
        "# Written by glpi-inv. Do not hand-edit: this file is rewritten whole.\n\
         #\n\
         # Session expiry for the GLPI pool, changed from the dashboard.\n\
         # Importing this file is what makes these real, and\n\
         # `nixos-rebuild switch` is what applies them - the app cannot,\n\
         # because php.ini is a read-only store path and PHP has no runtime\n\
         # settings interface.\n\
         #\n\
         # phpOptions is types.lines, so this merges with the pool's own\n\
         # definition in glpi.nix instead of conflicting with it. But a key\n\
         # that glpi.nix's `php.buildEnv { extraConfig }` ALSO sets is loaded\n\
         # after this one and wins, so only keys absent from there appear\n\
         # here.\n\
         { ... }:\n\
         {\n",
    );

    let lines: Vec<String> = set
        .iter()
        .filter(|(name, _)| matches!(setting(name).map(|s| s.owner), Some(Owner::Pool)))
        .map(|(name, value)| format!("    {name} = {value}"))
        .collect();

    if lines.is_empty() {
        // An empty attribute set rather than an empty phpOptions string: a
        // `''\n''` with nothing in it is a line of noise in the pool's ini
        // and reads like something failed to render.
        out.push_str("  # Nothing is set from the dashboard.\n");
    } else {
        let _ = write!(
            out,
            "  services.phpfpm.pools.{POOL}.phpOptions = ''\n{}\n  '';\n",
            lines.join("\n")
        );
    }

    out.push_str("}\n");
    out
}

/// The tuning file on disk: what it currently asks for, and rewriting it.
///
/// A path rather than an open handle, like the other two files this app
/// writes and for the same reason - it is also whatever someone edited on the
/// server since.
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

    /// What the file currently sets, read back from its own output.
    ///
    /// Parsed rather than remembered: the process restarts, and the file is
    /// the state. Unparseable content comes back as nothing set rather than
    /// as an error - the file is replaced whole on the next write anyway, and
    /// refusing to render the page because someone hand-edited it would be
    /// the wrong failure.
    pub fn current(&self) -> Vec<(String, String)> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            // php.ini comments are ';' and Nix's are '#'; this file has both.
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            // Only the keys this page owns, and only as a bare php.ini line:
            // the `services.phpfpm.pools.glpi.phpOptions = ''` line also
            // contains an '=' and must not be read as a setting.
            if matches!(setting(name).map(|s| s.owner), Some(Owner::Pool))
                && value.bytes().all(|b| b.is_ascii_digit())
                && !value.is_empty()
            {
                out.push((name.to_string(), value.to_string()));
            }
        }
        out
    }

    /// Sets one value, or clears it when None, and rewrites the file.
    pub fn set(&self, name: &str, value: Option<&str>) -> Result<Vec<(String, String)>, String> {
        let s = setting(name).ok_or_else(|| format!("{name} is not a setting this page knows"))?;
        match s.owner {
            Owner::Pool => {}
            Owner::Package => {
                return Err(format!(
                    "{name} is set in {PACKAGE_INI_SOURCE}, which PHP loads after this file and \
                     which therefore wins. Writing it here would change nothing and report \
                     success. Change it there."
                ))
            }
            Owner::Glpi { .. } => {
                return Err(format!(
                    "{name} is GLPI's own configuration, not PHP's. Change it in GLPI, under \
                     Setup > General; this app reads GLPI and does not write to it."
                ))
            }
        }

        let mut set = self.current();
        set.retain(|(n, _)| n != name);
        if let Some(v) = value {
            set.push((name.to_string(), check(s, v)?));
        }
        // Written in the order SETTINGS declares, so the file does not
        // reshuffle itself on every change and a diff shows the change rather
        // than the churn.
        set.sort_by_key(|(n, _)| {
            SETTINGS
                .iter()
                .position(|s| s.name == *n)
                .unwrap_or(usize::MAX)
        });

        let text = render(&set);

        // Atomic where it can be, in place where it cannot - the same two
        // cases as the nginx tuning file, and for the same reason: the file
        // is ours and /etc/nixos is root's, so writing a neighbour to rename
        // over the target needs a permission on the DIRECTORY that this
        // process does not have there.
        let tmp = self.path.with_extension("nix.new");
        let atomic = std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, &self.path));
        if let Err(rename_err) = atomic {
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
    fn the_floor_on_gc_maxlifetime_is_not_zero() {
        let s = setting("session.gc_maxlifetime").unwrap();
        assert!(check(s, "0").is_err(), "0 collects sessions immediately");
        assert!(check(s, "59").is_err());
        assert_eq!(check(s, "60").unwrap(), "60");
        assert_eq!(check(s, "28800").unwrap(), "28800");
    }

    /// 0 is the DEFAULT for the cookie and means "until the browser closes",
    /// so the floor that protects gc_maxlifetime must not apply here.
    #[test]
    fn zero_is_a_real_cookie_lifetime() {
        let s = setting("session.cookie_lifetime").unwrap();
        assert_eq!(check(s, "0").unwrap(), "0");
    }

    #[test]
    fn only_whole_seconds_are_accepted() {
        let s = setting("session.gc_maxlifetime").unwrap();
        for v in ["", " ", "8h", "28800s", "-1", "1.5", "1e4", "28800; evil"] {
            assert!(check(s, v).is_err(), "{v:?} must be refused");
        }
        // Canonicalised, so the file holds one spelling of a value.
        assert_eq!(check(s, " 03600 ").unwrap(), "3600");
    }

    #[test]
    fn the_ceiling_is_thirty_days() {
        let s = setting("session.gc_maxlifetime").unwrap();
        assert_eq!(check(s, "2592000").unwrap(), "2592000");
        assert!(check(s, "2592001").is_err());
    }

    /// The whole reason `Owner` exists: these two would be written, the
    /// rebuild would succeed, and the value would not change.
    #[test]
    fn a_key_the_php_package_owns_is_refused_rather_than_written() {
        let dir = std::env::temp_dir().join("glpi-inv-logout-package");
        let file = TuningFile::new(dir.to_str().unwrap());
        for name in ["session.cookie_httponly", "session.cookie_samesite"] {
            let err = file.set(name, Some("1")).unwrap_err();
            assert!(err.contains("loads after this file"), "{err}");
        }
        assert!(!dir.exists(), "a refused write must not create the file");
    }

    #[test]
    fn glpis_own_setting_is_refused_with_where_to_go() {
        let dir = std::env::temp_dir().join("glpi-inv-logout-glpi");
        let file = TuningFile::new(dir.to_str().unwrap());
        let err = file.set("login_remember_time", Some("3600")).unwrap_err();
        assert!(err.contains("Setup > General"), "{err}");
    }

    /// Round trip: what `set` writes is what `current` reads back, in
    /// SETTINGS order rather than submission order.
    #[test]
    fn a_written_value_is_read_back() {
        let path = std::env::temp_dir().join(format!("glpi-inv-logout-{}.nix", std::process::id()));
        let file = TuningFile::new(path.to_str().unwrap());
        let _ = std::fs::remove_file(&path);

        file.set("session.cookie_lifetime", Some("0")).unwrap();
        file.set("session.gc_maxlifetime", Some("28800")).unwrap();

        assert_eq!(
            file.current(),
            vec![
                ("session.gc_maxlifetime".to_string(), "28800".to_string()),
                ("session.cookie_lifetime".to_string(), "0".to_string()),
            ]
        );

        // The option line itself must not be read back as a setting.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!("services.phpfpm.pools.{POOL}.phpOptions")));
        assert!(text.contains("session.gc_maxlifetime = 28800"));

        // Clearing one leaves the other.
        file.set("session.gc_maxlifetime", None).unwrap();
        assert_eq!(
            file.current(),
            vec![("session.cookie_lifetime".to_string(), "0".to_string())]
        );

        // Clearing the last one leaves a file that still parses as Nix.
        file.set("session.cookie_lifetime", None).unwrap();
        assert!(file.current().is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Nothing is set from the dashboard"));
        // The OPTION, not the word: the header comment explains phpOptions
        // and mentions it whether anything is set or not.
        assert!(!text.contains("services.phpfpm.pools"));

        let _ = std::fs::remove_file(&path);
    }

    /// A hand-edited file must not take the page down.
    #[test]
    fn nonsense_in_the_file_reads_as_nothing_set() {
        let path =
            std::env::temp_dir().join(format!("glpi-inv-logout-junk-{}.nix", std::process::id()));
        std::fs::write(&path, "{ this is not nix at all = ; }\nsession.bogus = 5\n").unwrap();
        let file = TuningFile::new(path.to_str().unwrap());
        assert!(file.current().is_empty());
        let _ = std::fs::remove_file(&path);
    }

    /// GLPI serves config values as strings as readily as numbers.
    #[test]
    fn remember_time_reads_both_shapes_glpi_sends() {
        let as_int = serde_json::json!({"cfg_glpi": {"login_remember_time": 604800}});
        let as_str = serde_json::json!({"cfg_glpi": {"login_remember_time": "604800"}});
        assert_eq!(remember_time(&as_int), Some(604_800));
        assert_eq!(remember_time(&as_str), Some(604_800));
        // Unwrapped, as a caller holding only cfg_glpi would pass it.
        assert_eq!(
            remember_time(&serde_json::json!({"login_remember_time": 7200})),
            Some(7200)
        );
        // Absent is unknown, not the default.
        assert_eq!(remember_time(&serde_json::json!({"cfg_glpi": {}})), None);
    }
}
