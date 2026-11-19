//! Which part of the server a variable belongs to.
//!
//! The counterpart to `refused::variable_tier`, which cuts the same 693 by
//! what CHANGING one costs. This cuts them by what one is FOR, because the
//! two questions are asked at different moments: the tier says what it will
//! take to move something, this says whether the thing you are hunting is
//! even on the page.
//!
//! IT IS AN OPINION AND HAS TO SAY SO. MariaDB publishes no functional
//! taxonomy. `information_schema.SYSTEM_VARIABLES.VARIABLE_COMMENT` is
//! populated for 692 of this server's 693 and averages 106 characters, but
//! it describes rather than categorises - `sql_mode` reads "Sets the sql
//! mode" - so nothing here can be derived and every line below is a
//! judgement. Same standing as `INNODB_RULES`, which this deliberately
//! copies: ordered substring rules, first match wins, `Others` as the honest
//! remainder.
//!
//! ORDER IS LOAD-BEARING. The exceptions at the top are pinned ahead of the
//! general rules they would otherwise trip over - `lock_wait_timeout` before
//! "timeout", `old_mode` before the bare "old", `flush_time` before "time".
//! `INNODB_RULES` records the one accident of this kind that produced a
//! wrong rather than an arguable answer, and the same hazard lives here.
//!
//! EVERY innodb_ VARIABLE IS ONE GROUP, not seven. InnoDB has its own panel
//! that already cuts its 137 into seven by the same technique, and repeating
//! that cut here would be two taxonomies of the same thing in two places.
//! On the variables pages InnoDB is a destination, not a breakdown.
//!
//! Measured when written, on mariadb-server-11.4.12: of the live tier's 497,
//! 113 are dormant and 106 are InnoDB, leaving 278 for these rules, which
//! place all but 3. Of restart's 167: 61 dormant, 28 InnoDB, 78 grouped,
//! none left over. `Others` is kept visible anyway - a variable a future
//! MariaDB adds has to show up uncategorised rather than disappear.

/// Every group `variable_group` can return. The list exists so a typo in a
/// rule's target fails a test rather than quietly creating a group of one -
/// `INNODB_GROUPS` earns its keep the same way.
///
/// Some of these are named by a single rule and that is correct: a whole
/// subsystem with one prefix needs exactly one.
pub const VAR_GROUPS: [&str; 21] = [
    "Character sets & collation",
    "Compression",
    "Connections & threads",
    "Files & paths",
    "Full-text",
    "Galera",
    "Locale & time",
    "Logging & diagnostics",
    "Memory & caches",
    "Optimizer & statistics",
    "Other storage engines",
    "Performance schema",
    "Protocol & session",
    "Query cache",
    "Replication",
    "SQL semantics",
    "Security & encryption",
    "Server identity",
    "Timeouts & limits",
    "Transactions & locking",
    "Others",
];

