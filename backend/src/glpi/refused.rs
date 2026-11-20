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
    /// Settings worth seeing. Chosen rather than dumped: a full SHOW
    /// VARIABLES is 600 rows and answers nothing.
    pub variables: Vec<ServerSetting>,
    /// Storage engines the server has compiled in, with the default marked.
    pub engines: Vec<(String, String)>,
    /// GLPI's own database: how many tables, and what it occupies on disk.
    pub glpi_tables: u64,
    pub glpi_bytes: u64,
    /// Tables in GLPI's database whose engine is not the default. Empty is
    /// the expected answer and the interesting case is a non-empty one.
    pub glpi_other_engines: Vec<(String, String)>,
}

/// One of the ten settings the server panel names, with where its value
/// came from.
///
/// `value` alone cannot answer the question the panel is for - whether this
/// server has been tuned away from what GLPI assumes - because the common
/// case is a value that equals the default and tells you nothing about
/// whether anyone chose it. `origin` does answer it, in the server's own
/// vocabulary, and catches the case no comparison can: a SET GLOBAL back to
/// the default, which leaves the value identical and the origin SQL.
#[derive(Debug, Clone, Serialize)]
pub struct ServerSetting {
    pub name: String,
    pub value: String,
    /// None where the server has no default to compare against - log_bin is
    /// one, and a naive value-vs-default test reports it as a divergence
    /// when there is simply nothing on the other side.
    pub default: Option<String>,
    pub origin: String,
    pub read_only: bool,
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
        // server happened to expose. These are the ones that could change what
        // GLPI does or how its data is stored.
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
        // One query where this was ten, and a richer answer: SHOW GLOBAL
        // VARIABLES gives the value and nothing else, while SYSTEM_VARIABLES
        // carries the default, whether SQL may change it, and where the
        // running value came from. The IN list is built from WANTED above -
        // compile-time strings, never anything from a request.
        let list = WANTED
            .iter()
            .map(|n| format!("'{}'", n.to_uppercase()))
            .collect::<Vec<_>>()
            .join(", ");
        let rows: Vec<(String, String, Option<String>, String, bool)> = format!(
            "SELECT LOWER(VARIABLE_NAME), COALESCE(GLOBAL_VALUE, ''), DEFAULT_VALUE, \
             COALESCE(GLOBAL_VALUE_ORIGIN, ''), READ_ONLY = 'YES' \
             FROM information_schema.SYSTEM_VARIABLES WHERE VARIABLE_NAME IN ({list})"
        )
        .fetch(&mut conn)
        .await
        .context("reading the server's settings")?;

