//! The full rights of the profile this app's session is running under.
//!
//! `glpi::session` already reads this blob, but keeps only the two rights the
//! dashboard needs. This module keeps all of them, because "which rights does
//! this account actually hold" is a question the GLPI UI answers only by
//! clicking through a profile form one tab at a time, and the API answers in
//! one call.
//!
//! WHERE THE DATA COMES FROM. `GET /session` includes
//! `active_profile.rights` - a map of right name to integer bitmask - but only
//! from API version 2.2 onwards. CoreController.php:625-634 in 11.0.12:
//!
//!   if (version_compare($this->getAPIVersion($request), '2.2', '>=')) {
//!       $all_right_names = array_keys(ProfileRight::getAllPossibleRights());
//!
//! This client speaks v2.3, so the map is present. On an older version the
//! key is simply absent and this yields an empty breakdown rather than
//! guessing - see `from_value`.
//!
//! Note what is NOT used here: `Administration/Profile` IS exposed by the HL
//! API, but its schema carries only id, name, comment, is_default,
//! helpdesk_hardware, helpdesk_item_type, managed_domainrecordtypes and
//! mfa_enforced (AdministrationController.php:645-699). The rights themselves
//! are not in it, and ProfileRight has no endpoint at all. So the session blob
//! is the only API route to this, and it describes THIS session's active
//! profile - not every profile in GLPI.
use serde::{Deserialize, Serialize};

/// GLPI's global right bits, from src/autoload/constants.php:66-78 in
/// 11.0.12. Read straight out of the release rather than remembered:
///
///   define("READ", 1);      define("UPDATE", 2);   define("CREATE", 4);
///   define("DELETE", 8);    define("PURGE", 16);   define("READNOTE", 32);
///   define("UPDATENOTE", 64); define("UNLOCK", 128);
///   define("READ_ASSIGNED", 256);  define("UPDATE_ASSIGNED", 512);
///   define("READ_OWNED", 1024);    define("UPDATE_OWNED", 2048);
///
/// ALLSTANDARDRIGHT (31) is deliberately absent: it is the OR of the first
/// five, not a bit of its own, and listing it would double-count.
const KNOWN_BITS: [(i64, &str); 12] = [
    (1, "READ"),
    (2, "UPDATE"),
    (4, "CREATE"),
    (8, "DELETE"),
    (16, "PURGE"),
    (32, "READNOTE"),
    (64, "UPDATENOTE"),
    (128, "UNLOCK"),
    (256, "READ_ASSIGNED"),
    (512, "UPDATE_ASSIGNED"),
    (1024, "READ_OWNED"),
    (2048, "UPDATE_OWNED"),
];

/// Every bit `KNOWN_BITS` can name. Anything outside this is real permission
/// that this code cannot label - see `unnamed_bits`.
const KNOWN_MASK: i64 = 4095;

/// The bits that let the holder change something.
///
/// UNLOCK is counted as one. It is not a standard CRUD bit, but what it grants
/// is clearing the lock on a dynamically-imported field so it can be edited,
/// which is a mutation however it is named.
const WRITE_BITS: i64 = 2 | 4 | 8 | 16 | 64 | 128 | 512 | 2048;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RightEntry {
    /// GLPI's own name for the right, e.g. "agent". Not prettified: it is the
    /// string to search for in GLPI's profile form and in its source.
    pub name: String,

    /// The raw bitmask. The authoritative value - everything below is a
    /// reading of it.
    pub value: i64,

    /// `value` decoded through GLPI's global constants, e.g. ["READ",
    /// "READNOTE"]. Empty when the right is not granted at all.
    pub bits: Vec<String>,

    /// Whatever `value` holds beyond the twelve global constants, 0 when
    /// there is none.
    ///
    /// Not an error and not padding. GLPI lets a module define its own bits on
    /// top of the global ones - Ticket and Problem both do, which is why a
    /// live `ticket` arrives as 138241 - and `Session::getRightNameForError`
    /// resolves those through `Profile::getRightsForForm()`, a form definition
    /// with no API route. So these bits are genuine permissions whose meaning
    /// is only knowable per module. They are reported rather than masked off:
    /// dropping them would understate what the account can do, which is the
    /// one direction a permissions screen must never err in.
    pub unnamed_bits: i64,

    /// A known write bit is set, so this right is not read-only.
    pub writes: bool,
}

