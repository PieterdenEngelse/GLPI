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
