//! GLPI's own health, including the automatic actions that quietly stopped.
//!
//! WHY THIS IS IN THIS APP. This whole dashboard exists because GLPI has no
//! notion of an inventory agent being overdue. GLPI's *own* scheduled work has
//! the same shape of failure - a task that stops running looks exactly like a
//! task with nothing to do - and GLPI does report that, at
//! `/api.php/v2.3/status/all`. Nothing here read it until now.
//!
//! On this install it was not hypothetical: the first authenticated read
//! returned `stuck: ["queuedwebhook"]`, a task nobody had noticed.
//!
//! 12.0 added the `errored` list beside the existing `stuck` one, plus `ERROR`
//! and `ABORTED` task states and a backoff-retry, so the shape below is richer
//! on 12.0.1 than on 11.0.12. The `status_msg` string also changed format -
//! upstream's changelog says `ERROR` and the code says `ERRORED` - which is
//! exactly why this reads the structured `stuck`/`errored` arrays and never
//! parses that sentence.
//!
//! THREE TRAPS IN THIS ENDPOINT, all verified against
//! `src/Glpi/System/Status/StatusChecker.php` in the 12.0.1 release:
//!
//! 1. **`glpi.status` is not GLPI's status.** `getServiceStatus` ends with
//!    `$status['glpi']['status'] = self::calculateGlobalStatus($status)`, so
//!    that field is the rollup across *every* service, overwritten into the
//!    `glpi` object. The GLPI service's own health is
//!    `glpi.database_version.status`. Reading the former as the latter reports
//!    a database problem whenever any unrelated service is unhappy - which is
//!    the state this install is in right now.
//!
//! 2. **Unauthenticated, `uptodate` is hardcoded false.**
//!    `'uptodate' => $public_only ? false : Update::isDbUpToDate()`, with
//!    `defined` and `installed` replaced by the literal `REDACTED`. So an
//!    anonymous read reports the schema as out of date on a perfectly current
//!    install. `public_only` is `!Session::haveRight(Config::$rightname, READ)`
//!    and this app's account holds `config` at READ, so an authenticated call
//!    gets the truth. That is lucky rather than designed, and it can silently
//!    flip back if the token fails or the right is withdrawn, so
//!    `Report::redacted` detects it instead of trusting it.
//!
//! 3. **`NO_DATA` is not healthy and not broken.** Most services on a small
//!    install report it because they are not configured - no LDAP, no IMAP.
//!    It maps to `Unknown`, never to `Ok`.
use anyhow::Result;
use serde::Serialize;

use super::posture::State;

