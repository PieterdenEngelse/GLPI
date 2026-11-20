//! The startup file MariaDB replays, so a variable outlives a restart.
//!
//! Named `innodb_init` until 2026-11-20, when the variables pages gained a
//! write and this stopped being about InnoDB. Nothing in the mechanism
//! changed with it: the file was always just `SET GLOBAL` statements, and
//! what may go in it was always the server's boundary rather than this
//! app's.
//!
//! WHY A FILE AT ALL. Nothing in MariaDB persists `SET GLOBAL`: there is no
//! `SET PERSIST` (that is MySQL 8), no `mysqld-auto.cnf`, and `mariadbd
//! --help --verbose` on 11.4.12 has no persistence option of any kind. The
//! only thing that outlives a restart is a file the server reads at startup,
//! and on this deployment there is exactly one - the unit runs `mysqld
//! --defaults-file=/etc/my.cnf`, which replaces the whole search path, and
//! that file is a Nix store symlink nothing but a rebuild can change.
//!
//! `--init-file` is the way in. The server replays its SQL before any client
//! connects, so GLPI never sees the pre-init values, and the file itself is
//! an ordinary path that can be rewritten without root once the option has
//! been set once.
//!
//! WHAT IT COVERS, AND WHY THAT LINE IS WHERE IT IS. SQL can only set what
//! `SET GLOBAL` can set, which is exactly the settings this app lets you
//! edit. That was 106 of this server's 137 InnoDB settings when this module
//! was written; since the variables pages gained a write it is 484 of its
//! 693, and this file holds any of them. The rest are taken at startup
//! before any SQL runs and still need my.cnf - on NixOS,
//! `services.mysql.settings`. The boundary is the server's, not a decision
//! made here, which is why widening the app's write widened this with no
//! change to the rule.
//!
//! THE FAILURE MODE, MEASURED RATHER THAN ASSUMED. On MariaDB 11.4.12, a
//! statement in an init file that fails does NOT stop the server: it logs
//! `ERROR: 1193 Unknown system variable` and startup continues. But
//! execution of the file STOPS THERE. Measured on a throwaway instance: with
//! a bad line in the middle, the line before it applied and the line after
//! it never ran; with the bad line last, both earlier lines applied. So one
//! line a later MariaDB no longer understands silently drops every line
//! below it, and only the error log says so.
//!
//! That is why every value is validated against the running server before it
//! is written here, and why the panel shows what this file asks for beside
//! what the server actually has. There is no `--force` for init files and no
//! per-statement continue; detecting the mismatch is the only defence
//! available.
//!
//! The file is rewritten whole, sorted by name, so two settings written in
//! either order produce the same file and a hand edit is not quietly merged
//! with app state that no longer matches it.
//!
//! IT MUST EXIST AND BE READABLE, OR THE DATABASE DOES NOT START. Also
//! measured, and the sharpest edge here by some way: an init-file mysqld
//! cannot open is not a setting that quietly fails - the server logs "Can't
//! create/write to file ... (Errcode: 2)" and then "Aborting", and exits 1.
//! So whoever points MariaDB at this path must create it first (a
//! systemd.tmpfiles `f` rule), and this module never deletes it: dropping
//! the last setting rewrites it with the header and no statements.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The header, rewritten with the file. Addressed to whoever opens it on the
/// server rather than in this repository - the truncation rule is the thing
/// they need and the one thing the file's own contents cannot show.
const HEADER: &str = "\
-- Written by glpi-inv. Do not hand-edit: this file is rewritten whole.
--
-- MariaDB replays these with --init-file at startup, before any client can
-- connect. They are the system variables this app was told to keep across
-- restarts; everything else returns to what my.cnf and the compiled defaults
-- give it.
--
-- Only variables SET GLOBAL can set appear here, which is what an init file
-- can do and nothing more. A setting the server reads at startup still
-- belongs in my.cnf - on NixOS, in services.mysql.settings.
--
-- A statement that fails here does not stop the server - it logs the error
-- and startup continues - but the rest of THIS FILE is then skipped. A line
-- a later MariaDB no longer understands therefore drops every line below it.
-- That is why every value was checked against the running server before it
-- was written, and why the app shows what this file asks for beside what the
-- server actually has.
";

