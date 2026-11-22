//! The history sampler: the schema this app keeps its own readings in, and
//! the MariaDB EVENT that fills it.
//!
//! WHY THE APP OWNS THIS. The perf page's history panels read
//! `glpi_inv.status_samples`. For a day that table existed only because it
//! had been created by hand over SQL, which meant a fresh clone, a datadir
//! restore or a new machine got a server with no sampler and a panel saying
//! so. Putting the definition here makes the app carry it: the only way to
//! have the app is to have the schema it stores in.
//!
//! WHAT IT MAY TOUCH, and the line is the whole point. `glpi_inv` is THIS
//! APP'S schema. Nothing here writes to `glpi`, reads from it, or alters
//! anything GLPI owns, and that restriction is not a convention to be
//! relaxed later - GLPI maintains its own history, entity scoping and
//! derived state, and writing behind it corrupts the install in ways its UI
//! will not show. A history table inside `glpi` would have broken that rule
//! for a dashboard's convenience.
//!
//! WHY IT IS NOT BEHIND A FLAG, when every other write in this app is. The
//! flags - `GLPI_INV_INNODB_WRITES`, `GLPI_INV_VARIABLE_WRITES`,
//! `GLPI_INV_DB_RESTART` - all gate changes to something SOMEONE ELSE owns:
//! the server's running configuration, its startup file, its lifecycle. A
//! flag is right there because the blast radius is the whole install. This
//! creates one schema that belongs to this app and writes rows to it. Gating
//! it would also defeat the purpose: a flag off by default leaves a fresh
//! install with no sampler, which is the exact thing being fixed.
//!
//! WHY IT NEVER FAILS STARTUP. The account may not hold CREATE, or the
//! server may be one this app only reads. Either way the other six pages
//! work, so a failure here is logged and the history panel renders its "not
//! sampling" state - which is already a state it has, and a truthful one.
use anyhow::{Context, Result};
use mysql_async::prelude::*;

use super::refused::{Db, SAMPLED_COUNTERS};

/// The schema this app stores its own readings in. Never `glpi`.
pub const SCHEMA: &str = "glpi_inv";

/// How long a sample is kept. 14 days because the question the history
/// answers is "what did last week look like"; a year of minutes is a
/// different product and a different storage decision.
pub const KEEP_DAYS: u32 = 14;

impl Db {
    /// Creates the schema, the table and the event if they are not there,
    /// and leaves them alone if they are.
    ///
    /// Idempotent by construction: IF NOT EXISTS on the schema and the table
    /// so existing samples are never dropped, and CREATE OR REPLACE on the
    /// event so a changed counter list takes effect on the next start
    /// without a migration. Replacing the event loses nothing - it holds no
    /// state, only a schedule.
    pub async fn ensure_sampler(&self) -> Result<()> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        format!(
            "CREATE DATABASE IF NOT EXISTS {SCHEMA} \
             CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )
        .ignore(&mut conn)
        .await
        .with_context(|| format!("creating the {SCHEMA} schema"))?;

        // Narrow rather than a column per counter, so adding one is a change
        // to SAMPLED_COUNTERS and never a migration. The key is
        // (name, taken_at) and not the reverse because every read is one
        // series over a window, which that order makes a range scan.
        format!(
            "CREATE TABLE IF NOT EXISTS {SCHEMA}.status_samples ( \
               name      VARCHAR(64)  NOT NULL, \
               taken_at  TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP, \
               value     BIGINT       NOT NULL, \
               PRIMARY KEY (name, taken_at) \
             ) ENGINE=InnoDB"
        )
        .ignore(&mut conn)
        .await
        .context("creating the status_samples table")?;

        // PHP's timings, which no EVENT can collect: the database cannot
        // make an HTTP request, so this table is filled by a task in this
        // process instead. Same schema, same pruning, different writer -
        // and that asymmetry is the whole reason the two are separate
        // tables rather than one with a "kind" column.
        format!(
            "CREATE TABLE IF NOT EXISTS {SCHEMA}.php_samples ( \
               name      VARCHAR(32)  NOT NULL, \
               taken_at  TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP, \
               ms        DOUBLE       NOT NULL, \
               ok        TINYINT(1)   NOT NULL, \
               PRIMARY KEY (name, taken_at) \
             ) ENGINE=InnoDB"
        )
        .ignore(&mut conn)
        .await
        .context("creating the php_samples table")?;

        // An event that exists but is never run is the failure mode this
        // line prevents, and it is easy to miss: event_scheduler defaults to
        // OFF, so a correctly defined event simply never fires and the table
        // stays empty with nothing reporting an error.
        "SET GLOBAL event_scheduler = ON"
            .ignore(&mut conn)
            .await
            .context("turning the event scheduler on")?;

        event_sql()
            .ignore(&mut conn)
            .await
            .context("creating the sampling event")?;

        Ok(())
    }
}