impl RightEntry {
    fn new(name: String, value: i64) -> Self {
        Self {
            name,
            value,
            bits: KNOWN_BITS
                .iter()
                .filter(|(bit, _)| value & bit != 0)
                .map(|(_, label)| (*label).to_string())
                .collect(),
            unnamed_bits: value & !KNOWN_MASK,
            writes: value & WRITE_BITS != 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RightsBreakdown {
    /// Repeated from the session so this response stands on its own: a rights
    /// list means nothing without knowing whose rights it is, and GLPI
    /// activates exactly one profile per session regardless of how many the
    /// account holds.
    pub profile_name: Option<String>,
    pub profile_interface: Option<String>,

    /// Every right GLPI listed, granted or not, sorted by name. Includes the
    /// zero ones on purpose: "this right exists and you do not have it" is the
    /// answer to a 403, and omitting it leaves the reader unable to tell an
    /// ungranted right from one GLPI has never heard of.
    pub entries: Vec<RightEntry>,
}

impl RightsBreakdown {
    pub fn from_value(v: &serde_json::Value) -> Self {
        let profile = v.get("active_profile");
        let text = |key: &str| {
            profile
                .and_then(|p| p.get(key))
                .and_then(|x| x.as_str())
                .map(str::to_string)
        };

        // An absent rights map yields no entries, never a fabricated one. This
        // is the pre-2.2 API shape, and an empty list the frontend reports as
        // empty is honest; inventing defaults would describe an account that
        // does not exist.
        let mut entries: Vec<RightEntry> = profile
            .and_then(|p| p.get("rights"))
            .and_then(|r| r.as_object())
            .map(|map| {
                map.iter()
                    .map(|(name, value)| {
                        // GLPI casts these to int server-side, but a string
                        // survives a JSON round trip just as easily; parse both
                        // rather than silently scoring a quoted value as zero.
                        let n = value
                            .as_i64()
                            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                            .unwrap_or(0);
                        RightEntry::new(name.clone(), n)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // By name, not by value: this list is read by someone looking for one
        // particular right, and 122 of them in GLPI's own map order is
        // unsearchable by eye.
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            profile_name: text("name"),
            profile_interface: text("interface"),
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values taken from a live v2.3 /session response, not invented, so this
    /// pins the decoding against the real thing.
    #[test]
    fn decodes_the_values_a_live_glpi_returns() {
        let v = serde_json::json!({
            "active_profile": {
                "name": "inv",
                "interface": "central",
                "rights": {
                    "agent": 1,
                    "entity": 33,
                    "computer": 1313,
                    "name": 0,
                }
            }
        });
        let b = RightsBreakdown::from_value(&v);
        assert_eq!(b.profile_name.as_deref(), Some("inv"));

        let by = |n: &str| b.entries.iter().find(|e| e.name == n).unwrap().clone();

        // Read alone.
        let agent = by("agent");
        assert_eq!(agent.bits, vec!["READ"]);
        assert!(!agent.writes);
        assert_eq!(agent.unnamed_bits, 0);

        // 33 = READ | READNOTE, and a note is not a write.
        assert_eq!(by("entity").bits, vec!["READ", "READNOTE"]);
        assert!(!by("entity").writes);

        // 1313 = 1 | 32 | 256 | 1024, all four of them read bits.
        let computer = by("computer");
        assert_eq!(
            computer.bits,
            vec!["READ", "READNOTE", "READ_ASSIGNED", "READ_OWNED"]
        );
        assert!(!computer.writes);

        // Not granted, and still listed.
        let name = by("name");
        assert_eq!(name.value, 0);
        assert!(name.bits.is_empty());
    }

    /// The property that matters most: a bit this code cannot name must be
    /// reported, never masked off. 132129 is a live `change` right - 1 | 32 |
    /// 1024 plus 131072, which belongs to the ITIL module's own definitions.
    #[test]
    fn a_bit_outside_the_global_constants_is_reported_not_dropped() {
        let v = serde_json::json!({
            "active_profile": { "rights": { "change": 132129, "task": 8193 } }
        });
        let b = RightsBreakdown::from_value(&v);
        let by = |n: &str| b.entries.iter().find(|e| e.name == n).unwrap().clone();

        let change = by("change");
        assert_eq!(change.value, 132129);
        assert_eq!(change.bits, vec!["READ", "READNOTE", "READ_OWNED"]);
        assert_eq!(
            change.unnamed_bits, 131072,
            "an unnameable bit must survive into the output"
        );

        // 8193 = READ plus 8192, which no global constant covers.
        assert_eq!(by("task").unnamed_bits, 8192);
        assert_eq!(by("task").bits, vec!["READ"]);
    }

    #[test]
    fn write_bits_are_recognised() {
        let v = serde_json::json!({
            "active_profile": { "rights": {
                "a": 31,   // ALLSTANDARDRIGHT
                "b": 2,    // UPDATE alone
                "c": 128,  // UNLOCK alone
                "d": 1057, // READ | READNOTE | READ_OWNED - reads only
            }}
        });
        let b = RightsBreakdown::from_value(&v);
        let writes = |n: &str| b.entries.iter().find(|e| e.name == n).unwrap().writes;
        assert!(writes("a"), "ALLSTANDARDRIGHT contains UPDATE");
        assert!(writes("b"));
        assert!(writes("c"), "UNLOCK mutates and must count as a write");
        assert!(!writes("d"), "read bits alone must not be called a write");

        // ALLSTANDARDRIGHT is an alias for five bits, not a thirteenth bit, so
        // it must decode to those five and leave nothing unnamed.
        let all = b.entries.iter().find(|e| e.name == "a").unwrap();
        assert_eq!(
            all.bits,
            vec!["READ", "UPDATE", "CREATE", "DELETE", "PURGE"]
        );
        assert_eq!(all.unnamed_bits, 0);
    }

    /// Pre-2.2 GLPI omits the map entirely. Empty is the honest answer.
    #[test]
    fn a_session_without_rights_yields_no_entries_rather_than_panicking() {
        let b = RightsBreakdown::from_value(&serde_json::json!({}));
        assert!(b.entries.is_empty() && b.profile_name.is_none());

        let b = RightsBreakdown::from_value(&serde_json::json!({
            "active_profile": { "name": "inv" }
        }));
        assert!(b.entries.is_empty());
        assert_eq!(b.profile_name.as_deref(), Some("inv"));
    }

    /// GLPI casts to int, but a quoted number must not score as ungranted -
    /// that would read as "permission missing" on screen.
    #[test]
    fn a_quoted_value_is_parsed_rather_than_treated_as_zero() {
        let b = RightsBreakdown::from_value(&serde_json::json!({
            "active_profile": { "rights": { "agent": "1" } }
        }));
        assert_eq!(b.entries[0].value, 1);
        assert_eq!(b.entries[0].bits, vec!["READ"]);
    }
}
