//! Submissions GLPI threw away.
//!
//! The third inventory failure, and the only one that looks healthy from
//! every other angle. An agent submits, an import rule refuses it, the agent
//! exits 0, and no asset appears or updates. Nothing in the agent's log says
//! so.
//!
//! WHY THIS READS THE DATABASE. `RefusedEquipment` is exposed by neither API:
//! zero occurrences under src/Glpi/Api/HL/, and v1 has no route for it
//! either. The table is the only source. See ~/docs/gl-sql-plan.md for the
//! trade and the grant.
//!
//! The account is expected to hold SELECT on this table and glpi_logs and
//! nothing else - narrower than the API account, which can read every
//! computer, agent, profile and user in the install.
use anyhow::{Context, Result};
use mysql_async::prelude::*;
use serde::Serialize;

/// Rows are capped. This table grows without bound on a busy GLPI, and a
/// page showing the most recent refusals does not need the history.
const ROW_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct Refused {
    pub id: i64,
    pub name: Option<String>,
    pub itemtype: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,
    pub ip: Option<String>,
    pub mac: Option<String>,
    pub method: Option<String>,

    /// The rule that refused it - the actionable field. Shown as an id and
    /// linked into GLPI rather than resolved to a name here: resolving would
    /// need SELECT on glpi_rules too, and the grant is deliberately two
    /// tables.
    pub rules_id: Option<i64>,

    /// The agent that sent it, when GLPI recorded one. Ties a refusal back
    /// to a row on the Agents page.
    pub agents_id: Option<i64>,

    pub entities_id: Option<i64>,
    pub date_creation: Option<String>,
}

/// One row exactly as the driver hands it back, before it becomes a
/// `Refused`. Named because the column order is the contract between the
/// SELECT and the mapping below, and a twelve-field tuple inline hides that.
type Row = (
    i64,
    Option<String>, // name
    Option<String>, // itemtype
    Option<String>, // serial
    Option<String>, // uuid
    Option<String>, // ip
    Option<String>, // mac
    Option<String>, // method
    Option<i64>,    // rules_id
    Option<i64>,    // agents_id
    Option<i64>,    // entities_id
    Option<String>, // date_creation
);

pub struct Db {
    pool: mysql_async::Pool,
}

impl Db {
    pub fn new(url: &str) -> Result<Self> {
        let opts = mysql_async::Opts::from_url(url).context("parsing GLPI_DB_URL")?;
        Ok(Self {
            pool: mysql_async::Pool::new(opts),
        })
    }

    /// Recent refusals, newest first, restricted to the entities the caller
    /// can already see through the API.
    ///
    /// The entity filter is not a permission check - the database account
    /// has no notion of GLPI entities and could read every row. It is there
    /// so that this page and every other page mean the same thing by
    /// "everything". A screen mixing entity-filtered API rows with
    /// unfiltered SQL rows would have two different scopes and no way to
    /// tell which you are looking at.
    ///
    /// An empty `visible_entities` returns nothing rather than everything:
    /// the caller could establish no scope, and guessing wider would be the
    /// one direction that cannot be undone by the reader.
    pub async fn refused(&self, visible_entities: &[i64]) -> Result<Vec<Refused>> {
        if visible_entities.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to GLPI's database")?;

        // Entity ids are i64 from GLPI's own API, never user input, and are
        // formatted rather than bound because IN-lists cannot be a single
        // placeholder. Everything else is a literal.
        let ids = visible_entities
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "SELECT id, name, itemtype, serial, uuid, ip, mac, method, \
                    rules_id, agents_id, entities_id, \
                    DATE_FORMAT(date_creation, '%Y-%m-%dT%H:%i:%s') \
             FROM glpi_refusedequipments \
             WHERE entities_id IN ({ids}) \
             ORDER BY date_creation DESC \
             LIMIT {ROW_LIMIT}"
        );

        let rows: Vec<Row> = conn
            .query(sql)
            .await
            .context("querying glpi_refusedequipments")?;

        Ok(rows
            .into_iter()
            .map(|r| Refused {
                id: r.0,
                name: r.1,
                itemtype: r.2,
                serial: r.3,
                uuid: r.4,
                ip: r.5,
                mac: r.6,
                method: r.7,
                rules_id: r.8,
                agents_id: r.9,
                entities_id: r.10,
                date_creation: r.11,
            })
            .collect())
    }
}