        // Back into WANTED's order, which is a reading order rather than an
        // alphabetical one, so the list stays the decision it was written as.
        let mut variables: Vec<ServerSetting> = Vec::with_capacity(rows.len());
        for name in WANTED {
            if let Some((n, value, default, origin, read_only)) =
                rows.iter().find(|(n, ..)| n == name).cloned()
            {
                variables.push(ServerSetting {
                    name: n,
                    value,
                    default,
                    origin,
                    read_only,
                });
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

/// The groups the InnoDB panel shows, in the order it shows them.
///
/// MariaDB publishes no taxonomy of its own - `SHOW VARIABLES LIKE 'innodb%'`
/// is 137 rows in alphabetical order and nothing more - so this grouping is
/// this app's reading, not the server's. Named here rather than in the UI
/// because the rules below decide it and the two belong together.
pub const INNODB_GROUPS: [&str; 7] = [
    "Memory & Buffer Pool",
    "I/O & Flushing",
    "Transactions & Locks",
    "Statistics & Optimizer",
    "Compression & Encryption",
    "Tablespace & Row Format",
    "Others",
];

/// Needle -> group, matched against the name with `innodb_` stripped, first
/// match winning. The order IS the decision, and three entries exist only
/// because of it:
///
/// - `flush_log_at_trx_commit` carries "trx" and is a durability knob, so it
///   is pinned to I/O ahead of the "trx" rule.
/// - `old_blocks_pct` and `old_blocks_time` contain the substring "lock".
///   They are buffer-pool LRU settings and nothing to do with locking, so
///   they are pinned ahead of the "lock" rule - the one accident in this
///   list that produced a wrong answer rather than an arguable one.
/// - the full-text family is pinned to Others ahead of "optimize_", so
///   `optimize_fulltext_only` sits with the other `ft_*` settings instead of
///   alone under the optimizer.
///
/// Anything unmatched is Others, which is also where a variable added by a
/// future MariaDB lands: a new name shows up in the panel uncategorised
/// rather than being silently dropped.
const INNODB_RULES: [(&str, &str); 53] = [
    ("flush_log_at_trx_commit", "I/O & Flushing"),
    ("old_blocks", "Memory & Buffer Pool"),
    ("ft_", "Others"),
    ("fulltext", "Others"),
    ("undo_directory", "Tablespace & Row Format"),
    ("undo_tablespaces", "Tablespace & Row Format"),
    ("compress", "Compression & Encryption"),
    ("encrypt", "Compression & Encryption"),
    ("scrub", "Compression & Encryption"),
    ("cmp_per_index", "Compression & Encryption"),
    ("lock", "Transactions & Locks"),
    ("trx", "Transactions & Locks"),
    ("purge", "Transactions & Locks"),
    ("undo", "Transactions & Locks"),
    ("snapshot_isolation", "Transactions & Locks"),
    ("rollback_on_timeout", "Transactions & Locks"),
    ("stats_", "Statistics & Optimizer"),
    ("optimize_", "Statistics & Optimizer"),
    ("force_primary_key", "Statistics & Optimizer"),
    (
        "prefix_index_cluster_optimization",
        "Statistics & Optimizer",
    ),
    ("flush", "I/O & Flushing"),
    ("io_capacity", "I/O & Flushing"),
    ("io_threads", "I/O & Flushing"),
    ("doublewrite", "I/O & Flushing"),
    ("aio", "I/O & Flushing"),
    ("atomic_writes", "I/O & Flushing"),
    ("read_ahead", "I/O & Flushing"),
    ("lru_", "I/O & Flushing"),
    ("max_dirty_pages", "I/O & Flushing"),
    ("log_file_", "I/O & Flushing"),
    ("log_write_ahead", "I/O & Flushing"),
    ("log_checkpoint_now", "I/O & Flushing"),
    ("data_file_buffering", "I/O & Flushing"),
    ("data_file_write_through", "I/O & Flushing"),
    ("open_files", "I/O & Flushing"),
    ("fast_shutdown", "I/O & Flushing"),
    ("sort_file_cache", "I/O & Flushing"),
    ("buffer_pool", "Memory & Buffer Pool"),
    ("buf_dump", "Memory & Buffer Pool"),
    ("adaptive_hash", "Memory & Buffer Pool"),
    ("sort_buffer_size", "Memory & Buffer Pool"),
    ("log_buffer_size", "Memory & Buffer Pool"),
    ("data_file_path", "Tablespace & Row Format"),
    ("data_home_dir", "Tablespace & Row Format"),
    ("log_group_home_dir", "Tablespace & Row Format"),
    ("tmpdir", "Tablespace & Row Format"),
    ("default_row_format", "Tablespace & Row Format"),
    ("file_per_table", "Tablespace & Row Format"),
    ("page_size", "Tablespace & Row Format"),
    ("autoextend_increment", "Tablespace & Row Format"),
    ("fill_factor", "Tablespace & Row Format"),
    ("tablespace", "Tablespace & Row Format"),
    ("instant_alter_column", "Tablespace & Row Format"),
];

/// Which group a variable belongs to. Pure, so the grouping is testable
/// without a server.
pub fn innodb_group(name: &str) -> &'static str {
    let stem = name.strip_prefix("innodb_").unwrap_or(name);
    INNODB_RULES
        .iter()
        .find(|(needle, _)| stem.contains(needle))
        .map(|(_, group)| *group)
        .unwrap_or("Others")
}

/// The four tiers every system variable falls into, cut by what changing one
/// costs rather than by what it is about.
///
/// `live` is the server's own answer: `READ_ONLY = 'NO'` and `SET GLOBAL` is
/// accepted. The other three are a reading of the 196 it refuses, and two of
/// them have to be written down by hand because no query produces them.
///
/// Why they cannot be derived, both routes having been tried against 11.4.12:
///
/// - `information_schema.SYSTEM_VARIABLES.COMMAND_LINE_ARGUMENT` is NULL for
///   `character_set_server`, which `-C, --character-set-server=name` sets
///   perfectly well. False negatives, so it cannot be trusted either way.
/// - The `Variables (--variable-name=value)` table in `mariadbd --help
///   --verbose` is a dump of current values, not a list of settable ones: it
///   contains `system-time-zone`, `protocol-version` and `server-uid`, none
///   of which can be set.
///
/// What defeats a name join even in principle is that MariaDB's option names
/// and its variable names are separate namespaces: `--memlock` sets
/// `locked_in_memory`, and `--external-locking` sets `skip_external_locking`.
/// Both were in an earlier draft of REPORTS for exactly that reason, and both
/// are ordinary restart settings.
const INSTALL: [&str; 3] = [
    "innodb_page_size",
    "innodb_data_file_path",
    "innodb_undo_tablespaces",
];

/// Read-only variables with no edit behind them at any price - they report
/// the build, the host, or the last statement.
///
/// Measured on 11.4.12 by taking the 196 the server calls read-only and
/// removing every one with a startup option, then checking the remainder by
/// hand against `mariadbd --help --verbose`. The `have_` family is a prefix
/// rule rather than ten names so a capability a later MariaDB adds lands
/// here instead of in `restart`.
///
/// Over-matching is safe in one direction only, and it is the safe one: this
/// is consulted for read-only variables alone, so a name that became
/// settable would be `live` before any rule here could see it.
const REPORTS: [&str; 16] = [
    "aria_used_for_temp_tables",
    "character_set_system",
    "error_count",
    "external_user",
    "gtid_binlog_pos",
    "gtid_current_pos",
    "hostname",
    "in_transaction",
    "large_page_size",
    "last_gtid",
    "license",
    "log_bin_basename",
    "proxy_user",
    "relay_log_basename",
    "warning_count",
    "wsrep_sr_store",
];

/// Which tier a variable is in. Pure, so the cut is testable without a
/// server.
pub fn variable_tier(name: &str, read_only: bool) -> &'static str {
    if !read_only {
        return "live";
    }
    if INSTALL.contains(&name) {
        return "install";
    }
    if name.starts_with("have_") || REPORTS.contains(&name) {
        return "reports";
    }
    "restart"
}

/// One system variable as the tier pages read it.
///
/// Four fields and no help text: this is the whole of the server's variable
/// space, about 700 rows, and the InnoDB panel already carries descriptions
/// for the 137 that have a page willing to explain them.
#[derive(Debug, Clone, Serialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    /// The server's own word for where the running value came from -
    /// COMPILE-TIME, CONFIG, COMMAND-LINE, SQL, AUTO. `SQL` is the one that
    /// means the running server and the next restart disagree.
    pub origin: String,
    pub read_only: bool,
    pub tier: &'static str,
    /// What the variable is FOR, as against `tier`, which is what changing
    /// it costs. An opinion, and `vargroup` says so at length.
    pub group: &'static str,
    /// Whether SET GLOBAL can touch this at all.
    ///
    /// NOT the same as `!read_only`, and the difference is 13 rows on this
    /// server: a SESSION ONLY variable reports READ_ONLY = NO and still has
    /// no global value to set. The tier says live for those, because the tier
    /// is cut on READ_ONLY alone, so a page that offered a control wherever
    /// the tier said live would offer 13 that cannot work.
    pub settable: bool,
    /// The server's own word for the type - BOOLEAN, ENUM, BIGINT UNSIGNED,
    /// VARCHAR - which is what decides the control a row gets.
    pub kind: String,
    /// Every value an enum or boolean accepts, from the server. Empty for
    /// everything else.
    pub options: Vec<String>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub default: Option<String>,
    /// Why this one needs confirming before it is set, if it does. See
    /// `glpi::hazard` - valid values the server accepts and that break
    /// something the server cannot see.
    pub hazard: Option<&'static str>,
}