/// What each sampled counter means, and what a reading of it is worth.
///
/// This app's words, not the server's: MariaDB publishes no description for
/// any `GLOBAL_STATUS` row - the table is two columns, a name and a value -
/// so every line here is a judgement and is written as one. Keyed by the
/// server's own name so a counter that stops existing falls through to an
/// honest default rather than being quietly labelled as another.
///
/// Each says what the number is, and then the part a bare figure cannot
/// carry: what makes it worth looking at, or why it usually is not.
pub const COUNTER_NOTES: [(&str, &str); 16] = [
    (
        "QUESTIONS",
        "Every statement the server was asked to run, including the ones this \
         dashboard and the sampler itself issue - so it never reads zero on a \
         machine that is watching it. Its shape matters more than its height: \
         GLPI's load is people and agents, so a flat line through the night \
         with a daytime hump is the expected picture, and a plateau that does \
         not fall is usually something automated rather than somebody working.",
    ),
    (
        "COM_SELECT",
        "Reads, counted separately from the four write verbs beside it. On this \
         install it should dominate all of them together: GLPI is read-heavy \
         between inventories, and a SELECT line that drops toward the write \
         lines means either nobody is using the UI or something is doing bulk \
         writes.",
    ),
    (
        "COM_INSERT",
        "New rows. This is the verb an inventory submission is made of - an \
         agent reporting a machine GLPI has not seen writes assets, software, \
         ports and log rows in one transaction - so inserts arrive in bursts \
         on the agent timer rather than evenly.",
    ),
    (
        "COM_UPDATE",
        "Changed rows, and the normal shape of a RE-inventory: an agent \
         reporting a machine GLPI already knows updates far more than it \
         inserts. Updates rising while inserts stay flat is the fleet being \
         re-scanned; both rising together is the fleet growing.",
    ),
    (
        "COM_DELETE",
        "Removed rows. Expected to be near zero almost always - GLPI soft-deletes \
         assets by setting is_deleted rather than removing them, which is an \
         UPDATE. Sustained deletes are a purge, a plugin, or GLPI's own cleanup \
         cron, and worth knowing which.",
    ),
    (
        "SLOW_QUERIES",
        "Statements that ran longer than long_query_time - 0.5s on this server. \
         The count only says how many; WHICH ones is the slow log tile on the \
         perf page, and that is the one worth opening. A rise here with no \
         rise in queries is the interesting case: the same work suddenly \
         costing more.",
    ),
    (
        "CONNECTIONS",
        "Connection attempts, successful or not, counted since startup - so this \
         is a rate of NEW connections rather than a count of open ones. PHP \
         opens one per request unless persistent connections are on, so this \
         tracks GLPI's request rate more closely than it tracks anything about \
         the database.",
    ),
    (
        "ABORTED_CLIENTS",
        "Connections that ended without a close - the client went away, timed \
         out, or was killed. A steady trickle is normal from browsers \
         navigating away mid-request. A step change is not, and usually means \
         PHP processes dying or a timeout somewhere between GLPI and here.",
    ),
    (
        "THREADS_CONNECTED",
        "A GAUGE, not a counter: how many connections are open right now, drawn \
         as it was read rather than as a difference. It is the number to check \
         against max_connections, because the connection limit is one of the \
         few settings that locks everyone out when it is reached - including \
         whatever would be used to raise it again.",
    ),
    (
        "THREADS_RUNNING",
        "Also a gauge, and the more telling of the two: how many of those \
         connections are actually executing rather than sitting idle. Connected \
         can be high and harmless; running high is contention. A running count \
         that stays near the core count while queries do not rise is the \
         signature of queries waiting on each other.",
    ),
    (
        "INNODB_BUFFER_POOL_READ_REQUESTS",
        "Pages InnoDB was asked for. Read together with the misses beside it and \
         nothing else - on its own this is just how busy the engine was. The \
         ratio of the two is the buffer pool hit rate, which is the figure \
         people actually mean when they ask whether the pool is big enough.",
    ),
    (
        "INNODB_BUFFER_POOL_READS",
        "Requests the pool could NOT answer, so InnoDB went to disk. This is the \
         half of the hit rate that matters: near zero means the working set \
         fits in memory, and a sustained rise means it no longer does. GLPI's \
         database here is around 51 MB against a buffer pool far larger than \
         that, so anything but near-zero is a surprise worth chasing.",
    ),
    (
        "INNODB_ROW_LOCK_WAITS",
        "Times a transaction had to wait for a row another one held. Expected to \
         be zero on a single-user install. It is the counter that explains a \
         slow GLPI nobody can reproduce: each inventory import is one \
         transaction touching the asset, its software and its ports, so two \
         agents reporting the same machine at once is exactly how this becomes \
         non-zero.",
    ),
    (
        "CREATED_TMP_DISK_TABLES",
        "Temporary tables that would not fit in memory and were written to disk. \
         A query needing a temp table is normal - GROUP BY and some joins do - \
         but one spilling to disk is orders of magnitude slower than one that \
         did not. Rises here and in slow queries together point at the same \
         statements, and tmp_table_size is the setting involved.",
    ),
    (
        "OPENED_TABLES",
        "Tables opened since startup, which should flatten and stay flat: the \
         table cache holds them open, so after a warm-up this only climbs when \
         the cache is too small and tables are being evicted and reopened. A \
         line that never stops rising is table_open_cache, not load.",
    ),
    (
        "TABLE_LOCKS_WAITED",
        "Times a table-level lock had to be waited for. On an all-InnoDB install \
         this should stay at zero, because InnoDB locks rows rather than \
         tables - every one of GLPI's 447 tables is InnoDB. Anything here means \
         something took a table lock explicitly, or a non-InnoDB table appeared.",
    ),
];