/// Map GLPI's status vocabulary onto this app's three states.
///
/// `WARNING` joins `PROBLEM` as `Attention`: both are things to look at, and
/// this app deliberately has no severity ladder beyond "look at this".
fn state_of(s: Option<&str>) -> State {
    match s {
        Some("OK") => State::Ok,
        Some("PROBLEM") | Some("WARNING") => State::Attention,
        // NO_DATA, absent, or a value this build has never heard of.
        _ => State::Unknown,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Service {
    pub name: String,
    pub state: State,
    /// GLPI's own word for it, carried through unmapped so a value this build
    /// does not know still reaches the screen.
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The rollup across every service - read from `glpi.status`, which is
    /// what that field actually holds. See trap 1.
    pub overall: State,

    /// GLPI's schema-version check: its real health, from
    /// `glpi.database_version.status`, not from `glpi.status`.
    pub schema_state: State,
    pub schema_defined: Option<String>,
    pub schema_installed: Option<String>,

    /// True when GLPI served the anonymous view, so `schema_*` above is
    /// meaningless and `uptodate` was hardcoded false. A finding in its own
    /// right: it means this app is reading GLPI with less authority than it
    /// should have.
    pub redacted: bool,

    /// Automatic actions GLPI considers hung. Named, because the name is the
    /// only actionable part.
    pub stuck: Vec<String>,
    /// Automatic actions that failed repeatedly and now need manual
    /// intervention. New in 12.0; always empty on 11.0.12.
    pub errored: Vec<String>,
    pub crontask_state: State,
    /// GLPI's own summary sentence, shown but never parsed.
    pub crontask_summary: Option<String>,

    /// Every service the endpoint reported, including the configured-away
    /// ones, so a reader can see what was actually checked.
    pub services: Vec<Service>,

    pub cannot_check: &'static str,
}

const CANNOT_CHECK: &str = "Reports what GLPI reports about itself. A task \
GLPI never scheduled cannot appear here, and \u{201c}stuck\u{201d} is GLPI's \
own judgement (getZombieCronTasks), not an independent one. Services reading \
NO_DATA are usually unconfigured rather than broken, which is why they are \
shown as unknown and never as healthy.";

/// The strings GLPI uses for a redacted value and for its own service names.
const REDACTED: &str = "REDACTED";

/// Build a report from the parsed `/status/all` document.
///
/// Pure over an already-fetched value, like `posture` and `schema_check`, so
/// every trap above is testable without a GLPI.
pub fn parse(doc: &serde_json::Value) -> Report {
    let glpi = doc.get("glpi");
    let dbv = glpi.and_then(|g| g.get("database_version"));
    let str_at = |v: Option<&serde_json::Value>, k: &str| -> Option<String> {
        v.and_then(|x| x.get(k))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    };

    let defined = str_at(dbv, "defined");
    let installed = str_at(dbv, "installed");
    let redacted = defined.as_deref() == Some(REDACTED) || installed.as_deref() == Some(REDACTED);

    let crontasks = doc.get("crontasks");
    let names = |k: &str| -> Vec<String> {
        crontasks
            .and_then(|c| c.get(k))
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|n| n.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    // Every top-level key is a service. Taken from the document rather than
    // from a list here, so a service GLPI adds appears by itself.
    let mut services: Vec<Service> = doc
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(name, v)| Service {
                    name: name.clone(),
                    state: state_of(v.get("status").and_then(|s| s.as_str())),
                    raw: str_at(Some(v), "status"),
                })
                .collect()
        })
        .unwrap_or_default();
    services.sort_by(|a, b| a.name.cmp(&b.name));

    Report {
        overall: state_of(glpi.and_then(|g| g.get("status")).and_then(|s| s.as_str())),
        // Unknown rather than Ok when the view is redacted: the field is
        // present and says WARNING, but it was computed from a hardcoded
        // false and means nothing.
        schema_state: if redacted {
            State::Unknown
        } else {
            state_of(dbv.and_then(|d| d.get("status")).and_then(|s| s.as_str()))
        },
        schema_defined: defined.filter(|d| d != REDACTED),
        schema_installed: installed.filter(|d| d != REDACTED),
        redacted,
        stuck: names("stuck"),
        errored: names("errored"),
        crontask_state: state_of(
            crontasks
                .and_then(|c| c.get("status"))
                .and_then(|s| s.as_str()),
        ),
        crontask_summary: str_at(crontasks, "status_msg"),
        services,
        cannot_check: CANNOT_CHECK,
    }
}