/// One row of the variables SELECT. Named for the reason `SettingRow` is:
/// the column order is the contract between the query and the mapping.
type VariableRow = (
    String,         // name, lowercased
    String,         // global value
    String,         // where the running value came from
    bool,           // read_only
    String,         // variable type
    String,         // enum values, comma separated
    Option<String>, // numeric min
    Option<String>, // numeric max
    Option<String>, // default value
    String,         // GLOBAL, SESSION, or SESSION ONLY
);

/// One InnoDB setting: what it is called, what it is set to, and what the
/// server's own binary says it does.
///
/// `help` is None only where the server documents nothing - two of this
/// server's 137 - and the panel distinguishes that from a setting it simply
/// has no button for. See glpi::innodb_help for where the text comes from.
#[derive(Debug, Clone, Serialize)]
pub struct InnodbSetting {
    pub name: String,
    pub value: String,
    pub help: Option<&'static str>,
    /// True when the server refuses a runtime change outright - 31 of this
    /// server's 137. The panel shows those as text rather than as a control
    /// that cannot work.
    pub read_only: bool,
    /// The server's own word for the type: BOOLEAN, ENUM, BIGINT UNSIGNED,
    /// INT UNSIGNED, DOUBLE, VARCHAR. Passed through rather than mapped onto
    /// a taxonomy of this app's, so a type a later MariaDB adds still
    /// arrives intact and the panel can fall back on a text box.
    pub options: Vec<String>,
    pub kind: String,
    pub min: Option<String>,
    pub max: Option<String>,
    /// What the server rounds a numeric value to. 1 MiB for the buffer pool,
    /// so 100000000 is accepted and stored as 100663296 - which is why a
    /// write reads the value back instead of echoing what was sent.
    pub block: Option<String>,
    pub default: Option<String>,
    /// Where the RUNNING value came from, in the server's own words:
    /// COMPILE-TIME (nobody set it), CONFIG (a line in my.cnf, which on this
    /// deployment means /etc/nixos and a rebuild), COMMAND-LINE (the unit's
    /// ExecStart), SQL (someone ran SET GLOBAL - so it reverts at the next
    /// restart), AUTO (the server sized it).
    ///
    /// Worth more than comparing the value against `default`, and this is
    /// the case that proves it: a SET GLOBAL that put a setting back to its
    /// default leaves the value identical and the origin SQL. By eye the row
    /// is untouched; the origin is the only thing that says the running
    /// server no longer matches what a restart would give you.
    pub origin: String,
    /// What the startup file asks for, when it names this setting. None
    /// means a restart returns it to my.cnf and the compiled default - and
    /// a value here that differs from `value` is the mismatch the panel
    /// exists to show.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persisted: Option<String>,
}

/// One group of InnoDB settings, as the panel renders it.
#[derive(Debug, Clone, Serialize)]
pub struct InnodbGroup {
    pub category: String,
    pub variables: Vec<InnodbSetting>,
}

impl Db {
    /// Every InnoDB setting the server has, grouped.
    ///
    /// All of them, unlike the ten in `server_facts`. The ten are a decision
    /// about what changes GLPI's behaviour; this is the whole surface of the
    /// engine GLPI's 447 tables are on, which is a reference rather than a
    /// finding - so it is a separate request, made only when the panel is on
    /// screen.
    ///
    /// Empty groups are kept. A category with nothing in it is a fact about
    /// this server - no encryption settings compiled in, say - and dropping
    /// it would make the panel's shape depend on the server in a way the
    /// reader cannot see.
    pub async fn innodb_variables(&self) -> Result<Vec<InnodbGroup>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;
        let rows = innodb_settings(&mut conn).await?;