/// This app's note on a counter, by the server's own name.
///
/// None for a counter it has never seen, which the panel says plainly rather
/// than showing an empty button: a reading with no explanation is still a
/// reading, and pretending otherwise would hide that the list has drifted.
pub fn counter_note(name: &str) -> Option<&'static str> {
    COUNTER_NOTES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, body)| *body)
}

/// The GLPI endpoints the PHP sampler times, and what each one is.
///
/// Both are unauthenticated and small, which is what makes them safe to call
/// every minute forever: `status.php` runs GLPI's own health checks and
/// `index.php` renders the login page. Neither is a heavy page, so these are
/// a FLOOR - the overhead every request pays before doing anything
/// particular - and not a typical page. A filtered list view costs this plus
/// its own work.
///
/// The same two the /perf readings use, deliberately: the point of keeping
/// them is to say whether the number measured now is normal, and that only
/// works if it is the same measurement.
pub const PHP_ENDPOINTS: [(&str, &str); 2] = [
    (
        "status.php",
        "GLPI\u{2019}s health check. The cheapest thing it serves, so this is close to \
         pure PHP startup: interpreter, autoloader, config, database handshake. A rise \
         here is rarely about a page and usually about the install - opcache cold, the \
         database slow to answer, or the filesystem.",
    ),
    (
        "index.php",
        "The login page, rendered. Costs everything status.php costs plus a real \
         template, which makes it the better stand-in for what a person waits for. \
         Watch it against status.php rather than alone: the two rising together is the \
         install, index.php alone is rendering.",
    ),
];