/// What the server this app talks to actually is.
///
/// Every value here is read from the live instance rather than from GLPI's
/// source, which is the point: the rest of the DB page describes what GLPI
/// *expects* of a server, and this is the one place that says what is
/// actually running. The two disagreeing is a finding.
#[derive(Debug, serde::Serialize)]
pub struct ServerFacts {
    /// `@@version` and `@@version_comment`, as the server reports them.
    pub version: String,
    pub flavour: String,
    /// Seconds since start, from SHOW GLOBAL STATUS.
    pub uptime_seconds: u64,
    /// Settings worth seeing, as (name, value). Chosen rather than dumped:
    /// a full SHOW VARIABLES is 600 rows and answers nothing.
    pub variables: Vec<(String, String)>,
    /// Storage engines the server has compiled in, with the default marked.
    pub engines: Vec<(String, String)>,
    /// GLPI's own database: how many tables, and what it occupies on disk.
    pub glpi_tables: u64,
    pub glpi_bytes: u64,
    /// Tables in GLPI's database whose engine is not the default. Empty is
    /// the expected answer and the interesting case is a non-empty one.
    pub glpi_other_engines: Vec<(String, String)>,
}

impl Db {
    /// Facts about the running server.
    ///
    /// Read as six small queries rather than one join: they come from three
    /// different places (system variables, global status, information_schema)
    /// and a failure in one should not cost the others.
    pub async fn server_facts(&self) -> Result<ServerFacts> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        let (version, flavour): (String, String) = "SELECT @@version, @@version_comment"
            .first(&mut conn)
            .await
            .context("reading @@version")?
            .unwrap_or_else(|| ("unknown".into(), String::new()));

        let uptime: Option<(String, String)> = "SHOW GLOBAL STATUS LIKE 'Uptime'"
            .first(&mut conn)
            .await
            .context("reading Uptime")?;
        let uptime_seconds = uptime.and_then(|(_, v)| v.parse().ok()).unwrap_or_default();

        // Named one by one, so the list is a decision rather than whatever the
        // server happened to expose. These are the ones that change what GLPI
        // does or how its data is stored.
        const WANTED: [&str; 10] = [
            "character_set_server",
            "collation_server",
            "innodb_default_row_format",
            "innodb_file_per_table",
            "innodb_buffer_pool_size",
            "max_connections",
            "max_allowed_packet",
            "sql_mode",
            "lower_case_table_names",
            "log_bin",
        ];
        let mut variables = Vec::new();
        for name in WANTED {
            let row: Option<(String, String)> = format!("SHOW GLOBAL VARIABLES LIKE '{name}'")
                .first(&mut conn)
                .await
                .with_context(|| format!("reading {name}"))?;
            if let Some((n, v)) = row {
                variables.push((n, v));
            }
        }

        let engines: Vec<(String, String)> =
            "SELECT ENGINE, SUPPORT FROM information_schema.ENGINES \
             WHERE SUPPORT IN ('YES', 'DEFAULT') ORDER BY ENGINE"
                .fetch(&mut conn)
                .await
                .context("reading engines")?;

        let (glpi_tables, glpi_bytes): (u64, u64) =
            "SELECT COUNT(*), COALESCE(SUM(data_length + index_length), 0) \
             FROM information_schema.tables WHERE table_schema = 'glpi'"
                .first(&mut conn)
                .await
                .context("measuring the glpi database")?
                .unwrap_or((0, 0));

        // The expected answer is none. A table on a different engine than the
        // rest is the kind of thing nothing else in this app would notice.
        let glpi_other_engines: Vec<(String, String)> =
            "SELECT table_name, engine FROM information_schema.tables \
             WHERE table_schema = 'glpi' AND engine IS NOT NULL AND engine <> 'InnoDB' \
             ORDER BY table_name"
                .fetch(&mut conn)
                .await
                .context("checking engines in use")?;

        Ok(ServerFacts {
            version,
            flavour,
            uptime_seconds,
            variables,
            engines,
            glpi_tables,
            glpi_bytes,
            glpi_other_engines,
        })
    }
}
