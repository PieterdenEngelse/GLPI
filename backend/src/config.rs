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

    /// Optional direct read of GLPI's database, for the two tables neither
    /// API exposes: refused submissions and field-level history.
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

    /// Optional read of two of GLPI's own log files, for the Logs page.
    ///
    /// Absent by default and the page says so rather than showing empty
    /// panes. Set to GLPI's GLPI_LOG_DIR, which on this deployment is
    /// /var/lib/glpi/files/_log - and note that directory is 0750 glpi:glpi,
    /// so this process also needs read access granted to it. Only
    /// access-errors.log and api.log are ever opened; see logs::ALLOWED.
    pub log_dir: Option<String>,
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
            log_dir: std::env::var("GLPI_INV_LOG_DIR")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        })
    }
}
