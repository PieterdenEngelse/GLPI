//! Security posture: four high-level questions, each answered from data GLPI
//! already reports.
//!
//! Every check here is a pure function over values the caller has already
//! fetched. That keeps the judgements testable without a GLPI, and keeps the
//! fetching - which is the awkward part, spanning two APIs - in one place.
//!
//! THREE STATES, NOT TWO. Most of this cannot be fully determined: a password
//! cannot be verified, only observed never to have changed. `Unknown` exists
//! so that "we could not tell" never renders as "fine", which is the same
//! rule the rest of this app applies to an empty list versus a failed fetch.
//!
//! NO SCORE. There is deliberately no overall number. One figure hides which
//! check failed and invites tuning the figure rather than the system.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Ok,
    Attention,
    Unknown,
}

impl State {
    /// The worst of a set, for a tile's headline. Attention outranks Unknown:
    /// a thing known to be wrong is more actionable than a thing unmeasured.
    fn worst(states: impl IntoIterator<Item = State>) -> State {
        states.into_iter().fold(State::Ok, |acc, s| match (acc, s) {
            (State::Attention, _) | (_, State::Attention) => State::Attention,
            (State::Unknown, _) | (_, State::Unknown) => State::Unknown,
            _ => State::Ok,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub state: State,
    pub label: String,
    pub detail: String,
    /// Path into GLPI where this is changed, relative to its base URL.
    pub link: Option<String>,
}

impl Finding {
    fn new(state: State, label: &str, detail: String, link: Option<&str>) -> Self {
        Self {
            state,
            label: label.into(),
            detail,
            link: link.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Tile {
    pub id: &'static str,
    pub title: &'static str,
    pub state: State,
    pub findings: Vec<Finding>,
    /// What this tile cannot establish. Stated on the tile itself, because a
    /// reader's trust should match the check's actual reach.
    pub cannot_check: &'static str,
}

impl Tile {
    fn new(
        id: &'static str,
        title: &'static str,
        cannot_check: &'static str,
        findings: Vec<Finding>,
    ) -> Self {
        Self {
            id,
            title,
            state: State::worst(findings.iter().map(|f| f.state)),
            findings,
            cannot_check,
        }
    }
}

// ---------------------------------------------------------------- access

pub struct AccessInput<'a> {
    pub profile_name: Option<&'a str>,
    pub interface: Option<&'a str>,
    /// Rights on the active profile carrying any write bit.
    pub write_rights: Vec<String>,
    /// Authorizations the account holds other than the one in force.
    pub spare_authorizations: Vec<String>,
}

/// What this app itself may do in GLPI.
///
/// The app only ever reads, so any write right it holds is authority it
/// cannot use and would hand over if its credentials leaked.
pub fn access(input: &AccessInput) -> Tile {
    let mut findings = Vec::new();

    findings.push(match input.profile_name {
        None => Finding::new(
            State::Unknown,
            "Active profile",
            "GLPI did not report which profile this session is using.".into(),
            None,
        ),
        Some(name) => Finding::new(
            State::Ok,
            "Active profile",
            format!("Running as {name}."),
            Some("front/profile.php"),
        ),
    });

    findings.push(if input.write_rights.is_empty() {
        Finding::new(
            State::Ok,
            "Read-only",
            "The active profile grants no create, update or delete anywhere.".into(),
            None,
        )
    } else {
        let n = input.write_rights.len();
        let sample = input
            .write_rights
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        Finding::new(
            State::Attention,
            "Write access held but never used",
            format!(
                "{n} right{} on this profile allow changing data: {sample}{}. This app only \
                 reads, so that authority is unused - and available to anyone who obtains its \
                 credentials.",
                if n == 1 { "" } else { "s" },
                if n > 6 { ", …" } else { "" }
            ),
            Some("front/profile.php"),
        )
    });

    findings.push(match input.interface {
        Some("central") => Finding::new(
            State::Ok,
            "Interface",
            "Standard, which is what the inventory rights require.".into(),
            None,
        ),
        Some(other) => Finding::new(
            State::Attention,
            "Interface",
            format!("{other}. The rights this app needs cannot exist on a simplified profile."),
            Some("front/profile.php"),
        ),
        None => Finding::new(State::Unknown, "Interface", "Not reported.".into(), None),
    });

    if !input.spare_authorizations.is_empty() {
        findings.push(Finding::new(
            State::Attention,
            "Unused authorizations",
            format!(
                "This account may also sign in as: {}. GLPI activates one profile per session, \
                 so these grant nothing today - but they widen what the account becomes if its \
                 default is ever changed.",
                input.spare_authorizations.join(", ")
            ),
            Some("front/user.php"),
        ));
    }

    Tile::new(
        "access",
        "Access",
        "Whether the credentials themselves are strong, or where they are stored. This \
         describes authority, not secrecy.",
        findings,
    )
}

// -------------------------------------------------------------- accounts

pub struct UserRow {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    pub is_deleted: bool,
    pub last_login: Option<String>,
    pub password_changed: Option<String>,
}

/// GLPI ships these four with published passwords.
const SHIPPED: [&str; 4] = ["glpi", "post-only", "tech", "normal"];

/// GLPI's own logins.
///
/// `system_user_id` comes from GLPI's own config (`cfg_glpi.system_user`) and
/// is excluded from every check. That account exists for GLPI's internal
/// operations and cannot sign in, so "its password was never changed" is true
/// and meaningless. Flagging it would be the kind of false positive that
/// teaches a reader to stop reading the tile - which costs more than the
/// check is worth.
pub fn accounts(users: &[UserRow], system_user_id: Option<i64>) -> Tile {
    if users.is_empty() {
        return Tile::new(
            "accounts",
            "Accounts",
            "Whether any password is actually a default - only whether one was ever changed.",
            vec![Finding::new(
                State::Unknown,
                "Accounts",
                "GLPI returned no users. This app's account may lack permission to list them."
                    .into(),
                Some("front/user.php"),
            )],
        );
    }

    let live = |u: &&UserRow| u.is_active && !u.is_deleted && Some(u.id) != system_user_id;
    let mut findings = Vec::new();

    let shipped: Vec<&UserRow> = users
        .iter()
        .filter(|u| SHIPPED.contains(&u.name.as_str()) && live(u))
        .collect();

    findings.push(if shipped.is_empty() {
        Finding::new(
            State::Ok,
            "Accounts GLPI ships with",
            "None of the four default accounts are active.".into(),
            None,
        )
    } else {
        Finding::new(
            State::Attention,
            "Accounts GLPI ships with",
            format!(
                "Still active: {}. Their passwords are published in GLPI's documentation, so \
                 each is only as safe as the change somebody remembered to make.",
                shipped
                    .iter()
                    .map(|u| u.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some("front/user.php"),
        )
    });

    let never_changed: Vec<&UserRow> = users
        .iter()
        .filter(|u| live(u) && u.password_changed.is_none())
        .collect();

    if !never_changed.is_empty() {
        findings.push(Finding::new(
            State::Attention,
            "Password never changed",
            format!(
                "{} still carry the password they were created with: {}.",
                never_changed.len(),
                never_changed
                    .iter()
                    .map(|u| u.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some("front/user.php"),
        ));
    }

    let dormant: Vec<&UserRow> = users
        .iter()
        .filter(|u| live(u) && u.last_login.is_none() && u.password_changed.is_some())
        .collect();

    if !dormant.is_empty() {
        findings.push(Finding::new(
            State::Attention,
            "Active but never signed in",
            format!(
                "{}. An account nobody uses is an account nobody notices being used.",
                dormant
                    .iter()
                    .map(|u| u.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some("front/user.php"),
        ));
    }

    Tile::new(
        "accounts",
        "Accounts",
        "Whether any password is actually a default - only whether one was ever changed. A \
         changed password can still be weak.",
        findings,
    )
}

// -------------------------------------------------------------- exposure

pub struct ExposureInput<'a> {
    /// What this dashboard binds to.
    pub bind: &'a str,
    /// GLPI's own configured base URL.
    pub url_base: Option<&'a str>,
    pub enable_api: Option<bool>,
    pub enable_api_login_credentials: Option<bool>,
}

/// What is reachable, and by whom.
pub fn exposure(input: &ExposureInput) -> Tile {
    let mut findings = Vec::new();

    let loopback = input.bind.starts_with("127.") || input.bind.starts_with("[::1]");
    findings.push(if loopback {
        Finding::new(
            State::Ok,
            "This dashboard",
            format!(
                "Bound to {}, so it is reachable only from this machine.",
                input.bind
            ),
            None,
        )
    } else {
        Finding::new(
            State::Attention,
            "This dashboard",
            format!(
                "Bound to {}, which is wider than loopback. It holds GLPI credentials and has \
                 no login of its own, so anyone who can reach it can read GLPI through it.",
                input.bind
            ),
            None,
        )
    });

    findings.push(match input.url_base {
        None => Finding::new(State::Unknown, "GLPI address", "Not reported.".into(), None),
        Some(u) if u.starts_with("https://") => {
            Finding::new(State::Ok, "GLPI address", format!("{u} - encrypted."), None)
        }
        Some(u) => Finding::new(
            if u.contains("localhost") || u.contains("127.0.0.1") {
                State::Ok
            } else {
                State::Attention
            },
            "GLPI address",
            format!(
                "{u} - plain HTTP.{}",
                if u.contains("localhost") || u.contains("127.0.0.1") {
                    " On loopback that is fine: the traffic never reaches a network."
                } else {
                    " Credentials and session cookies cross the network in clear."
                }
            ),
            None,
        ),
    });

    findings.push(match input.enable_api_login_credentials {
        None => Finding::new(
            State::Unknown,
            "API login with a password",
            "Not reported.".into(),
            None,
        ),
        Some(false) => Finding::new(
            State::Ok,
            "API login with a password",
            "Disabled. GLPI's API accepts tokens only.".into(),
            None,
        ),
        Some(true) => Finding::new(
            State::Ok,
            "API login with a password",
            "Enabled, and this app depends on it: GLPI's older API is the only route to \
             profile and authorization data, and it rejects the token the rest of the app \
             uses. Disabling it would close an entry point and break the Rights and Status \
             pages. A dependency to know about, not a fault to fix."
                .into(),
            Some("front/config.form.php"),
        ),
    });

    Tile::new(
        "exposure",
        "Exposure",
        "Anything outside GLPI - a reverse proxy, the firewall, or who else is on the \
         network.",
        findings,
    )
}

// -------------------------------------------------------------- versions

/// What is running.
pub fn versions(glpi_version: Option<&str>, agents: &[(String, Option<String>)]) -> Tile {
    let mut findings = Vec::new();

    findings.push(match glpi_version {
        Some(v) => Finding::new(State::Ok, "GLPI", format!("Version {v}."), None),
        None => Finding::new(
            State::Unknown,
            "GLPI",
            "Version not reported. GLPI exposes it only through its older API.".into(),
            None,
        ),
    });

    let known: Vec<&str> = agents.iter().filter_map(|(_, v)| v.as_deref()).collect();
    if known.is_empty() {
        findings.push(Finding::new(
            State::Unknown,
            "Agents",
            "No agent reported a version.".into(),
            None,
        ));
    } else {
        // Highest string wins. Not semver-aware on purpose: inventing an
        // ordering for versions this app does not control would be a guess,
        // and the useful signal is only "they differ".
        let newest = known.iter().max().copied().unwrap_or_default().to_string();
        let behind: Vec<&String> = agents
            .iter()
            .filter(|(_, v)| v.as_deref().is_some_and(|v| v != newest))
            .map(|(name, _)| name)
            .collect();

        findings.push(if behind.is_empty() {
            Finding::new(
                State::Ok,
                "Agents",
                format!("All {} on {newest}.", agents.len()),
                None,
            )
        } else {
            Finding::new(
                State::Attention,
                "Agents behind the newest",
                format!(
                    "Newest seen is {newest}; {} not on it: {}.",
                    behind.len(),
                    behind
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some("front/agent.php"),
            )
        });
    }

    Tile::new(
        "versions",
        "Versions",
        "Whether a version is actually vulnerable. This reports what is running, not what \
         is wrong with it - and says nothing about PHP, which GLPI does not expose.",
        findings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(
        name: &str,
        active: bool,
        deleted: bool,
        login: Option<&str>,
        pw: Option<&str>,
    ) -> UserRow {
        with_id(1, name, active, deleted, login, pw)
    }

    fn with_id(
        id: i64,
        name: &str,
        active: bool,
        deleted: bool,
        login: Option<&str>,
        pw: Option<&str>,
    ) -> UserRow {
        UserRow {
            id,
            name: name.into(),
            is_active: active,
            is_deleted: deleted,
            last_login: login.map(str::to_string),
            password_changed: pw.map(str::to_string),
        }
    }

    /// Attention outranks Unknown: a known problem is more actionable than an
    /// unmeasured one, and burying it under "unknown" would hide it.
    #[test]
    fn a_tile_reports_its_worst_finding_with_attention_winning() {
        assert_eq!(
            State::worst([State::Ok, State::Unknown, State::Attention]),
            State::Attention
        );
        assert_eq!(State::worst([State::Ok, State::Unknown]), State::Unknown);
        assert_eq!(State::worst([State::Ok, State::Ok]), State::Ok);
    }

    #[test]
    fn write_rights_on_a_read_only_app_are_flagged() {
        let t = access(&AccessInput {
            profile_name: Some("inv"),
            interface: Some("central"),
            write_rights: vec!["computer".into()],
            spare_authorizations: vec![],
        });
        assert_eq!(t.state, State::Attention);
    }

    #[test]
    fn a_read_only_central_profile_is_clean() {
        let t = access(&AccessInput {
            profile_name: Some("inv"),
            interface: Some("central"),
            write_rights: vec![],
            spare_authorizations: vec![],
        });
        assert_eq!(t.state, State::Ok);
    }

    /// A helpdesk profile cannot hold the inventory rights at all.
    #[test]
    fn a_simplified_interface_is_flagged() {
        let t = access(&AccessInput {
            profile_name: Some("x"),
            interface: Some("helpdesk"),
            write_rights: vec![],
            spare_authorizations: vec![],
        });
        assert_eq!(t.state, State::Attention);
    }

    /// Soft-deleted shipped accounts are gone as far as login is concerned,
    /// so they must not be reported as still active.
    #[test]
    fn deleted_shipped_accounts_do_not_count_as_active() {
        let t = accounts(
            &[
                user("glpi", true, false, Some("2026-11-01"), Some("2026-10-30")),
                user("post-only", true, true, None, None),
                user("tech", true, true, None, None),
            ],
            None,
        );
        let shipped = t
            .findings
            .iter()
            .find(|f| f.label.contains("ships with"))
            .unwrap();
        assert!(shipped.detail.contains("glpi"));
        assert!(
            !shipped.detail.contains("post-only"),
            "deleted must be excluded: {}",
            shipped.detail
        );
    }

    #[test]
    fn no_users_is_unknown_rather_than_clean() {
        assert_eq!(accounts(&[], None).state, State::Unknown);
    }

    /// GLPI's internal account cannot sign in, so its never-changed password
    /// is true and meaningless. Reporting it would train the reader to ignore
    /// the tile.
    #[test]
    fn the_system_account_is_excluded() {
        let users = [
            with_id(6, "glpi-system", true, false, None, None),
            with_id(
                7,
                "inv",
                true,
                false,
                Some("2026-11-02"),
                Some("2026-11-01"),
            ),
        ];
        let flagged = accounts(&users, Some(6));
        assert_eq!(flagged.state, State::Ok, "{:?}", flagged.findings);

        // and without the exclusion it WOULD be flagged, so the test is real
        assert_eq!(accounts(&users, None).state, State::Attention);
    }

    #[test]
    fn a_loopback_bind_is_clean_and_a_wider_one_is_not() {
        let mk = |bind| {
            exposure(&ExposureInput {
                bind,
                url_base: Some("http://localhost:8080"),
                enable_api: Some(true),
                enable_api_login_credentials: Some(true),
            })
        };
        assert_eq!(mk("127.0.0.1:3200").state, State::Ok);
        assert_eq!(mk("0.0.0.0:3200").state, State::Attention);
    }

    /// Plain HTTP to loopback is not a finding; plain HTTP across a network is.
    #[test]
    fn http_is_judged_by_where_it_goes() {
        let mk = |url| {
            exposure(&ExposureInput {
                bind: "127.0.0.1:3200",
                url_base: Some(url),
                enable_api: Some(true),
                enable_api_login_credentials: Some(true),
            })
        };
        assert_eq!(mk("http://localhost:8080").state, State::Ok);
        assert_eq!(mk("http://glpi.example.lan").state, State::Attention);
    }

    #[test]
    fn agents_on_different_versions_are_flagged() {
        let t = versions(
            Some("11.0.12"),
            &[
                ("a".into(), Some("1.20".into())),
                ("b".into(), Some("1.19".into())),
            ],
        );
        assert_eq!(t.state, State::Attention);
    }

    #[test]
    fn one_agent_version_everywhere_is_clean() {
        let t = versions(Some("11.0.12"), &[("a".into(), Some("1.20".into()))]);
        assert_eq!(t.state, State::Ok);
    }

    /// An unreported version must not read as "current".
    #[test]
    fn a_missing_glpi_version_is_unknown() {
        assert_eq!(
            versions(None, &[("a".into(), Some("1.20".into()))]).state,
            State::Unknown
        );
    }
}