/// Fetch and parse GLPI's status document.
///
/// `status/all` on the default HL version. The route is `SECURITY_NONE`, so it
/// answers without a token at all - but authenticated is what gives the
/// unredacted view, so this goes through the normal client rather than a bare
/// request. See trap 2.
pub async fn fetch(client: &super::Client) -> Result<Report> {
    let doc = client.get("status/all").await?;
    Ok(parse(&doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The authenticated shape, as this install actually returned it - note
    /// `glpi.status` PROBLEM beside `database_version.status` OK, which is
    /// trap 1 in the wild.
    fn authenticated() -> serde_json::Value {
        serde_json::json!({
            "glpi": {
                "status": "PROBLEM",
                "database_version": {
                    "defined": "12.0.1@5768fcc",
                    "installed": "12.0.1@5768fcc",
                    "status": "OK",
                    "uptodate": true
                }
            },
            "db": { "status": "OK", "main": { "status": "OK" } },
            "ldap": { "status": "NO_DATA", "servers": [] },
            "crontasks": {
                "status": "PROBLEM",
                "stuck": ["queuedwebhook"],
                "errored": [],
                "status_msg": "RUNNING: 1, STUCK: 1, ERRORED: 0, TOTAL: 50"
            },
            "filesystem": { "status": "OK" }
        })
    }

    /// The anonymous shape: REDACTED values and a hardcoded-false uptodate.
    fn anonymous() -> serde_json::Value {
        serde_json::json!({
            "glpi": {
                "status": "PROBLEM",
                "database_version": {
                    "defined": "REDACTED",
                    "installed": "REDACTED",
                    "uptodate": false,
                    "status": "NO_DATA"
                }
            },
            "crontasks": { "status": "OK", "stuck": [], "errored": [] }
        })
    }

    #[test]
    fn a_stuck_task_is_named() {
        let r = parse(&authenticated());
        assert_eq!(r.crontask_state, State::Attention);
        assert_eq!(r.stuck, vec!["queuedwebhook"]);
        assert!(r.errored.is_empty());
    }

    /// Trap 1: the GLPI service's own health must not be read off
    /// `glpi.status`, which is the global rollup.
    #[test]
    fn the_global_rollup_is_not_mistaken_for_the_schema_check() {
        let r = parse(&authenticated());
        assert_eq!(
            r.overall,
            State::Attention,
            "rollup reflects the stuck task"
        );
        assert_eq!(
            r.schema_state,
            State::Ok,
            "the schema check itself is fine and must not inherit the rollup"
        );
        assert_eq!(r.schema_installed.as_deref(), Some("12.0.1@5768fcc"));
    }

    /// Trap 2: an anonymous read must not be reported as a real schema
    /// problem, because `uptodate` was not computed.
    #[test]
    fn a_redacted_view_is_unknown_not_a_finding() {
        let r = parse(&anonymous());
        assert!(r.redacted);
        assert_eq!(r.schema_state, State::Unknown);
        assert_eq!(r.schema_defined, None, "REDACTED is not a version string");
        assert_eq!(r.schema_installed, None);
    }

    /// Trap 3: unconfigured services are unknown, never healthy.
    #[test]
    fn no_data_is_unknown_not_ok() {
        let r = parse(&authenticated());
        let ldap = r.services.iter().find(|s| s.name == "ldap").unwrap();
        assert_eq!(ldap.state, State::Unknown);
        assert_eq!(ldap.raw.as_deref(), Some("NO_DATA"));
    }

    #[test]
    fn services_are_taken_from_the_document_and_sorted() {
        let r = parse(&authenticated());
        let names: Vec<&str> = r.services.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["crontasks", "db", "filesystem", "glpi", "ldap"]);
    }

    /// 11.0.12 has no `errored` key at all. It must read as empty rather than
    /// failing the parse, so the same build serves both versions.
    #[test]
    fn a_missing_errored_list_is_empty_not_an_error() {
        let doc = serde_json::json!({
            "crontasks": { "status": "OK", "stuck": [] }
        });
        let r = parse(&doc);
        assert!(r.errored.is_empty());
        assert_eq!(r.crontask_state, State::Ok);
    }

    #[test]
    fn an_empty_document_is_unknown_throughout() {
        let r = parse(&serde_json::json!({}));
        assert_eq!(r.overall, State::Unknown);
        assert_eq!(r.schema_state, State::Unknown);
        assert_eq!(r.crontask_state, State::Unknown);
        assert!(!r.redacted, "nothing was redacted; nothing was served");
        assert!(r.services.is_empty());
    }

    /// A status word this build has never seen must not become Ok.
    #[test]
    fn an_unknown_status_word_is_not_healthy() {
        let doc = serde_json::json!({ "crontasks": { "status": "DEGRADED" } });
        let r = parse(&doc);
        assert_eq!(r.crontask_state, State::Unknown);
        assert_eq!(r.services[0].raw.as_deref(), Some("DEGRADED"));
    }
}