/// This app's note on a PHP endpoint, by path.
pub fn php_note(name: &str) -> Option<&'static str> {
    PHP_ENDPOINTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

/// The event, built from `SAMPLED_COUNTERS` rather than written out beside
/// it.
///
/// This used to be a hand-written IN list on the server, with the same names
/// typed again in Rust - two lists that drifted silently, which is why
/// `StatusHistory::missing` exists to report the drift. Generating it means
/// there is one list, and the drift it reported cannot happen from this side
/// any more. `missing` is kept for the case it still catches: an event
/// edited by hand on the server.
///
/// The names are compile-time constants from this crate, never input, so
/// there is nothing here for a value to escape from.
fn event_sql() -> String {
    let names = SAMPLED_COUNTERS
        .iter()
        .map(|(name, _, _)| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "CREATE OR REPLACE EVENT {SCHEMA}.sample_status \
           ON SCHEDULE EVERY 1 MINUTE \
           ON COMPLETION PRESERVE \
           COMMENT 'glpi-inv: GLOBAL_STATUS snapshot, pruned to {KEEP_DAYS} days' \
           DO BEGIN \
             INSERT IGNORE INTO {SCHEMA}.status_samples (name, taken_at, value) \
             SELECT VARIABLE_NAME, NOW(), CAST(VARIABLE_VALUE AS SIGNED) \
               FROM information_schema.GLOBAL_STATUS \
              WHERE VARIABLE_NAME IN ({names}); \
             DELETE FROM {SCHEMA}.status_samples \
              WHERE taken_at < NOW() - INTERVAL {KEEP_DAYS} DAY; \
           END"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated event must name every counter the reader expects, or
    /// the panel shows empty charts for whatever was left out.
    #[test]
    fn the_event_names_every_sampled_counter() {
        let sql = event_sql();
        for (name, _, _) in SAMPLED_COUNTERS {
            assert!(
                sql.contains(&format!("'{name}'")),
                "{name} missing from the event"
            );
        }
    }

    /// It writes to this app's schema and reads information_schema, and
    /// touches GLPI's schema nowhere. The rule is in the module docs; this
    /// is the part of it a test can hold.
    #[test]
    fn the_event_never_mentions_glpis_schema() {
        let sql = event_sql();
        assert!(sql.contains("glpi_inv.status_samples"));
        // "glpi_inv" contains "glpi", so the check has to be for the schema
        // used as a qualifier rather than for the substring.
        assert!(!sql.contains(" glpi."), "{sql}");
        assert!(!sql.contains("INTO glpi."), "{sql}");
    }

    /// Every sampled counter needs a note, or its panel ships an info button
    /// with nothing behind it. The two lists are separate consts and this is
    /// what keeps them in step.
    #[test]
    fn every_sampled_counter_has_a_note() {
        for (name, _, _) in SAMPLED_COUNTERS {
            assert!(counter_note(name).is_some(), "no note for {name}");
        }
    }

    /// Same rule as the counters: a sampled endpoint with no note ships an
    /// info button with nothing behind it.
    #[test]
    fn every_php_endpoint_has_a_note() {
        for (name, _) in PHP_ENDPOINTS {
            assert!(php_note(name).is_some(), "no note for {name}");
        }
        assert!(php_note("ajax/telemetry.php").is_none());
    }

    #[test]
    fn a_counter_this_app_has_never_seen_has_no_note() {
        assert!(counter_note("WSREP_FLOW_CONTROL_PAUSED").is_none());
    }

    #[test]
    fn the_event_prunes_and_is_replaceable() {
        let sql = event_sql();
        assert!(sql.contains("CREATE OR REPLACE EVENT"));
        assert!(sql.contains(&format!("INTERVAL {KEEP_DAYS} DAY")));
    }
}