        Ok(INNODB_GROUPS
            .iter()
            .map(|group| InnodbGroup {
                category: (*group).to_string(),
                variables: rows
                    .iter()
                    .filter(|v| innodb_group(&v.name) == *group)
                    .cloned()
                    .collect(),
            })
            .collect())
    }

    /// Every system variable the server has, each tagged with its tier.
    ///
    /// One query and no filter, which is the point: the panels above this
    /// one read ten names and `innodb%`, so neither could say how much of
    /// the server they were leaving out. About 700 rows on 11.4.12.
    ///
    /// SESSION ONLY variables have no GLOBAL_VALUE and arrive empty rather
    /// than being dropped - `warning_count` having no global value is a fact
    /// about it, and the tier it lands in is the one that says so.
    pub async fn variables(&self) -> Result<Vec<Variable>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        // The same columns the InnoDB panel reads, for every row rather than
        // for innodb_ only: what each row may be set to is the server's to
        // say, and asking it once here is what lets the page offer a control
        // that cannot propose an invalid value.
        let rows: Vec<VariableRow> = "SELECT LOWER(VARIABLE_NAME), COALESCE(GLOBAL_VALUE, ''), \
             COALESCE(GLOBAL_VALUE_ORIGIN, ''), READ_ONLY = 'YES', \
             VARIABLE_TYPE, COALESCE(ENUM_VALUE_LIST, ''), NUMERIC_MIN_VALUE, \
             NUMERIC_MAX_VALUE, DEFAULT_VALUE, VARIABLE_SCOPE \
             FROM information_schema.SYSTEM_VARIABLES ORDER BY VARIABLE_NAME"
            .fetch(&mut conn)
            .await
            .context("reading the server's variables")?;

        Ok(rows
            .into_iter()
            .map(
                |(name, value, origin, read_only, kind, options, min, max, default, scope)| {
                    Variable {
                        tier: variable_tier(&name, read_only),
                        group: crate::glpi::vargroup::variable_group(&name),
                        settable: !read_only && !scope.eq_ignore_ascii_case("SESSION ONLY"),
                        hazard: crate::glpi::hazard::hazard(&name),
                        kind,
                        options: options
                            .split(',')
                            .map(str::trim)
                            .filter(|o| !o.is_empty())
                            .map(str::to_string)
                            .collect(),
                        min,
                        max,
                        default,
                        name,
                        value,
                        origin,
                        read_only,
                    }
                },
            )
            .collect())
    }

    /// Set one InnoDB setting on the running server, and report what it
    /// became.
    ///
    /// Four things this refuses to take from the caller, because the caller
    /// is a web page and this ends in SET GLOBAL:
    ///
    /// - **The name.** It must match a row the server itself reports under
    ///   `innodb%`, and that row's own spelling is what goes into the
    ///   statement. A name the server does not report never reaches SQL.
    /// - **Whether it may be written at all.** `read_only` is re-read here
    ///   rather than trusted from whatever the page was rendered with, which
    ///   may be minutes stale or forged outright.
    /// - **The shape of the value.** A boolean or enum must match one of the
    ///   server's own ENUM_VALUE_LIST entries, and that entry - not the
    ///   submitted text - is what gets quoted. A number must parse and sit
    ///   inside the server's own range.
    /// - **What it became.** Read back from the server afterwards instead of
    ///   echoed: values are rounded to NUMERIC_BLOCK_SIZE, case is
    ///   normalised, and a buffer pool resize is asynchronous, so what was
    ///   asked for and what is now set are routinely different.
    ///
    /// Not persisted, and the panel says so. MariaDB has no SET PERSIST -
    /// that is MySQL 8 - so a restart hands the value back to my.cnf.
    pub async fn set_innodb_setting(&self, name: &str, value: &str) -> Result<String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        let (canonical, literal) = self.innodb_literal(name, value).await?;
        let var_name = canonical;
        // The name is the server's own spelling of a variable it reported a
        // line ago, and the literal was built here out of a parsed number or
        // a matched enum entry. Neither is the submitted text.
        conn.query_drop(format!("SET GLOBAL {var_name} = {literal}"))
            .await
            .with_context(|| format!("setting {var_name}"))?;

        let after: Option<String> =
            "SELECT COALESCE(GLOBAL_VALUE, '') FROM information_schema.SYSTEM_VARIABLES \
             WHERE VARIABLE_NAME = :name"
                .with(params! { "name" => var_name.to_uppercase() })
                .first(&mut conn)
                .await
                .with_context(|| format!("reading {var_name} back"))?;

        Ok(after.unwrap_or_default())
    }

    /// Everything the Perf page reads from the database in one go.
    ///
    /// Four queries, deliberately: the page is a reading of a live server
    /// and a reader will reload it, so what it costs matters. Returned as a
    /// tuple rather than a struct because every piece has a different shape
    /// and the caller assembles the report anyway.
    ///
    /// THESE ARE BEHAVIOUR, NOT CONFIGURATION. The DB page already says what
    /// the engine is set to; these say what came of it - how often a page was
    /// found in memory, how often a sort spilled to disk, what GLPI's own
    /// background work costs, and where the bytes are. A setting and its
    /// consequence belong on different pages, because the consequence is the
    /// only one of the two that can be wrong.
    #[allow(clippy::type_complexity)]
    pub async fn perf_readings(
        &self,
    ) -> Result<(
        Vec<super::perf::Gauge>,
        Vec<super::perf::CronTask>,
        Vec<super::perf::TableSize>,
        i64,
        i64,
    )> {
        use super::perf::{CronTask, Gauge, TableSize};

        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        // Status counters are cumulative since the server started, so every
        // rate below is an average over that period and says so. An
        // instantaneous rate would need two samples a second apart, which is
        // a second this page is not worth.
        let status: Vec<(String, String)> =
            "SELECT VARIABLE_NAME, VARIABLE_VALUE FROM information_schema.GLOBAL_STATUS"
                .fetch(&mut conn)
                .await
                .context("reading server status")?;
        let get = |name: &str| -> f64 {
            status
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0.0)
        };
        let max_connections: Option<(String, String)> =
            "SHOW GLOBAL VARIABLES LIKE 'max_connections'"
                .first(&mut conn)
                .await
                .context("reading max_connections")?;
        let max_connections = max_connections
            .and_then(|(_, v)| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        let reads = get("Innodb_buffer_pool_reads");
        let requests = get("Innodb_buffer_pool_read_requests");
        let tmp = get("Created_tmp_tables");
        let tmp_disk = get("Created_tmp_disk_tables");
        let uptime = get("Uptime").max(1.0);
        let pages_total = get("Innodb_buffer_pool_pages_total").max(1.0);

        let engine = vec![
            Gauge {
                label: "Pages served from memory".to_string(),
                value: format!("{:.2}%", 100.0 * (1.0 - reads / requests.max(1.0))),
                detail: format!(
                    "{requests:.0} page requests, {reads:.0} of which had to come from disk. \
                     The whole point of the buffer pool, and the first number to move when \
                     the data stops fitting in it"
                ),
            },
            Gauge {
                label: "Buffer pool in use".to_string(),
                value: format!(
                    "{:.0}% dirty",
                    100.0 * get("Innodb_buffer_pool_pages_dirty") / pages_total
                ),
                detail: format!(
                    "{:.0} of {pages_total:.0} pages hold changes not yet written to disk. \
                     High is not a fault - it is work batched rather than work pending",
                    get("Innodb_buffer_pool_pages_dirty")
                ),
            },
            Gauge {
                label: "Sorts that spilled to disk".to_string(),
                value: format!("{:.1}%", 100.0 * tmp_disk / tmp.max(1.0)),
                detail: format!(
                    "{tmp_disk:.0} of {tmp:.0} temporary tables were written to disk rather \
                     than held in memory. Decided by tmp_table_size and max_heap_table_size, \
                     and by queries that ask for more than either"
                ),
            },
            Gauge {
                label: "Queries answered".to_string(),
                value: format!("{:.2}/s", get("Questions") / uptime),
                detail: format!(
                    "{:.0} statements over {:.1} days of uptime - an average, not a rate \
                     now",
                    get("Questions"),
                    uptime / 86_400.0
                ),
            },
            Gauge {
                label: "Slow queries".to_string(),
                value: format!("{:.0}", get("Slow_queries")),
                detail: "Statements that took longer than long_query_time. Counted whether \
                         or not the slow log is being written"
                    .to_string(),
            },
            Gauge {
                label: "Connections in use".to_string(),
                value: format!("{:.0} of {max_connections:.0}", get("Threads_connected")),
                detail: format!(
                    "Against max_connections. GLPI opens one per PHP worker, so the ceiling \
                     that binds first is usually php-fpm's, not this one. {:.0} connections \
                     have been aborted since startup",
                    get("Aborted_connects")
                ),
            },
            Gauge {
                label: "Row lock waits".to_string(),
                value: format!("{:.0}", get("Innodb_row_lock_waits")),
                detail: "Times a statement waited for another transaction's lock. The \
                         number that rises when agents report concurrently"
                    .to_string(),
            },
        ];

        // GLPI's own account of its background work. LEFT JOIN with a filter
        // on elapsed so a task that has never run still appears - "scheduled
        // and never run" is a finding, and an INNER JOIN would hide it.
        let cron: Vec<(
            String,
            i64,
            i64,
            Option<String>,
            Option<f64>,
            Option<f64>,
            i64,
        )> = "SELECT c.name, c.frequency, c.state, CAST(c.lastrun AS CHAR), \
                    AVG(l.elapsed), MAX(l.elapsed), COUNT(l.id) \
             FROM glpi_crontasks c \
             LEFT JOIN glpi_crontasklogs l ON l.crontasks_id = c.id AND l.elapsed > 0 \
             GROUP BY c.id, c.name, c.frequency, c.state, c.lastrun \
             ORDER BY AVG(l.elapsed) DESC, c.name LIMIT 12"
            .fetch(&mut conn)
            .await
            .context("reading GLPI's scheduled tasks")?;
        let cron = cron
            .into_iter()
            .map(
                |(name, frequency, state, last_run, avg, max, runs)| CronTask {
                    name,
                    frequency,
                    state,
                    last_run,
                    avg_seconds: avg.unwrap_or(0.0),
                    max_seconds: max.unwrap_or(0.0),
                    runs,
                },
            )
            .collect();

        let tables: Vec<(String, i64, i64)> = "SELECT table_name, COALESCE(table_rows, 0), \
                    CAST(ROUND((data_length + index_length) / 1024) AS SIGNED) \
             FROM information_schema.tables WHERE table_schema = 'glpi' \
             ORDER BY (data_length + index_length) DESC LIMIT 10"
            .fetch(&mut conn)
            .await
            .context("measuring GLPI's tables")?;
        let tables = tables
            .into_iter()
            .map(|(name, rows, kib)| TableSize { name, rows, kib })
            .collect();

        let (total_kib, total_tables): (i64, i64) =
            "SELECT CAST(ROUND(COALESCE(SUM(data_length + index_length), 0) / 1024) AS SIGNED), \
                    COUNT(*) \
             FROM information_schema.tables WHERE table_schema = 'glpi'"
                .first(&mut conn)
                .await
                .context("measuring GLPI's database")?
                .unwrap_or((0, 0));

        Ok((engine, cron, tables, total_kib, total_tables))
    }

    /// Time a query of the shape GLPI's own pages run.
    ///
    /// A COUNT over the largest table GLPI's inventory fills, which is what
    /// a list view does before it draws a page of rows.
    ///
    /// WHAT IS IN THE NUMBER, said plainly because an earlier version of
    /// this tried to exclude it and quietly failed: the round trip from this
    /// process over the unix socket, including the driver. There is no
    /// server-side stopwatch in MariaDB a client can read back per
    /// statement: no last_query_cost, and the performance schema answers
    /// aggregates rather than "that one". So the honest choice is to measure
    /// what can be measured and label it. The socket and driver are worth
    /// tenths of a millisecond; where this figure matters is when it stops
    /// being tenths, which this measures just as well.
    ///
    /// Run `samples` times and reported as best and median: the first run
    /// fills the buffer pool and is not what the next thousand will cost.
    pub async fn time_query(&self, samples: usize) -> Result<super::perf::Timing> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        let mut times = Vec::with_capacity(samples);
        for _ in 0..samples {
            let started = std::time::Instant::now();
            let _: Option<i64> = "SELECT COUNT(*) FROM glpi_items_softwareversions"
                .first(&mut conn)
                .await
                .context("timing a GLPI-shaped query")?;
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }

        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Ok(super::perf::Timing {
            what: "COUNT over glpi_items_softwareversions".to_string(),
            detail: "The largest table GLPI's inventory fills - what a list view counts \
                     before it draws a page. Round trip over the unix socket, driver \
                     included"
                .to_string(),
            best_ms: times.first().copied().unwrap_or_default(),
            median_ms: times[times.len() / 2],
            worst_ms: times.last().copied().unwrap_or_default(),
            error: None,
        })
    }

    /// -> the server's own spelling of this setting, and the SQL literal for
    /// this value, or why there is neither.
    ///
    /// The whole of the check that stands between a web page and SET GLOBAL,
    /// in one place because two callers need exactly the same answer: the
    /// live write, and the startup file that will replay it unattended after
    /// the next restart. A value good enough to apply now is the only kind
    /// worth writing into a file nobody will be watching when it runs.
    pub async fn innodb_literal(&self, name: &str, value: &str) -> Result<(String, String)> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;
        let known = innodb_settings(&mut conn).await?;
        let var = known
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("{name} is not an InnoDB setting on this server"))?;
        if var.read_only {
            anyhow::bail!(
                "{} is read-only: the server takes it at startup, before any SQL runs, so \
                 neither this panel nor the startup file can set it - only my.cnf can",
                var.name
            );
        }
        Ok((var.name.clone(), literal_for(var, value)?))
    }

    /// Set ANY system variable on the running server, and report what it
    /// became.
    ///
    /// The generalisation of `set_innodb_setting`, and almost the same
    /// function, because the validation underneath was never InnoDB-specific:
    /// `literal_for` builds its literal out of VARIABLE_TYPE, ENUM_VALUE_LIST
    /// and the numeric bounds that SYSTEM_VARIABLES publishes for every row
    /// alike. The only thing that made the InnoDB write InnoDB-shaped was the
    /// WHERE clause feeding it.
    ///
    /// SET GLOBAL only, so this changes the running server and not the next
    /// one - the same limit the InnoDB panel has, and why `origin` reads SQL
    /// afterwards. Persisting is a separate mechanism and still InnoDB-only.
    ///
    /// Hazards are deliberately NOT checked here. What they need is a second,
    /// informed visit from the caller, which is an HTTP concern; a `confirmed`
    /// bool threaded down to here would put that policy in the layer that
    /// knows least about it. See `glpi::hazard`.
    pub async fn set_variable(&self, name: &str, value: &str) -> Result<String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to MariaDB")?;

        let (var, scope) = variable_meta(&mut conn, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("{name} is not a system variable on this server"))?;

        if var.read_only {
            anyhow::bail!(
                "{} is read-only: the server reads it at startup, before any SQL runs, so \
                 nothing sent to a running server can change it - only a config file and a \
                 restart",
                var.name
            );
        }
        // 13 of this server's 497 dynamic variables, and a category error
        // rather than a permission one: identity, timestamp, the rand_seed
        // pair and the rest exist per connection and have no global value to
        // set. The server's own refusal explains neither half.
        if scope.eq_ignore_ascii_case("SESSION ONLY") {
            anyhow::bail!(
                "{} exists per connection and has no global value, so there is nothing \
                 server-wide to set",
                var.name
            );
        }

        let literal = literal_for(&var, value)?;
        let var_name = var.name;
        // The same rule the InnoDB write states: the name is the server's own
        // spelling of a row it reported a line ago, and the literal was built
        // from a parsed number or a matched enum entry. Neither is the text
        // that arrived.
        conn.query_drop(format!("SET GLOBAL {var_name} = {literal}"))
            .await
            .with_context(|| format!("setting {var_name}"))?;

        let after: Option<String> =
            "SELECT COALESCE(GLOBAL_VALUE, '') FROM information_schema.SYSTEM_VARIABLES \
             WHERE VARIABLE_NAME = :name"
                .with(params! { "name" => var_name.to_uppercase() })
                .first(&mut conn)
                .await
                .with_context(|| format!("reading {var_name} back"))?;

        Ok(after.unwrap_or_default())
    }
}

