//! What each InnoDB setting is, in the server's own words.
//!
//! GENERATED, not written - by scripts/innodb-help.py, from
//! `mariadbd --help --verbose` on MariaDB 11.4.12. Every description below is
//! the server's own, reflowed and nothing else: no paraphrase, no manual, no
//! text of this app's. Same reason the engine page read
//! `information_schema.ENGINES.COMMENT` rather than quoting documentation - a
//! sentence written here would be this app's claim about a server it does not
//! ship, and would rot at the next upgrade with nothing to notice.
//!
//! To regenerate after a MariaDB upgrade:
//!
//! ```sh
//! mariadbd --help --verbose | scripts/innodb-help.py > backend/src/glpi/innodb_help.rs
//! ```
//!
//! THE COUNT DOES NOT MATCH THE PANEL'S, on purpose. This server reports 137
//! `innodb_%` system variables; the binary documents 165 `--innodb-*`
//! options. 135 names are in both. The other 30 here are information_schema
//! plugins - `innodb_trx`, `innodb_locks`, `innodb_cmp` and friends - which
//! are loadable plugins rather than settings, so nothing ever looks them up.
//! They are kept because filtering them would mean this file is no longer
//! simply what the command printed, and that property is the whole point.
//!
//! The two variables going the other way are the server's doing:
//! `innodb_lru_flush_size` and `innodb_purge_rseg_truncate_frequency` are
//! reported as variables but have no option help at all, so there is nothing
//! to quote. `innodb_help` answers None for them and the panel says as much
//! rather than inventing a sentence.