const VAR_RULES: [(&str, &str); 170] = [
    // The subsystems first, and the order matters as much as anywhere here:
    // `binlog_cache_size` contains "log_" and would file under logging, and
    // `wsrep_slave_threads` contains "thread". These are whole subsystems, so
    // they win over every general rule below. They are grouped even though
    // the variables pages fold them away when their switch is off - what a
    // variable is FOR does not depend on whether this server happens to use
    // it, and something other than that page may ask.
    ("wsrep", "Galera"),
    ("galera", "Galera"),
    ("performance_schema", "Performance schema"),
    ("query_cache", "Query cache"),
    ("binlog", "Replication"),
    ("relay_log", "Replication"),
    ("replicate_", "Replication"),
    ("slave_", "Replication"),
    ("master_", "Replication"),
    ("rpl_", "Replication"),
    ("gtid", "Replication"),
    ("sync_master_info", "Replication"),
    ("low_priority_updates", "SQL semantics"),
    ("lock_wait_timeout", "Transactions & locking"),
    ("innodb_lock_wait_timeout", "Transactions & locking"),
    ("old_mode", "SQL semantics"),
    ("old_passwords", "Security & encryption"),
    ("table_open_cache", "Memory & caches"),
    ("flush_time", "Files & paths"),
    ("init_file", "Files & paths"),
    ("init_connect", "Connections & threads"),
    ("init_slave", "Logging & diagnostics"),
    ("character_set", "Character sets & collation"),
    ("character_sets", "Character sets & collation"),
    ("collation", "Character sets & collation"),
    ("lc_", "Locale & time"),
    ("time_zone", "Locale & time"),
    ("system_versioning", "Locale & time"),
    ("explicit_defaults_for_timestamp", "Locale & time"),
    ("ssl", "Security & encryption"),
    ("tls", "Security & encryption"),
    ("secure_", "Security & encryption"),
    ("password", "Security & encryption"),
    ("auth", "Security & encryption"),
    ("encrypt", "Security & encryption"),
    ("require_", "Security & encryption"),
    ("skip_grant", "Security & encryption"),
    ("allow_suspicious", "Security & encryption"),
    ("local_infile", "Security & encryption"),
    ("ft_", "Full-text"),
    ("fulltext", "Full-text"),
    ("optimizer", "Optimizer & statistics"),
    ("histogram", "Optimizer & statistics"),
    ("analyze_", "Optimizer & statistics"),
    ("use_stat", "Optimizer & statistics"),
    ("eq_range", "Optimizer & statistics"),
    ("expensive_subquery", "Optimizer & statistics"),
    ("join_cache", "Optimizer & statistics"),
    ("join_buffer", "Optimizer & statistics"),
    ("mrr", "Optimizer & statistics"),
    ("range_", "Optimizer & statistics"),
    ("max_seeks_for_key", "Optimizer & statistics"),
    ("max_length_for_sort", "Optimizer & statistics"),
    ("in_predicate", "Optimizer & statistics"),
    ("rowid_merge", "Optimizer & statistics"),
    ("statement_analysis", "Optimizer & statistics"),
    ("autocommit", "Transactions & locking"),
    ("deadlock", "Transactions & locking"),
    ("transaction", "Transactions & locking"),
    ("isolation", "Transactions & locking"),
    ("rollback", "Transactions & locking"),
    ("savepoint", "Transactions & locking"),
    ("completion_type", "Transactions & locking"),
    ("lock", "Transactions & locking"),
    ("commit", "Transactions & locking"),
    ("thread", "Connections & threads"),
    ("max_connections", "Connections & threads"),
    ("max_user_connections", "Connections & threads"),
    ("connect", "Connections & threads"),
    ("back_log", "Connections & threads"),
    ("host_cache", "Connections & threads"),
    ("extra_port", "Connections & threads"),
    ("bind_address", "Connections & threads"),
    ("port", "Connections & threads"),
    ("net_", "Protocol & session"),
    ("session_track", "Protocol & session"),
    ("protocol", "Protocol & session"),
    ("socket", "Protocol & session"),
    ("max_allowed_packet", "Protocol & session"),
    ("timeout", "Timeouts & limits"),
    ("max_error_count", "Timeouts & limits"),
    ("max_sp_recursion", "Timeouts & limits"),
    ("max_recursive", "Timeouts & limits"),
    ("max_prepared_stmt", "Timeouts & limits"),
    ("group_concat_max_len", "Timeouts & limits"),
    ("long_query_time", "Logging & diagnostics"),
    ("limit", "Timeouts & limits"),
    ("max_write_lock", "Timeouts & limits"),
    ("max_statement", "Timeouts & limits"),
    ("aria_", "Other storage engines"),
    ("myisam", "Other storage engines"),
    ("spider", "Other storage engines"),
    ("federated", "Other storage engines"),
    ("rocksdb", "Other storage engines"),
    ("heap", "Other storage engines"),
    ("enforce_storage_engine", "Other storage engines"),
    ("default_storage_engine", "Other storage engines"),
    ("default_tmp_storage_engine", "Other storage engines"),
    ("delay_key_write", "Other storage engines"),
    ("concurrent_insert", "Other storage engines"),
    ("general_log", "Logging & diagnostics"),
    ("slow_query", "Logging & diagnostics"),
    ("log_", "Logging & diagnostics"),
    ("_log", "Logging & diagnostics"),
    ("profiling", "Logging & diagnostics"),
    ("progress_report", "Logging & diagnostics"),
    ("debug", "Logging & diagnostics"),
    ("core_file", "Logging & diagnostics"),
    ("stack_trace", "Logging & diagnostics"),
    ("syslog", "Logging & diagnostics"),
    ("column_compression", "Compression"),
    ("compress", "Compression"),
    ("buffer", "Memory & caches"),
    ("cache", "Memory & caches"),
    ("tmp_", "Memory & caches"),
    ("sort_", "Memory & caches"),
    ("_size", "Memory & caches"),
    ("mem", "Memory & caches"),
    ("big_tables", "Memory & caches"),
    ("preload", "Memory & caches"),
    ("datadir", "Files & paths"),
    ("basedir", "Files & paths"),
    ("tmpdir", "Files & paths"),
    ("dir", "Files & paths"),
    ("path", "Files & paths"),
    ("file", "Files & paths"),
    ("open_files", "Files & paths"),
    ("flush", "Files & paths"),
    ("version", "Server identity"),
    ("hostname", "Server identity"),
    ("server_id", "Server identity"),
    ("system_time", "Server identity"),
    ("plugin", "Server identity"),
    ("event_scheduler", "Server identity"),
    ("have_", "Server identity"),
    ("license", "Server identity"),
    ("sql_", "SQL semantics"),
    ("ansi", "SQL semantics"),
    ("old", "SQL semantics"),
    ("strict", "SQL semantics"),
    ("auto_increment", "SQL semantics"),
    ("alter_algorithm", "SQL semantics"),
    ("check_constraint", "SQL semantics"),
    ("div_precision", "SQL semantics"),
    ("unique_checks", "SQL semantics"),
    ("foreign_key", "SQL semantics"),
    ("updatable_views", "SQL semantics"),
    ("default_", "SQL semantics"),
    ("identity", "SQL semantics"),
    ("insert_id", "SQL semantics"),
    ("rand_", "SQL semantics"),
    ("read_only", "SQL semantics"),
    ("skip_", "SQL semantics"),
    ("sp_", "SQL semantics"),
    ("mode", "SQL semantics"),
    ("secure", "SQL semantics"),
    ("keep_files", "SQL semantics"),
    ("query_alloc", "Memory & caches"),
    ("storage_engine", "Other storage engines"),
    ("ignore_builtin_innodb", "Other storage engines"),
    ("slow_launch_time", "Logging & diagnostics"),
    ("note_verbosity", "Logging & diagnostics"),
    ("max_digest_length", "Logging & diagnostics"),
    ("mysql56_temporal_format", "Locale & time"),
    ("standard_compliant_cte", "SQL semantics"),
    ("lower_case_table_names", "SQL semantics"),
    ("large_pages", "Memory & caches"),
    ("server_uid", "Server identity"),
    ("tcp_", "Protocol & session"),
    ("userstat", "Logging & diagnostics"),
];

