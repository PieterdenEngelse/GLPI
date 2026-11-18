//! Configuration, from the environment.
//!
//! The OAuth client secret is read from the environment and never written to
//! a file in the repo or to the Nix store - both are world-readable. See the
//! .env note in CLAUDE.md.
use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    /// Where this backend listens. Loopback by default: this is a local
    /// dashboard, and anything wider is an explicit choice.
    pub bind: String,

    /// GLPI's base URL, e.g. http://localhost:8080 - NOT including /api.php.
    pub glpi_url: String,

    /// OAuth2 client credentials, created in GLPI under its API settings.
    /// The HL API uses OAuth2; there is no simple user-token path to it,
    /// unlike the legacy /apirest.php API.
    pub client_id: String,
    pub client_secret: String,

    /// A GLPI USER to act as. The High-Level API authorises per user - it
    /// checks Session::getLoginUserID() - so a client-only token is refused
    /// however valid it is. See the long note in glpi::Client::token.
    ///
    /// Use a dedicated service account with a read-only profile, not a
    /// super-admin: this app only ever reads.
    pub username: String,
    pub password: String,

    /// Optional direct read of GLPI's database, for what neither API
    /// exposes: refused submissions and field-level history.
    ///
    /// The account's grant stopped being the limit on 2026-11-13 - it now
    /// holds all privileges server-wide - so what this app reads is decided
    /// by the code below and nothing else. One query, one table. See
    /// CLAUDE.md for what that widening cost and what it did not.
    ///
    /// Absent by default and the app is fully functional without it - the
    /// Refused page then says it is not configured rather than showing an
    /// empty table, because "no refusals" and "cannot look" mean opposite
    /// things.
    ///
    /// Expected to use socket authentication, which needs no password:
    ///   mysql://pde@localhost/glpi?socket=/run/mysqld/mysqld.sock
    /// The account should hold SELECT on glpi_refusedequipments and
    /// glpi_logs and nothing else.
    pub db_url: Option<String>,

    /// Whether the two restart links on the Maria page may run
    /// scripts/db-restart.sh.
    ///
    /// Off unless GLPI_INV_DB_RESTART is set. Deliberately NOT the same flag
    /// as GLPI_INV_INNODB_WRITES: that one governs SET GLOBAL, which is
    /// reversible by setting the value back, while this one restarts GLPI's
    /// database and takes every in-flight inventory import with it. The
    /// script still asks polkit before it touches a system unit, so an
    /// operator consents to the action itself; this flag is what keeps an
    /// unattended install from offering it at all.
    pub db_restart: bool,

    /// Optional read of two of GLPI's own log files, for the Logs page.
    ///
    /// Absent by default and the page says so rather than showing empty
    /// panes. Set to GLPI's GLPI_LOG_DIR, which on this deployment is
    /// /var/lib/glpi/files/_log - and note that directory is 0750 glpi:glpi,
    /// so this process also needs read access granted to it. Only
    /// access-errors.log and api.log are ever opened; see logs::ALLOWED.
    pub log_dir: Option<String>,

    /// Whether the InnoDB panel may write. Off unless GLPI_INV_INNODB_WRITES
    /// is set, and off is the right state for an install nobody is sitting
    /// at: every other request this backend makes is a read, and this is the
    /// one that changes how the server GLPI runs on behaves.
    ///
    /// It gates the endpoint, not just the button. A page is a thing anyone
    /// who can reach the port can forge a request against, so the check that
    /// matters is the one in the handler.
    pub innodb_writes: bool,

    /// The file MariaDB replays at startup, when one is configured.
    ///
    /// Absent by default, and the panel then says a change lasts until the
    /// next restart - which is the truth, and a better one than a Persist
    /// button that writes a file nothing reads.
    ///
    /// Making it take effect is a one-time root step this app cannot do:
    /// `settings.mysqld.init-file` in the NixOS config, pointing here, plus
    /// a directory this service can write. It cannot live under /home - the
    /// mysql unit runs with ProtectHome=true, so the server cannot see it
    /// there.
    pub innodb_init_file: Option<String>,
}

/// Whether a flag that permits a write is set, by one rule for all of them.
///
/// `1`, `true`, `yes` or `on` - case and surrounding space ignored - and
/// nothing else. Anything unrecognised is OFF, which matters more than it
/// looks: these two flags used to disagree, because they were written in
/// separate sessions. GLPI_INV_INNODB_WRITES read any non-empty value as
/// consent, so `GLPI_INV_INNODB_WRITES=no` ENABLED writes, while
/// `GLPI_INV_DB_RESTART=no` did not. Two conventions in one env file is a
/// trap whichever way each one leans, and for a flag that permits a write
/// the safe default is to need the word.
fn enabled(var: &str) -> bool {
    permits(std::env::var(var).ok().as_deref())
}

/// The rule itself, split from the lookup so it can be tested without
/// setting a process-wide environment variable from a test.
fn permits(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind: std::env::var("GLPI_INV_BIND").unwrap_or_else(|_| "127.0.0.1:3200".into()),
            glpi_url: std::env::var("GLPI_URL").unwrap_or_else(|_| "http://localhost:8080".into()),
            client_id: std::env::var("GLPI_CLIENT_ID")
                .context("GLPI_CLIENT_ID is not set - create an OAuth client in GLPI first")?,
            client_secret: std::env::var("GLPI_CLIENT_SECRET")
                .context("GLPI_CLIENT_SECRET is not set")?,
            username: std::env::var("GLPI_USERNAME")
                .context("GLPI_USERNAME is not set - the API authorises per user")?,
            password: std::env::var("GLPI_PASSWORD").context("GLPI_PASSWORD is not set")?,
            db_url: std::env::var("GLPI_DB_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            db_restart: enabled("GLPI_INV_DB_RESTART"),
            log_dir: std::env::var("GLPI_INV_LOG_DIR")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            innodb_init_file: std::env::var("GLPI_INV_INNODB_INIT_FILE")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            innodb_writes: enabled("GLPI_INV_INNODB_WRITES"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::permits;

    #[test]
    fn the_four_words_permit() {
        for v in ["1", "true", "yes", "on", "TRUE", " Yes ", "ON"] {
            assert!(permits(Some(v)), "{v:?} should permit");
        }
    }

    /// The regression this rule exists for. GLPI_INV_INNODB_WRITES used to
    /// read any non-empty value as consent, so a reader who wrote the
    /// obvious thing turned the write ON.
    #[test]
    fn a_word_that_means_no_does_not_permit() {
        for v in ["no", "No", "off", "false", "0", "disabled", "", " "] {
            assert!(!permits(Some(v)), "{v:?} must not permit");
        }
    }

    #[test]
    fn anything_unrecognised_is_off() {
        for v in ["maybe", "2", "yes please", "true-ish"] {
            assert!(!permits(Some(v)), "{v:?} must not permit");
        }
    }

    #[test]
    fn unset_is_off() {
        assert!(!permits(None));
    }
}