/// One app-owned file of `SET GLOBAL` statements.
///
/// Holds a path and nothing else: the file is the state, re-read on every
/// request rather than cached, because it is also whatever someone edited on
/// the server five minutes ago and a cache would make this app confidently
/// wrong about it.
pub struct InitFile {
    path: PathBuf,
}

impl InitFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What the file asks for, by setting name, as a value rather than as
    /// SQL. This is what the panel compares against the running server.
    pub fn read(&self) -> Result<BTreeMap<String, String>> {
        Ok(self
            .entries()?
            .into_iter()
            .map(|(name, literal)| (name, unquote(&literal)))
            .collect())
    }

    /// The same lines, kept as the SQL literal each one was written with.
    ///
    /// Rewriting goes through this rather than through `read`, so a line is
    /// reproduced exactly as it was validated. The alternative - re-quoting
    /// from the value - silently changed `= 9` into `= '9'` the next time
    /// any other setting was written, which is a different statement for the
    /// server to accept or refuse, decided by an edit to an unrelated row.
    ///
    /// A file that is not there yet is not an error: it is the normal state
    /// before anything has been persisted, and must read as "nothing is
    /// persisted" rather than failing every panel load on a fresh install.
    fn entries(&self) -> Result<BTreeMap<String, String>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", self.path.display())),
        };
        Ok(text.lines().filter_map(parse_line).collect())
    }

    /// Add or replace one setting, with the literal the server's own
    /// metadata produced for it.
    pub fn upsert(&self, name: &str, literal: &str) -> Result<()> {
        let mut lines = self.entries()?;
        lines.insert(name.to_string(), literal.to_string());
        self.write(&lines)
    }

    /// Stop persisting one setting. Removing what is not there succeeds: the
    /// caller asked for a state, not for an event.
    pub fn remove(&self, name: &str) -> Result<()> {
        let mut lines = self.entries()?;
        lines.remove(name);
        self.write(&lines)
    }

    /// Rewrite the file whole.
    ///
    /// Written beside the target and renamed over it. A half-written init
    /// file is read by the server on the next start exactly as it stands, so
    /// the window between truncate and write is a window in which a crash
    /// leaves the database starting from a fragment. rename(2) has no such
    /// window.
    ///
    fn write(&self, lines: &BTreeMap<String, String>) -> Result<()> {
        let mut out = String::from(HEADER);
        for (name, literal) in lines {
            out.push_str(&format!("SET GLOBAL {name} = {literal};\n"));
        }

        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, out).with_context(|| {
            format!(
                "writing {} - the directory must exist and be writable by this service",
                tmp.display()
            )
        })?;
        // World-readable, explicitly, rather than whatever umask this
        // service happened to start with. mysqld reads this file as the
        // mysql user, and a file it cannot read is not a setting that fails
        // to apply - measured on 11.4.12, an init-file it cannot open makes
        // the server ABORT STARTUP: "Can't create/write to file ...
        // (Errcode: 2)" followed by "Aborting". A 0600 file here would mean
        // the database does not come back.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644))
                .with_context(|| format!("making {} readable by the server", tmp.display()))?;
        }

        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))?;
        Ok(())
    }
}

/// `SET GLOBAL name = literal;` -> (name, value), or None for anything else.
///
/// Deliberately strict: this parses what this file writes, and a line it does
/// not recognise is reported as absent rather than guessed at. The panel then
/// shows that setting as not persisted, which is the safe direction to be
/// wrong in - it under-claims rather than promising a restart will keep
/// something it will not.
fn parse_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("SET GLOBAL ")?;
    let rest = rest.strip_suffix(';')?;
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((name.to_string(), value.trim().to_string()))
}