/// Everything the server will say about ONE variable, whatever subject it
/// belongs to, in the shape `literal_for` already reads.
///
/// `help` comes back None even for an innodb_ row that has text: this feeds
/// the write, which renders nothing. The scope rides beside the struct rather
/// than on it because only the write has any use for it, and this table is
/// the only place that knows.
async fn variable_meta(
    conn: &mut mysql_async::Conn,
    name: &str,
) -> Result<Option<(InnodbSetting, String)>> {
    let row: Option<ScopedSettingRow> =
        "SELECT LOWER(VARIABLE_NAME), COALESCE(GLOBAL_VALUE, ''), READ_ONLY = 'YES', \
          VARIABLE_TYPE, COALESCE(ENUM_VALUE_LIST, ''), NUMERIC_MIN_VALUE, \
          NUMERIC_MAX_VALUE, NUMERIC_BLOCK_SIZE, DEFAULT_VALUE, \
          COALESCE(GLOBAL_VALUE_ORIGIN, ''), VARIABLE_SCOPE \
          FROM information_schema.SYSTEM_VARIABLES WHERE VARIABLE_NAME = :name"
            .with(params! { "name" => name.to_uppercase() })
            .first(&mut *conn)
            .await
            .with_context(|| format!("reading {name}"))?;

    Ok(row.map(
        |(name, value, read_only, kind, options, min, max, block, default, origin, scope)| {
            (
                InnodbSetting {
                    name,
                    value,
                    help: None,
                    read_only,
                    kind,
                    options: options
                        .split(',')
                        .map(str::trim)
                        .filter(|o| !o.is_empty())
                        .map(str::to_string)
                        .collect(),
                    min,
                    max,
                    block,
                    default,
                    origin,
                    persisted: None,
                },
                scope,
            )
        },
    ))
}

