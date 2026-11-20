//! Variables the server will set without complaint and that break something
//! else when it does.
//!
//! Every name here is dynamic, globally settable, and takes a perfectly valid
//! value - so `literal_for` passes it, the server accepts it, and nothing in
//! the write path has any reason to object. The damage is downstream: in
//! GLPI, in this box's memory, or in the operator's ability to connect again
//! afterwards. That is exactly the class a type check cannot catch, which is
//! why it is a list of names rather than a rule.
//!
//! IT IS NOT A DENY LIST. These are settable; the write asks for a second,
//! separate confirmation first and shows the text below. The same shape the
//! restart links use - preview, then commit - and for the same reason: the
//! cost of the action is not visible in the control that performs it.
//!
//! The counterpart is `scripts/db-restart.sh`'s DENIED, which IS a refusal,
//! because those three leave a server that cannot start against its existing
//! datadir and no later restart recovers it. Nothing here is unrecoverable
//! from a running server, and everything here is recoverable only *from* one,
//! which is the distinction that decides which list a name belongs on.
//!
//! Measured on mariadb-server-11.4.12 on this host: all nine are
//! READ_ONLY = NO and none is SESSION ONLY, so all nine are reachable by the
//! write this guards.

/// (name, what setting it breaks). Lowercase, matching the rest of this app
/// and `LOWER(VARIABLE_NAME)`.
const HAZARDS: [(&str, &str); 9] = [
    (
        "max_connections",
        "Set below the number of connections already open and new clients are \
         refused - this app and GLPI included. Nothing can reconnect to raise \
         it again, so the way back is an already-open session or a restart.",
    ),
    (
        "innodb_buffer_pool_size",
        "Raising this commits the memory immediately. This host has 7.5 GB and \
         an OOM killer that takes the largest process, which is the database.",
    ),
    (
        "read_only",
        "ON leaves every read working and fails every write, inventory imports \
         included. GLPI goes on looking healthy while nothing it is sent lands.",
    ),
    (
        "sql_mode",
        "GLPI's inserts are written against this server's mode. Dropping \
         STRICT_TRANS_TABLES makes an over-long or ill-typed value truncate \
         silently where it used to be refused.",
    ),
    (
        "character_set_server",
        "GLPI requires utf8mb4, and this is the default new tables are created \
         with. Changing it splits the install across two encodings rather than \
         converting anything.",
    ),
    (
        "collation_server",
        "The collation half of the same requirement, with the same effect on \
         tables created after the change.",
    ),
    (
        "max_allowed_packet",
        "An inventory submission arrives as one statement. Below its size the \
         import fails at the agent, which exits 0, and no row is written.",
    ),
    (
        "general_log",
        "ON writes every statement to disk, GLPI's whole inventory traffic \
         included. It fills the filesystem, and the server stops when it does.",
    ),
    (
        "wsrep_on",
        "ON without a configured cluster makes the server try to join one and \
         refuse writes until it succeeds. Galera is off on this host.",
    ),
];

/// -> why this variable needs confirming, or None if it does not.
pub fn hazard(name: &str) -> Option<&'static str> {
    HAZARDS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, why)| *why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_variable_is_a_hazard_whatever_its_case() {
        assert!(hazard("max_connections").is_some());
        assert!(hazard("MAX_CONNECTIONS").is_some());
        assert!(hazard("Max_Connections").is_some());
    }

    #[test]
    fn an_ordinary_variable_is_not() {
        for name in [
            "long_query_time",
            "innodb_flush_log_at_trx_commit",
            "tmp_table_size",
        ] {
            assert_eq!(hazard(name), None, "{name}");
        }
    }

    /// A name that only matched as a prefix or substring would silently drag
    /// an unrelated variable into the confirm step - `read_only` against
    /// `innodb_read_only`, which is a different setting and read-only itself.
    #[test]
    fn matching_is_whole_name_not_substring() {
        assert_eq!(hazard("innodb_read_only"), None);
        assert_eq!(hazard("max_connections_per_hour"), None);
        assert_eq!(hazard("general_log_file"), None);
    }

    /// The text is what the operator decides on, so an empty one would make
    /// the confirm step a bare "are you sure".
    #[test]
    fn every_hazard_says_what_it_breaks() {
        for (name, why) in HAZARDS {
            assert!(why.len() > 40, "{name} explains too little");
            assert!(!name.is_empty());
        }
    }
}