fn unquote(literal: &str) -> String {
    match literal
        .strip_prefix('\'')
        .and_then(|l| l.strip_suffix('\''))
    {
        Some(inner) => inner.replace("\\'", "'").replace("\\\\", "\\"),
        None => literal.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("glpi-inv-init-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&p);
        p
    }

    #[test]
    fn a_missing_file_is_nothing_persisted_not_an_error() {
        let f = InitFile::new(tmpdir().join("absent.sql"));
        assert!(f.read().unwrap().is_empty());
    }

    #[test]
    fn a_value_survives_the_round_trip() {
        let f = InitFile::new(tmpdir().join("round.sql"));
        f.upsert("innodb_default_row_format", "'compact'").unwrap();
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        let got = f.read().unwrap();
        assert_eq!(got["innodb_default_row_format"], "compact");
        assert_eq!(got["innodb_print_all_deadlocks"], "ON");
        std::fs::remove_file(f.path()).unwrap();
    }

    /// Numbers are written unquoted, as the server's own literal, and must
    /// still read back as the value rather than as a quoted string.
    #[test]
    fn a_number_reads_back_as_itself() {
        let f = InitFile::new(tmpdir().join("num.sql"));
        f.upsert("innodb_buffer_pool_size", "134217728").unwrap();
        assert_eq!(f.read().unwrap()["innodb_buffer_pool_size"], "134217728");
        std::fs::remove_file(f.path()).unwrap();
    }

    #[test]
    fn upsert_replaces_rather_than_appends() {
        let f = InitFile::new(tmpdir().join("replace.sql"));
        f.upsert("innodb_default_row_format", "'compact'").unwrap();
        f.upsert("innodb_default_row_format", "'dynamic'").unwrap();
        let text = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(text.matches("innodb_default_row_format").count(), 1);
        assert_eq!(f.read().unwrap()["innodb_default_row_format"], "dynamic");
        std::fs::remove_file(f.path()).unwrap();
    }

    #[test]
    fn remove_takes_one_line_and_leaves_the_rest() {
        let f = InitFile::new(tmpdir().join("remove.sql"));
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        f.upsert("innodb_default_row_format", "'compact'").unwrap();
        f.remove("innodb_print_all_deadlocks").unwrap();
        let got = f.read().unwrap();
        assert!(!got.contains_key("innodb_print_all_deadlocks"));
        assert_eq!(got["innodb_default_row_format"], "compact");
        std::fs::remove_file(f.path()).unwrap();
    }

    /// The header is comments, and a comment is not a setting. Reading its
    /// own output back must find only the statements.
    #[test]
    fn the_header_is_not_mistaken_for_settings() {
        let f = InitFile::new(tmpdir().join("header.sql"));
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        assert_eq!(f.read().unwrap().len(), 1);
        std::fs::remove_file(f.path()).unwrap();
    }

    #[test]
    fn a_line_this_app_did_not_write_is_reported_as_absent() {
        assert_eq!(parse_line("DROP DATABASE glpi;"), None);
        assert_eq!(parse_line("SET GLOBAL innodb_x = 1"), None); // no semicolon
        assert_eq!(parse_line("-- SET GLOBAL innodb_x = 1;"), None);
        assert_eq!(
            parse_line("SET GLOBAL innodb_x = 'a';"),
            Some(("innodb_x".into(), "'a'".into()))
        );
    }

    /// Not a style point: mysqld aborts startup on an init-file it cannot
    /// read, so the mode is load-bearing.
    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_by_the_server() {
        use std::os::unix::fs::PermissionsExt;
        let f = InitFile::new(tmpdir().join("mode.sql"));
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        let mode = std::fs::metadata(f.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o044, 0o044, "mode was {:o}", mode);
        std::fs::remove_file(f.path()).unwrap();
    }

    /// Dropping the last setting must leave a file behind, for the same
    /// reason: no file, no database.
    #[test]
    fn removing_the_last_setting_leaves_the_file() {
        let f = InitFile::new(tmpdir().join("last.sql"));
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        f.remove("innodb_print_all_deadlocks").unwrap();
        assert!(f.path().exists());
        assert!(f.read().unwrap().is_empty());
        std::fs::remove_file(f.path()).unwrap();
    }

    #[test]
    fn quotes_in_a_value_survive_the_read() {
        assert_eq!(unquote(r"'O\'Brien'"), "O'Brien");
        assert_eq!(unquote(r"'a\\b'"), r"a\b");
    }

    /// The bug this shape exists to prevent: writing one setting used to
    /// re-quote every other line, turning `= 9` into `= '9'` - a different
    /// statement for the server to accept, produced by editing a row that
    /// had nothing to do with it.
    #[test]
    fn writing_one_line_leaves_the_others_byte_identical() {
        let f = InitFile::new(tmpdir().join("stable.sql"));
        f.upsert("innodb_stats_transient_sample_pages", "9")
            .unwrap();
        f.upsert("innodb_print_all_deadlocks", "'ON'").unwrap();
        let text = std::fs::read_to_string(f.path()).unwrap();
        assert!(
            text.contains("SET GLOBAL innodb_stats_transient_sample_pages = 9;"),
            "{text}"
        );
        std::fs::remove_file(f.path()).unwrap();
    }
}