/// `SettingRow` plus VARIABLE_SCOPE, for the one query that needs it.
type ScopedSettingRow = (
    String,         // name, lowercased
    String,         // global value
    bool,           // read_only
    String,         // variable type
    String,         // enum values, comma separated
    Option<String>, // numeric min
    Option<String>, // numeric max
    Option<String>, // numeric block size
    Option<String>, // default value
    String,         // where the running value came from
    String,         // GLOBAL, SESSION, or SESSION ONLY
);

/// Every InnoDB setting with everything the server will say about it.
///
/// One query behind both the panel and the write, so the two can never
/// disagree about what exists or about what may be set. SYSTEM_VARIABLES
/// rather than SHOW GLOBAL VARIABLES, which answers the value and nothing
/// else: the type decides which control a row gets, the enum list and the
/// range decide what that control may offer, and READ_ONLY decides whether
/// there is one at all. A table of that written here would be this app's
/// claim about a server it does not ship - the same mistake as a quoted
/// count, and it would rot at the next upgrade with nothing to notice.
///
/// Names come back uppercase from this table and lowercase from SHOW
/// VARIABLES. Lowercase is what the rest of this app, the help text and
/// MariaDB's own documentation use, so they are folded here.
/// One SYSTEM_VARIABLES row as the driver hands it back.
///
/// Named for the same reason `Row` above it is: the column order is the
/// contract between the SELECT and the mapping, and a ten-field tuple inline
/// hides that. Clippy asks for this too, which is the lesser reason.
type SettingRow = (
    String,         // name, lowercased
    String,         // global value
    bool,           // read_only
    String,         // variable type
    String,         // enum values, comma separated
    Option<String>, // numeric min
    Option<String>, // numeric max
    Option<String>, // numeric block size
    Option<String>, // default value
    String,         // where the running value came from
);