/// Which functional group a variable belongs to. Pure, so it is testable
/// without a server - the same contract `innodb_group` keeps.
pub fn variable_group(name: &str) -> &'static str {
    if name.starts_with("innodb_") {
        return "InnoDB";
    }
    VAR_RULES
        .iter()
        .find(|(fragment, _)| name.contains(fragment))
        .map(|(_, group)| *group)
        .unwrap_or("Others")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned exceptions earn their place only if they actually win, so
    /// each one is asserted against the general rule it sits ahead of.
    #[test]
    fn pinned_exceptions_beat_the_general_rule() {
        assert_eq!(
            variable_group("lock_wait_timeout"),
            "Transactions & locking"
        );
        assert_eq!(variable_group("flush_time"), "Files & paths");
        assert_eq!(variable_group("old_mode"), "SQL semantics");
        assert_eq!(variable_group("old_passwords"), "Security & encryption");
        assert_eq!(variable_group("table_open_cache"), "Memory & caches");
        assert_eq!(variable_group("init_connect"), "Connections & threads");
    }

    /// InnoDB is one destination here however many groups its own panel has.
    #[test]
    fn innodb_is_a_single_group() {
        assert_eq!(variable_group("innodb_buffer_pool_size"), "InnoDB");
        assert_eq!(variable_group("innodb_log_file_size"), "InnoDB");
        assert_eq!(variable_group("innodb_page_size"), "InnoDB");
    }

    /// An unmatched name lands in Others rather than vanishing, which is how
    /// a variable a future MariaDB adds stays visible.
    #[test]
    fn unmatched_lands_in_others() {
        assert_eq!(
            variable_group("totally_new_thing_from_the_future"),
            "Others"
        );
    }

    /// Spot checks across the groups, so a reordering that silently moves a
    /// family fails here rather than on the page.
    #[test]
    fn representative_names_land_where_expected() {
        assert_eq!(
            variable_group("character_set_server"),
            "Character sets & collation"
        );
        assert_eq!(variable_group("max_connections"), "Connections & threads");
        assert_eq!(variable_group("optimizer_switch"), "Optimizer & statistics");
        assert_eq!(variable_group("slow_query_log"), "Logging & diagnostics");
        assert_eq!(
            variable_group("aria_pagecache_buffer_size"),
            "Other storage engines"
        );
        assert_eq!(variable_group("have_ssl"), "Security & encryption");
    }

    /// Every rule has to target a group that actually exists, or a typo
    /// creates a group of one that looks deliberate. The same guard
    /// `innodb_tests::every_rule_names_a_real_group` keeps.
    #[test]
    fn every_rule_names_a_real_group() {
        for (fragment, group) in VAR_RULES.iter() {
            assert!(
                VAR_GROUPS.contains(group),
                "rule {fragment:?} targets {group:?}, which is not in VAR_GROUPS"
            );
        }
    }

    /// A subsystem wins over every general rule, because its names collide
    /// with them: binlog_cache_size contains "log_" and wsrep_slave_threads
    /// contains "thread".
    #[test]
    fn subsystems_beat_the_general_rules() {
        assert_eq!(variable_group("binlog_cache_size"), "Replication");
        assert_eq!(variable_group("wsrep_slave_threads"), "Galera");
        assert_eq!(
            variable_group("performance_schema_max_mutex_classes"),
            "Performance schema"
        );
        assert_eq!(variable_group("query_cache_size"), "Query cache");
    }
}