/// Variable name -> the server's own description. Sorted by name, which is
/// also the order `SHOW VARIABLES` returns them in.
pub const INNODB_HELP: [(&str, &str); 165] = [
    (
        "innodb_adaptive_flushing",
        "Attempt flushing dirty pages to avoid IO bursts at checkpoints. (Defaults to \
             on; use --skip-innodb-adaptive-flushing to disable.)",
    ),
    (
        "innodb_adaptive_flushing_lwm",
        "Percentage of log capacity below which no adaptive flushing happens.",
    ),
    (
        "innodb_adaptive_hash_index",
        "Enable InnoDB adaptive hash index (disabled by default).",
    ),
    (
        "innodb_adaptive_hash_index_parts",
        "Number of InnoDB Adaptive Hash Index Partitions (default 8)",
    ),
    (
        "innodb_alter_copy_bulk",
        "Allow bulk insert operation for copy alter operation (Defaults to on; use \
             --skip-innodb-alter-copy-bulk to disable.)",
    ),
    (
        "innodb_autoextend_increment",
        "Data file autoextend increment in megabytes",
    ),
    (
        "innodb_autoinc_lock_mode",
        "The AUTOINC lock modes supported by InnoDB: 0 => Old style AUTOINC locking \
             (for backward compatibility); 1 => New style AUTOINC locking; 2 => No \
             AUTOINC locking (unsafe for SBR)",
    ),
    (
        "innodb_buf_dump_status_frequency",
        "A number between [0, 100] that tells how oftern buffer pool dump status in \
             percentages should be printed. E.g. 10 means that buffer pool dump status is \
             printed when every 10% of number of buffer pool pages are dumped. Default is \
             0 (only start and end status is printed).",
    ),
    (
        "innodb_buffer_page",
        "Enable or disable INNODB_BUFFER_PAGE plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_buffer_page_lru",
        "Enable or disable INNODB_BUFFER_PAGE_LRU plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_buffer_pool_chunk_size",
        "Deprecated parameter with no effect",
    ),
    (
        "innodb_buffer_pool_dump_at_shutdown",
        "Dump the buffer pool into a file named @@innodb_buffer_pool_filename \
             (Defaults to on; use --skip-innodb-buffer-pool-dump-at-shutdown to disable.)",
    ),
    (
        "innodb_buffer_pool_dump_now",
        "Trigger an immediate dump of the buffer pool into a file named \
             @@innodb_buffer_pool_filename",
    ),
    (
        "innodb_buffer_pool_dump_pct",
        "Dump only the hottest N% of each buffer pool, defaults to 25",
    ),
    (
        "innodb_buffer_pool_filename",
        "Filename to/from which to dump/load the InnoDB buffer pool",
    ),
    (
        "innodb_buffer_pool_in_core_dump",
        "Include the buffer pool in core dump.",
    ),
    (
        "innodb_buffer_pool_load_abort",
        "Abort a currently running load of the buffer pool",
    ),
    (
        "innodb_buffer_pool_load_at_startup",
        "Load the buffer pool from a file named @@innodb_buffer_pool_filename \
             (Defaults to on; use --skip-innodb-buffer-pool-load-at-startup to disable.)",
    ),
    (
        "innodb_buffer_pool_load_now",
        "Trigger an immediate load of the buffer pool from a file named \
             @@innodb_buffer_pool_filename",
    ),
    (
        "innodb_buffer_pool_size",
        "The size of the memory buffer InnoDB uses to cache data and indexes of its \
             tables.",
    ),
    (
        "innodb_buffer_pool_size_auto_min",
        "Minimum innodb_buffer_pool_size for dynamic shrinking on memory pressure",
    ),
    (
        "innodb_buffer_pool_size_max",
        "Maximum innodb_buffer_pool_size",
    ),
    (
        "innodb_buffer_pool_stats",
        "Enable or disable INNODB_BUFFER_POOL_STATS plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_checksum_algorithm",
        "The algorithm InnoDB uses for page checksumming. Possible values are \
             FULL_CRC32 for new files, always use CRC-32C; for old, see CRC32 below; \
             STRICT_FULL_CRC32 for new files, always use CRC-32C; for old, see \
             STRICT_CRC32 below; CRC32 write crc32, allow previously used algorithms to \
             match when reading; STRICT_CRC32 write crc32, do not allow other algorithms \
             to match when reading; New files created with full_crc32 are readable by \
             MariaDB 10.4.3+",
    ),
    (
        "innodb_cmp",
        "Enable or disable INNODB_CMP plugin. One of: ON, OFF, FORCE (don't start if \
             the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but the plugin \
             can not be uninstalled).",
    ),
    (
        "innodb_cmp_per_index",
        "Enable or disable INNODB_CMP_PER_INDEX plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_cmp_per_index_enabled",
        "Enable INFORMATION_SCHEMA.innodb_cmp_per_index, may have negative impact on \
             performance (off by default)",
    ),
    (
        "innodb_cmp_per_index_reset",
        "Enable or disable INNODB_CMP_PER_INDEX_RESET plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_cmp_reset",
        "Enable or disable INNODB_CMP_RESET plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_cmpmem",
        "Enable or disable INNODB_CMPMEM plugin. One of: ON, OFF, FORCE (don't start \
             if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but the \
             plugin can not be uninstalled).",
    ),
    (
        "innodb_cmpmem_reset",
        "Enable or disable INNODB_CMPMEM_RESET plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_compression_algorithm",
        "Compression algorithm used on page compression. One of: none, zlib, lz4, \
             lzo, lzma, bzip2, or snappy",
    ),
    (
        "innodb_compression_default",
        "Is compression the default for new tables",
    ),
    (
        "innodb_compression_failure_threshold_pct",
        "If the compression failure rate of a table is greater than this number more \
             padding is added to the pages to reduce the failures. A value of zero \
             implies no padding",
    ),
    (
        "innodb_compression_level",
        "Compression level used for zlib compression. 0 is no compression, 1 is \
             fastest, 9 is best compression and default is 6.",
    ),
    (
        "innodb_compression_pad_pct_max",
        "Percentage of empty space on a data page that can be reserved to make the \
             page compressible.",
    ),
    (
        "innodb_data_file_buffering",
        "Whether the file system cache for data files is enabled",
    ),
    (
        "innodb_data_file_path",
        "Path to individual files and their sizes.",
    ),
    (
        "innodb_data_file_write_through",
        "Whether each write to data files writes through",
    ),
    (
        "innodb_data_home_dir",
        "The common part for InnoDB table spaces.",
    ),
    (
        "innodb_deadlock_detect",
        "Enable/disable InnoDB deadlock detector (default ON). if set to OFF, \
             deadlock detection is skipped, and we rely on innodb_lock_wait_timeout in \
             case of deadlock. (Defaults to on; use --skip-innodb-deadlock-detect to \
             disable.)",
    ),
    (
        "innodb_deadlock_report",
        "How to report deadlocks (if innodb_deadlock_detect=ON).. One of: off, basic, \
             full",
    ),
    (
        "innodb_default_encryption_key_id",
        "Default encryption key id used for table encryption.",
    ),
    (
        "innodb_default_row_format",
        "The default ROW FORMAT for all innodb tables created without explicit \
             ROW_FORMAT. Possible values are REDUNDANT, COMPACT, and DYNAMIC. The \
             ROW_FORMAT value COMPRESSED is not allowed. One of: redundant, compact, \
             dynamic",
    ),
    (
        "innodb_disable_sort_file_cache",
        "Whether to disable OS system file cache for sort I/O",
    ),
    (
        "innodb_doublewrite",
        "Whether and how to use the doublewrite buffer. OFF=Assume that writes of \
             innodb_page_size are atomic; ON=Prevent torn writes (the default); fast=Like \
             ON, but do not synchronize writes to data files",
    ),
    ("innodb_encrypt_log", "Enable redo log encryption"),
    (
        "innodb_encrypt_tables",
        "Enable encryption for tables. Don't forget to enable --innodb-encrypt-log \
             too. One of: OFF, ON, FORCE",
    ),
    (
        "innodb_encrypt_temporary_tables",
        "Enrypt the temporary table data.",
    ),
    (
        "innodb_encryption_rotate_key_age",
        "Key rotation - re-encrypt in background all pages that were encrypted with a \
             key that many (or more) versions behind. Value 0 indicates that key rotation \
             is disabled.",
    ),
    (
        "innodb_encryption_rotation_iops",
        "Use this many iops for background key rotation",
    ),
    (
        "innodb_encryption_threads",
        "Number of threads performing background key rotation",
    ),
    (
        "innodb_fast_shutdown",
        "Speeds up the shutdown process of the InnoDB storage engine. Possible values \
             are 0, 1 (faster), 2 (crash-like), 3 (fastest clean).",
    ),
    (
        "innodb_fatal_semaphore_wait_threshold",
        "Maximum number of seconds that semaphore times out in InnoDB.",
    ),
    (
        "innodb_file_per_table",
        "Stores each InnoDB table to an .ibd file in the database dir. (Defaults to \
             on; use --skip-innodb-file-per-table to disable.)",
    ),
    (
        "innodb_fill_factor",
        "Percentage of B-tree page filled during bulk insert",
    ),
    (
        "innodb_flush_log_at_timeout",
        "Write and flush logs every (n) second.",
    ),
    (
        "innodb_flush_log_at_trx_commit",
        "Controls the durability/speed trade-off for commits. Set to 0 (write and \
             flush redo log to disk only once per second), 1 (flush to disk at each \
             commit), 2 (write to log at commit but flush to disk only once per second) \
             or 3 (flush to disk at prepare and at commit, slower and usually redundant). \
             1 and 3 guarantees that after a crash, committed transactions will not be \
             lost and will be consistent with the binlog and other transactional engines. \
             2 can get inconsistent and lose transactions if there is a power failure or \
             kernel crash but not if mysqld crashes. 0 has no guarantees in case of \
             crash. 0 and 2 can be faster than 1 or 3.",
    ),
    (
        "innodb_flush_method",
        "With which method to flush data.. One of: fsync, O_DSYNC, littlesync, \
             nosync, O_DIRECT, O_DIRECT_NO_FSYNC",
    ),
    (
        "innodb_flush_neighbors",
        "Set to 0 (don't flush neighbors from buffer pool), 1 (flush contiguous \
             neighbors from buffer pool) or 2 (flush neighbors from buffer pool), when \
             flushing a block",
    ),
    (
        "innodb_flush_sync",
        "Allow IO bursts at the checkpoints ignoring io_capacity setting. (Defaults \
             to on; use --skip-innodb-flush-sync to disable.)",
    ),
    (
        "innodb_flushing_avg_loops",
        "Number of iterations over which the background flushing is averaged.",
    ),
    (
        "innodb_force_primary_key",
        "Do not allow creating a table without primary key (off by default)",
    ),
    (
        "innodb_force_recovery",
        "Helps to save your data in case the disk image of the database becomes \
             corrupt. Value 5 can return bogus data, and 6 can permanently corrupt data.",
    ),
    (
        "innodb_ft_aux_table",
        "FTS internal auxiliary table to be checked",
    ),
    (
        "innodb_ft_being_deleted",
        "Enable or disable INNODB_FT_BEING_DELETED plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_cache_size",
        "InnoDB Fulltext search cache size in bytes",
    ),
    (
        "innodb_ft_config",
        "Enable or disable INNODB_FT_CONFIG plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_default_stopword",
        "Enable or disable INNODB_FT_DEFAULT_STOPWORD plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_deleted",
        "Enable or disable INNODB_FT_DELETED plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_enable_diag_print",
        "Whether to enable additional FTS diagnostic printout",
    ),
    (
        "innodb_ft_enable_stopword",
        "Create FTS index with stopword. (Defaults to on; use --skip-innodb-ft- \
             enable-stopword to disable.)",
    ),
    (
        "innodb_ft_index_cache",
        "Enable or disable INNODB_FT_INDEX_CACHE plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_index_table",
        "Enable or disable INNODB_FT_INDEX_TABLE plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_ft_max_token_size",
        "InnoDB Fulltext search maximum token size in characters",
    ),
    (
        "innodb_ft_min_token_size",
        "InnoDB Fulltext search minimum token size in characters",
    ),
    (
        "innodb_ft_num_word_optimize",
        "InnoDB Fulltext search number of words to optimize for each optimize table call",
    ),
    (
        "innodb_ft_result_cache_limit",
        "InnoDB Fulltext search query result cache limit in bytes",
    ),
    (
        "innodb_ft_server_stopword_table",
        "The user supplied stopword table name.",
    ),
    (
        "innodb_ft_sort_pll_degree",
        "InnoDB Fulltext search parallel sort degree, will round up to nearest power \
             of 2 number",
    ),
    (
        "innodb_ft_total_cache_size",
        "Total memory allocated for InnoDB Fulltext Search cache",
    ),
    (
        "innodb_ft_user_stopword_table",
        "User supplied stopword table name, effective in the session level.",
    ),
    (
        "innodb_immediate_scrub_data_uncompressed",
        "Enable scrubbing of data",
    ),
    (
        "innodb_instant_alter_column_allowed",
        "File format constraint for ALTER TABLE. One of: never, add_last, \
             add_drop_reorder",
    ),
    (
        "innodb_io_capacity",
        "Number of IOPs the server can do. Tunes the background IO rate",
    ),
    (
        "innodb_io_capacity_max",
        "Limit to which innodb_io_capacity can be inflated.",
    ),
    (
        "innodb_linux_aio",
        "Specifies which Linux AIO implementation should be used. Possible value are \
             \"auto\" (default) to select io_uring and fallback to aio, or explicit \
             \"io_uring\" or \"aio\"",
    ),
    (
        "innodb_lock_wait_timeout",
        "Timeout in seconds an InnoDB transaction may wait for a lock before being \
             rolled back. The value 100000000 is infinite timeout.",
    ),
    (
        "innodb_lock_waits",
        "Enable or disable INNODB_LOCK_WAITS plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_locks",
        "Enable or disable INNODB_LOCKS plugin. One of: ON, OFF, FORCE (don't start \
             if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but the \
             plugin can not be uninstalled).",
    ),
    ("innodb_log_buffer_size", "Redo log buffer size in bytes."),
    (
        "innodb_log_checkpoint_now",
        "Write back dirty pages from the buffer pool and update the log checkpoint",
    ),
    (
        "innodb_log_file_buffering",
        "Whether the file system cache for ib_logfile0 is enabled",
    ),
    (
        "innodb_log_file_mmap",
        "Whether ib_logfile0 resides in persistent memory (when supported) or should \
             initially be memory-mapped (Defaults to on; use --skip-innodb-log-file-mmap \
             to disable.)",
    ),
    ("innodb_log_file_size", "Redo log size in bytes."),
    (
        "innodb_log_file_write_through",
        "Whether each write to ib_logfile0 is write through",
    ),
    ("innodb_log_group_home_dir", "Path to ib_logfile0"),
    (
        "innodb_log_spin_wait_delay",
        "Deprecated parameter with no effect",
    ),
    (
        "innodb_log_write_ahead_size",
        "Redo log write size to avoid read-on-write; must be a power of two",
    ),
    (
        "innodb_lru_scan_depth",
        "How deep to scan LRU to keep it clean",
    ),
    (
        "innodb_max_dirty_pages_pct",
        "Percentage of dirty pages allowed in bufferpool.",
    ),
    (
        "innodb_max_dirty_pages_pct_lwm",
        "Percentage of dirty pages at which flushing kicks in. The value 0 (default) \
             means 'refer to innodb_max_dirty_pages_pct'.",
    ),
    (
        "innodb_max_purge_lag",
        "Desired maximum length of the purge queue (0 = no limit)",
    ),
    (
        "innodb_max_purge_lag_delay",
        "Maximum delay of user threads in micro-seconds",
    ),
    (
        "innodb_max_purge_lag_wait",
        "Wait until History list length is below the specified limit",
    ),
    (
        "innodb_max_undo_log_size",
        "Desired maximum UNDO tablespace size in bytes",
    ),
    (
        "innodb_metrics",
        "Enable or disable INNODB_METRICS plugin. One of: ON, OFF, FORCE (don't start \
             if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but the \
             plugin can not be uninstalled).",
    ),
    ("innodb_monitor_disable", "Turn off a monitor counter"),
    ("innodb_monitor_enable", "Turn on a monitor counter"),
    ("innodb_monitor_reset", "Reset a monitor counter"),
    (
        "innodb_monitor_reset_all",
        "Reset all values for a monitor counter",
    ),
    (
        "innodb_old_blocks_pct",
        "Percentage of the buffer pool to reserve for 'old' blocks.",
    ),
    (
        "innodb_old_blocks_time",
        "Move blocks to the 'new' end of the buffer pool if the first access was at \
             least this many milliseconds ago. The timeout is disabled if 0.",
    ),
    (
        "innodb_online_alter_log_max_size",
        "Maximum modification log file size for online index creation",
    ),
    (
        "innodb_open_files",
        "How many files at the maximum InnoDB keeps open at the same time.",
    ),
    (
        "innodb_optimize_fulltext_only",
        "Only optimize the Fulltext index of the table",
    ),
    (
        "innodb_page_size",
        "Page size to use for all InnoDB tablespaces.",
    ),
    (
        "innodb_prefix_index_cluster_optimization",
        "Deprecated parameter with no effect (Defaults to on; use --skip-innodb- \
             prefix-index-cluster-optimization to disable.)",
    ),
    (
        "innodb_print_all_deadlocks",
        "Print all deadlocks to MariaDB error log (off by default)",
    ),
    (
        "innodb_purge_batch_size",
        "Number of UNDO log pages to purge in one batch from the history list.",
    ),
    (
        "innodb_purge_threads",
        "Number of tasks for purging transaction history",
    ),
    (
        "innodb_random_read_ahead",
        "Whether to use read ahead for random access within an extent.",
    ),
    (
        "innodb_read_ahead_threshold",
        "Number of pages that must be accessed sequentially for InnoDB to trigger a \
             readahead.",
    ),
    (
        "innodb_read_io_threads",
        "Number of background read I/O threads in InnoDB.",
    ),
    (
        "innodb_read_only",
        "Start InnoDB in read only mode (off by default)",
    ),
    (
        "innodb_read_only_compressed",
        "Make ROW_FORMAT=COMPRESSED tables read-only",
    ),
    (
        "innodb_rollback_on_timeout",
        "Roll back the complete transaction on lock wait timeout, for 4.x \
             compatibility (disabled by default)",
    ),
    (
        "innodb_snapshot_isolation",
        "Use snapshot isolation (write-write conflict detection).",
    ),
    (
        "innodb_sort_buffer_size",
        "Memory buffer size for index creation",
    ),
    (
        "innodb_spin_wait_delay",
        "Maximum delay between polling for a spin lock (4 by default)",
    ),
    (
        "innodb_stats_auto_recalc",
        "InnoDB automatic recalculation of persistent statistics enabled for all \
             tables unless overridden at table level (automatic recalculation is only \
             done when InnoDB decides that the table has changed too much and needs a new \
             statistics) (Defaults to on; use --skip-innodb-stats-auto-recalc to \
             disable.)",
    ),
    (
        "innodb_stats_include_delete_marked",
        "Include delete marked records when calculating persistent statistics",
    ),
    (
        "innodb_stats_method",
        "Specifies how InnoDB index statistics collection code should treat NULLs. \
             Possible values are NULLS_EQUAL (default), NULLS_UNEQUAL and NULLS_IGNORED. \
             One of: nulls_equal, nulls_unequal, nulls_ignored",
    ),
    (
        "innodb_stats_modified_counter",
        "The number of rows modified before we calculate new statistics (default 0 = \
             current limits)",
    ),
    (
        "innodb_stats_on_metadata",
        "Enable statistics gathering for metadata commands such as SHOW TABLE STATUS \
             for tables that use transient statistics (off by default)",
    ),
    (
        "innodb_stats_persistent",
        "InnoDB persistent statistics enabled for all tables unless overridden at \
             table level (Defaults to on; use --skip-innodb-stats-persistent to disable.)",
    ),
    (
        "innodb_stats_persistent_sample_pages",
        "The number of leaf index pages to sample when calculating persistent \
             statistics (by ANALYZE, default 20)",
    ),
    (
        "innodb_stats_traditional",
        "Enable traditional statistic calculation based on number of configured pages \
             (default true) (Defaults to on; use --skip-innodb-stats-traditional to \
             disable.)",
    ),
    (
        "innodb_stats_transient_sample_pages",
        "The number of leaf index pages to sample when calculating transient \
             statistics (if persistent statistics are not used, default 8)",
    ),
    (
        "innodb_status_file",
        "Enable SHOW ENGINE INNODB STATUS output in the innodb_status.<pid> file",
    ),
    (
        "innodb_status_output",
        "Enable InnoDB monitor output to the error log.",
    ),
    (
        "innodb_status_output_locks",
        "Enable InnoDB lock monitor output to the error log. Requires \
             innodb_status_output=ON.",
    ),
    (
        "innodb_strict_mode",
        "Use strict mode when evaluating create options. (Defaults to on; use --skip- \
             innodb-strict-mode to disable.)",
    ),
    (
        "innodb_sync_spin_loops",
        "Count of spin-loop rounds in InnoDB mutexes (30 by default)",
    ),
    (
        "innodb_sys_columns",
        "Enable or disable INNODB_SYS_COLUMNS plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_fields",
        "Enable or disable INNODB_SYS_FIELDS plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_foreign",
        "Enable or disable INNODB_SYS_FOREIGN plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_foreign_cols",
        "Enable or disable INNODB_SYS_FOREIGN_COLS plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_indexes",
        "Enable or disable INNODB_SYS_INDEXES plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_tables",
        "Enable or disable INNODB_SYS_TABLES plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_tablespaces",
        "Enable or disable INNODB_SYS_TABLESPACES plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_tablestats",
        "Enable or disable INNODB_SYS_TABLESTATS plugin. One of: ON, OFF, FORCE \
             (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, \
             but the plugin can not be uninstalled).",
    ),
    (
        "innodb_sys_virtual",
        "Enable or disable INNODB_SYS_VIRTUAL plugin. One of: ON, OFF, FORCE (don't \
             start if the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but \
             the plugin can not be uninstalled).",
    ),
    (
        "innodb_table_locks",
        "Enable InnoDB locking in LOCK TABLES (Defaults to on; use --skip-innodb- \
             table-locks to disable.)",
    ),
    (
        "innodb_tablespaces_encryption",
        "Enable or disable INNODB_TABLESPACES_ENCRYPTION plugin. One of: ON, OFF, \
             FORCE (don't start if the plugin fails to load), FORCE_PLUS_PERMANENT (like \
             FORCE, but the plugin can not be uninstalled).",
    ),
    (
        "innodb_temp_data_file_path",
        "Path to files and their sizes making temp-tablespace.",
    ),
    (
        "innodb_tmpdir",
        "Directory for temporary non-tablespace files.",
    ),
    (
        "innodb_truncate_temporary_tablespace_now",
        "Shrink the temporary tablespace",
    ),
    (
        "innodb_trx",
        "Enable or disable INNODB_TRX plugin. One of: ON, OFF, FORCE (don't start if \
             the plugin fails to load), FORCE_PLUS_PERMANENT (like FORCE, but the plugin \
             can not be uninstalled).",
    ),
    (
        "innodb_undo_directory",
        "Directory where undo tablespace files live, this path can be absolute.",
    ),
    (
        "innodb_undo_log_truncate",
        "Enable or Disable Truncate of UNDO tablespace.",
    ),
    (
        "innodb_undo_tablespaces",
        "Number of undo tablespaces to use.",
    ),
    (
        "innodb_use_atomic_writes",
        "Enable atomic writes, instead of using the doublewrite buffer, for files on \
             devices that supports atomic writes. (Defaults to on; use --skip-innodb-use- \
             atomic-writes to disable.)",
    ),
    (
        "innodb_use_native_aio",
        "Use native AIO if supported on this platform. (Defaults to on; use --skip- \
             innodb-use-native-aio to disable.)",
    ),
    (
        "innodb_write_io_threads",
        "Number of background write I/O threads in InnoDB.",
    ),
];

