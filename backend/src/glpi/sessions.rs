//! Who is logged into GLPI, how, and from where. New in 12.0.1.
//!
//! WHY THIS IS HERE. The Rights page answers "what may this account do" and
//! the Api tab answers "did the asking work". Neither can say who else is
//! currently inside GLPI. `User_SessionHistory` is new in 12.0 and is the
//! first time that is answerable at all - 11.0.12 has no equivalent at any API
//! version.
//!
//! THE TRAP, AND WHY THIS MODULE EXISTS RATHER THAN A BARE CALL.
//! `/Security/LoginSession/All` returns **200 whether or not the caller may
//! see everything**. Its `x-rights-conditions` degrade instead of refusing:
//!
//! ```php
//! if (Session::haveRight(Config::$rightname, UPDATE)) { return true; }
//! return ['WHERE' => ['_.users_id' => Session::getLoginUserID()]];
//! ```
//!
//! So without `config` at UPDATE the route silently narrows to the caller's
//! own sessions. A reader cannot tell "nobody else is logged in" from "I am
//! not allowed to see who is" - which is exactly what CLAUDE.md forbids
//! rendering as an empty list.
//!
//! This app's account holds `config` at READ (1), not UPDATE (2), so on this
//! install the narrow view is what it gets. `Scope` carries that distinction
//! to the screen instead of leaving the list to be misread.
//!
//! The authority test is cheap and needs no extra right: v3's `session`
//! endpoint carries `active_profile.rights`, verified against a running
//! 12.0.1. `READ` is 1 and `UPDATE` is 2 (`src/autoload/constants.php`), so it
//! is a single bit.
use anyhow::Result;
use serde::Serialize;

use super::{Client, HL_V3};

/// GLPI's `UPDATE` right bit.
const UPDATE: i64 = 2;

/// How much of GLPI's session list this account was actually shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// `config` UPDATE is held, so the list covers every user.
    Fleet,
    /// It is not, so GLPI narrowed the list to this account's own sessions
    /// without saying so.
    SelfOnly,
    /// The account's own rights could not be read, so which of the two above
    /// applies is unknown. Never treated as `Fleet`.
    Unknown,
}