async fn innodb_settings(conn: &mut mysql_async::Conn) -> Result<Vec<InnodbSetting>> {
    let rows: Vec<SettingRow> =
        "SELECT LOWER(VARIABLE_NAME), COALESCE(GLOBAL_VALUE, ''), READ_ONLY = 'YES', \
          VARIABLE_TYPE, COALESCE(ENUM_VALUE_LIST, ''), NUMERIC_MIN_VALUE, \
          NUMERIC_MAX_VALUE, NUMERIC_BLOCK_SIZE, DEFAULT_VALUE, \
          COALESCE(GLOBAL_VALUE_ORIGIN, '') \
          FROM information_schema.SYSTEM_VARIABLES \
          WHERE VARIABLE_NAME LIKE 'INNODB%' ORDER BY VARIABLE_NAME"
            .fetch(&mut *conn)
            .await
            .context("reading InnoDB settings")?;

    Ok(rows
        .into_iter()
        .map(
            |(name, value, read_only, kind, options, min, max, block, default, origin)| {
                let help = crate::glpi::innodb_help::innodb_help(&name);
                InnodbSetting {
                    name,
                    value,
                    help,
                    read_only,
                    kind,
                    options: options
                        .split(',')
                        .map(str::trim)
                        .filter(|o| !o.is_empty())
                        .map(str::to_string)
                        .collect(),
                    min,
                    max,
                    block,
                    default,
                    origin,
                    // Filled in by the handler, which is where the file is.
                    persisted: None,
                }
            },
        )
        .collect())
}

/// -> the SQL literal for this value, or why it is not one.
///
/// Apart from the write so that the quoting rules can be tested without a
/// server, which is where they are actually checked.
fn literal_for(var: &InnodbSetting, value: &str) -> Result<String> {
    let value = value.trim();

    // Booleans arrive as an enum list of their own - OFF,ON - so one branch
    // covers both, and neither can be set to anything the server did not
    // name itself.
    if !var.options.is_empty() {
        let matched = var
            .options
            .iter()
            .find(|o| o.eq_ignore_ascii_case(value))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{} takes one of {}, not {value:?}",
                    var.name,
                    var.options.join(", ")
                )
            })?;
        return Ok(format!("'{matched}'"));
    }

    match var.kind.as_str() {
        "DOUBLE" => {
            let n: f64 = value
                .parse()
                .with_context(|| format!("{} takes a number, not {value:?}", var.name))?;
            check_range(var, n)?;
            Ok(n.to_string())
        }
        k if k.contains("INT") => {
            let n: i64 = value
                .parse()
                .with_context(|| format!("{} takes a whole number, not {value:?}", var.name))?;
            check_range(var, n as f64)?;
            Ok(n.to_string())
        }
        // VARCHAR, and whatever a later MariaDB adds that is neither an enum
        // nor a number. Quoted and escaped rather than refused: these are
        // file and table names, and refusing them would make the panel lie
        // about which of its rows are writable.
        _ => Ok(format!(
            "'{}'",
            value.replace('\\', "\\\\").replace('\'', "\\'")
        )),
    }
}