/// The server's description of a setting, if its own binary printed one.
pub fn innodb_help(name: &str) -> Option<&'static str> {
    INNODB_HELP
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, help)| *help)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A regeneration that goes wrong grows duplicates, and a duplicate here
    /// is invisible: the lookup finds the first and the second never shows.
    #[test]
    fn every_name_appears_once() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in INNODB_HELP {
            assert!(seen.insert(name), "{name} is listed twice");
        }
    }

    /// Sorted, so the diff after a MariaDB upgrade is readable: added and
    /// removed settings stay put instead of the whole file shifting.
    #[test]
    fn the_list_is_sorted() {
        let given: Vec<&str> = INNODB_HELP.iter().map(|(n, _)| *n).collect();
        let mut sorted = given.clone();
        sorted.sort_unstable();
        assert_eq!(given, sorted);
    }

    /// An empty entry would render an info button that opens on a blank
    /// panel, which is worse than having no button.
    #[test]
    fn no_entry_is_empty() {
        for (name, help) in INNODB_HELP {
            assert!(!help.trim().is_empty(), "{name} has no description");
        }
    }

    #[test]
    fn the_two_the_server_does_not_describe_are_absent() {
        assert_eq!(innodb_help("innodb_lru_flush_size"), None);
        assert_eq!(innodb_help("innodb_purge_rseg_truncate_frequency"), None);
        assert!(innodb_help("innodb_buffer_pool_size").is_some());
    }
}