/// How a session authenticated. GLPI's `Auth` constants, which are stable
/// integers in `src/Auth.php`.
fn auth_label(v: Option<i64>) -> &'static str {
    match v {
        Some(0) => "not yet authenticated",
        Some(1) => "GLPI database",
        Some(2) => "mail",
        Some(3) => "LDAP",
        Some(4) => "external",
        Some(5) => "CAS",
        Some(6) => "X509",
        Some(7) => "API",
        Some(9) => "OAuth",
        // 8 is unused upstream, and a value this build has not seen should
        // still reach the screen as something rather than be hidden.
        _ => "unknown",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginSession {
    pub id: Option<i64>,
    /// GLPI's opaque session id. Shown because it is what a revoke takes.
    pub login_session_uid: Option<String>,
    pub user: Option<String>,
    pub user_id: Option<i64>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub auth_type: Option<i64>,
    /// `auth_type` decoded. Carried rather than left to the frontend so both
    /// halves cannot disagree about what 9 means.
    pub auth_label: &'static str,
    pub logged_in_at: Option<String>,
    /// Absent or null while the session is still live.
    pub logged_out_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub scope: Scope,
    pub sessions: Vec<LoginSession>,
    /// What this cannot establish, carried with the result as elsewhere in
    /// this app.
    pub cannot_check: &'static str,
}

const CANNOT_CHECK: &str = "GLPI narrows this list to the caller's own \
sessions unless the account holds config at UPDATE, and answers 200 either \
way - so a short list means \u{201c}self only\u{201d} rather than \
\u{201c}nobody else\u{201d} whenever the scope below says so. \
User_SessionHistory is new in 12.0, so it records nothing from before the \
upgrade and cannot describe sessions that predate it.";

/// Read `active_profile.rights.config` and test the UPDATE bit.
///
/// Returns `Unknown` rather than guessing when the field is missing: a build
/// of GLPI that stops reporting rights must not silently read as `Fleet`.
fn scope_from_session(session: &serde_json::Value) -> Scope {
    let config = session
        .get("active_profile")
        .and_then(|p| p.get("rights"))
        .and_then(|r| r.get("config"))
        .and_then(|c| c.as_i64());

    match config {
        Some(bits) if bits & UPDATE != 0 => Scope::Fleet,
        Some(_) => Scope::SelfOnly,
        None => Scope::Unknown,
    }
}

/// Parse one row of the `LoginSession` collection.
fn one(row: &serde_json::Value) -> LoginSession {
    let s = |k: &str| {
        row.get(k)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let auth_type = row.get("auth_type").and_then(|v| v.as_i64());

    LoginSession {
        id: row.get("id").and_then(|v| v.as_i64()),
        login_session_uid: s("login_session_uid"),
        // `user` is a dropdown object, not a string.
        user: row
            .get("user")
            .and_then(|u| u.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        user_id: row
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(|v| v.as_i64()),
        ip_address: s("ip_address"),
        user_agent: s("user_agent_string"),
        auth_type,
        auth_label: auth_label(auth_type),
        logged_in_at: s("logged_in_at"),
        logged_out_at: s("logged_out_at"),
    }
}

/// Build a report from an already-fetched session list and session document.
///
/// Pure, so the scope rule is testable without a GLPI - the same split
/// `posture`, `schema_check` and `system_status` use.
pub fn parse(list: &serde_json::Value, own_session: &serde_json::Value) -> Report {
    let sessions = list
        .as_array()
        .map(|rows| rows.iter().map(one).collect())
        .unwrap_or_default();

    Report {
        scope: scope_from_session(own_session),
        sessions,
        cannot_check: CANNOT_CHECK,
    }
}

/// Fetch the login sessions this account may see, and say which that is.
///
/// Two calls, both on v3: the list, and the session that establishes whether
/// the list is complete. Asking for `/All` regardless is deliberate - GLPI
/// narrows it rather than refusing, so the same call is correct either way and
/// only the label changes.
pub async fn fetch(client: &Client) -> Result<Report> {
    let list = client.get_at(HL_V3, "Security/LoginSession/All").await?;
    let own = client.get_at(HL_V3, "session").await?;
    Ok(parse(&list, &own))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_config(bits: Option<i64>) -> serde_json::Value {
        match bits {
            Some(b) => serde_json::json!({
                "active_profile": { "name": "inv", "rights": { "config": b } }
            }),
            None => serde_json::json!({ "active_profile": { "name": "inv" } }),
        }
    }

    /// This install: `config` is 1, so READ without UPDATE.
    #[test]
    fn config_read_only_is_self_only_not_fleet() {
        assert_eq!(
            scope_from_session(&session_with_config(Some(1))),
            Scope::SelfOnly
        );
    }

    #[test]
    fn config_with_the_update_bit_is_fleet_wide() {
        // 2 is UPDATE alone; 3 is READ|UPDATE; 15 is all standard rights.
        for bits in [2, 3, 15, 127] {
            assert_eq!(
                scope_from_session(&session_with_config(Some(bits))),
                Scope::Fleet,
                "config={bits} holds UPDATE"
            );
        }
    }

    #[test]
    fn config_absent_entirely_is_self_only() {
        assert_eq!(
            scope_from_session(&session_with_config(Some(0))),
            Scope::SelfOnly
        );
    }

    /// The important negative: unreadable rights must not read as fleet-wide,
    /// because that would present a narrowed list as complete.
    #[test]
    fn unreadable_rights_are_unknown_never_fleet() {
        assert_eq!(
            scope_from_session(&session_with_config(None)),
            Scope::Unknown
        );
        assert_eq!(scope_from_session(&serde_json::json!({})), Scope::Unknown);
    }

    #[test]
    fn a_row_is_decoded_including_the_nested_user() {
        let list = serde_json::json!([{
            "id": 1,
            "login_session_uid": "f79df8a7cd2d9938",
            "user": { "id": 7, "name": "inv" },
            "ip_address": "127.0.0.1",
            "user_agent_string": "",
            "auth_type": 9,
            "logged_in_at": "2026-11-04T11:16:43+01:00",
            "logged_out_at": null
        }]);
        let r = parse(&list, &session_with_config(Some(1)));

        assert_eq!(r.scope, Scope::SelfOnly);
        assert_eq!(r.sessions.len(), 1);
        let s = &r.sessions[0];
        assert_eq!(s.user.as_deref(), Some("inv"));
        assert_eq!(s.user_id, Some(7));
        assert_eq!(s.auth_label, "OAuth");
        assert_eq!(s.login_session_uid.as_deref(), Some("f79df8a7cd2d9938"));
        // An empty user-agent is None, not Some(""), so the UI renders a
        // blank rather than an empty-looking value.
        assert_eq!(s.user_agent, None);
        assert_eq!(s.logged_out_at, None, "still logged in");
    }

    #[test]
    fn every_auth_constant_upstream_defines_has_a_label() {
        for (v, want) in [
            (0, "not yet authenticated"),
            (1, "GLPI database"),
            (2, "mail"),
            (3, "LDAP"),
            (4, "external"),
            (5, "CAS"),
            (6, "X509"),
            (7, "API"),
            (9, "OAuth"),
        ] {
            assert_eq!(auth_label(Some(v)), want, "auth_type {v}");
        }
        // 8 is not defined upstream, and neither is anything above 9.
        assert_eq!(auth_label(Some(8)), "unknown");
        assert_eq!(auth_label(Some(99)), "unknown");
        assert_eq!(auth_label(None), "unknown");
    }

    /// An empty list is still a valid answer - but it means different things
    /// depending on scope, which is why scope travels with it.
    #[test]
    fn an_empty_list_keeps_its_scope() {
        let r = parse(&serde_json::json!([]), &session_with_config(Some(1)));
        assert!(r.sessions.is_empty());
        assert_eq!(r.scope, Scope::SelfOnly);

        let r = parse(&serde_json::json!([]), &session_with_config(Some(3)));
        assert!(r.sessions.is_empty());
        assert_eq!(r.scope, Scope::Fleet, "genuinely nobody is logged in");
    }

    #[test]
    fn a_non_array_response_is_no_sessions_rather_than_a_panic() {
        let r = parse(
            &serde_json::json!({"detail": "nope"}),
            &session_with_config(Some(1)),
        );
        assert!(r.sessions.is_empty());
    }
}