/// The server enforces its own range regardless - this is so the refusal
/// names the bound, rather than arriving as a MySQL error number.
fn check_range(var: &InnodbSetting, n: f64) -> Result<()> {
    if let Some(min) = var.min.as_ref().and_then(|m| m.parse::<f64>().ok()) {
        if n < min {
            anyhow::bail!("{} does not go below {min}", var.name);
        }
    }
    if let Some(max) = var.max.as_ref().and_then(|m| m.parse::<f64>().ok()) {
        if n > max {
            anyhow::bail!("{} does not go above {max}", var.name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod innodb_write_tests {
    use super::*;

    fn var(kind: &str, options: &[&str], min: Option<&str>, max: Option<&str>) -> InnodbSetting {
        InnodbSetting {
            name: "innodb_test".into(),
            value: String::new(),
            help: None,
            read_only: false,
            kind: kind.into(),
            options: options.iter().map(|o| (*o).to_string()).collect(),
            min: min.map(str::to_string),
            max: max.map(str::to_string),
            block: None,
            default: None,
            // Irrelevant to validation, which is all this fixture feeds.
            origin: "COMPILE-TIME".into(),
            persisted: None,
        }
    }

    /// The submitted text never reaches the statement: what is quoted is the
    /// server's own spelling of the option that matched.
    #[test]
    fn an_enum_is_quoted_as_the_server_spells_it() {
        let v = var("ENUM", &["redundant", "compact", "dynamic"], None, None);
        assert_eq!(literal_for(&v, "DYNAMIC").unwrap(), "'dynamic'");
        assert_eq!(literal_for(&v, " compact ").unwrap(), "'compact'");
    }

    #[test]
    fn an_enum_refuses_anything_not_on_the_list() {
        let v = var("ENUM", &["redundant", "compact", "dynamic"], None, None);
        let e = literal_for(&v, "dynamic'; DROP DATABASE glpi; --").unwrap_err();
        assert!(e.to_string().contains("takes one of"), "{e}");
    }

    /// A boolean is an enum of OFF,ON as far as the server is concerned, so
    /// it goes down the same path and gets the same refusal.
    #[test]
    fn a_boolean_takes_only_off_or_on() {
        let v = var("BOOLEAN", &["OFF", "ON"], None, None);
        assert_eq!(literal_for(&v, "on").unwrap(), "'ON'");
        assert!(literal_for(&v, "1; SELECT 1").is_err());
    }

    #[test]
    fn a_number_must_parse_and_is_rendered_from_the_parse() {
        let v = var(
            "BIGINT UNSIGNED",
            &[],
            Some("2097152"),
            Some("1099511627776"),
        );
        assert_eq!(literal_for(&v, " 134217728 ").unwrap(), "134217728");
        assert!(literal_for(&v, "134217728 OR 1=1").is_err());
    }

    /// The server would refuse these too - the point of checking here is
    /// that the refusal names the bound instead of arriving as an error
    /// number.
    #[test]
    fn a_number_outside_the_servers_range_is_refused_by_name() {
        let v = var(
            "BIGINT UNSIGNED",
            &[],
            Some("2097152"),
            Some("1099511627776"),
        );
        assert!(literal_for(&v, "1024")
            .unwrap_err()
            .to_string()
            .contains("does not go below"));
        assert!(literal_for(&v, "1099511627777")
            .unwrap_err()
            .to_string()
            .contains("does not go above"));
    }

    /// The only values that arrive as free text. Escaped, because a file or
    /// table name is a legitimate thing to set and refusing it would make
    /// the panel lie about which rows are writable.
    #[test]
    fn free_text_is_escaped_rather_than_refused() {
        let v = var("VARCHAR", &[], None, None);
        assert_eq!(literal_for(&v, "O'Brien").unwrap(), r"'O\'Brien'");
        assert_eq!(literal_for(&v, r"a\b").unwrap(), r"'a\\b'");
    }
}

#[cfg(test)]
mod tier_tests {
    use super::*;

    /// read_only is the gate, so a dynamic variable can never be filed as
    /// anything but live - including one whose name matches a rule below.
    #[test]
    fn dynamic_is_always_live() {
        assert_eq!(variable_tier("innodb_buffer_pool_size", false), "live");
        assert_eq!(variable_tier("have_ssl", false), "live");
        assert_eq!(variable_tier("innodb_page_size", false), "live");
    }

    /// The three that validate and then leave the server unable to start
    /// against its datadir. scripts/db-restart.sh refuses the same three by
    /// name; if these lists drift apart the page and the script disagree.
    #[test]
    fn the_install_three_are_not_restart() {
        for name in INSTALL {
            assert_eq!(variable_tier(name, true), "install", "{name}");
        }
    }

    /// have_ is a prefix so a capability a later MariaDB adds lands in
    /// reports rather than being offered as something a restart could change.
    #[test]
    fn capabilities_are_reports_by_prefix() {
        assert_eq!(variable_tier("have_ssl", true), "reports");
        assert_eq!(variable_tier("have_something_new", true), "reports");
    }

    /// The four that were in REPORTS until their startup options were found.
    /// MariaDB's option names are a separate namespace from its variable
    /// names, which is what made them look unsettable: --memlock sets
    /// locked_in_memory and --external-locking sets skip_external_locking.
    #[test]
    fn variables_with_differently_named_options_are_restart() {
        for name in [
            "locked_in_memory",
            "skip_external_locking",
            "binlog_do_db",
            "binlog_ignore_db",
        ] {
            assert_eq!(variable_tier(name, true), "restart", "{name}");
        }
    }

    /// Session results have no global value and no setter. The tier exists
    /// so they are not listed as things a restart would change.
    #[test]
    fn session_results_are_reports() {
        assert_eq!(variable_tier("warning_count", true), "reports");
        assert_eq!(variable_tier("in_transaction", true), "reports");
    }

    /// An unrecognised read-only name is restart, not reports: a config edit
    /// is the ordinary case, and guessing "no edit exists" for a variable
    /// nobody has classified would hide the one route that probably works.
    #[test]
    fn unknown_read_only_falls_to_restart() {
        assert_eq!(variable_tier("some_future_setting", true), "restart");
    }
}

#[cfg(test)]
mod innodb_tests {
    use super::*;

    /// Every rule's target has to be a group the panel actually renders, or
    /// its variables vanish - the grouping drops anything whose name is not
    /// in INNODB_GROUPS.
    #[test]
    fn every_rule_names_a_real_group() {
        for (needle, group) in INNODB_RULES {
            assert!(
                INNODB_GROUPS.contains(&group),
                "{needle} -> {group}, which is not a group"
            );
        }
    }

    /// The three orderings that exist because a substring lied. Written as
    /// tests because reordering the list is the easy way to break them and
    /// the symptom is a setting in the wrong tab, which nothing else notices.
    #[test]
    fn substring_accidents_stay_fixed() {
        assert_eq!(
            innodb_group("innodb_old_blocks_pct"),
            "Memory & Buffer Pool"
        );
        assert_eq!(
            innodb_group("innodb_old_blocks_time"),
            "Memory & Buffer Pool"
        );
        assert_eq!(
            innodb_group("innodb_flush_log_at_trx_commit"),
            "I/O & Flushing"
        );
        assert_eq!(innodb_group("innodb_optimize_fulltext_only"), "Others");
    }

    /// undo_* splits across two groups on purpose: where the undo log lives
    /// is a tablespace question, how big it grows is a transaction one.
    #[test]
    fn undo_splits_by_what_it_is_about() {
        assert_eq!(
            innodb_group("innodb_undo_directory"),
            "Tablespace & Row Format"
        );
        assert_eq!(
            innodb_group("innodb_undo_tablespaces"),
            "Tablespace & Row Format"
        );
        assert_eq!(
            innodb_group("innodb_max_undo_log_size"),
            "Transactions & Locks"
        );
        assert_eq!(
            innodb_group("innodb_undo_log_truncate"),
            "Transactions & Locks"
        );
    }

    #[test]
    fn a_setting_a_future_mariadb_adds_is_not_dropped() {
        assert_eq!(innodb_group("innodb_something_new_in_12_0"), "Others");
    }

    /// A name arriving without the prefix still classifies, because the
    /// rules are written against the stem.
    #[test]
    fn the_prefix_is_optional() {
        assert_eq!(innodb_group("buffer_pool_size"), "Memory & Buffer Pool");
        assert_eq!(
            innodb_group("innodb_buffer_pool_size"),
            "Memory & Buffer Pool"
        );
    }
}
