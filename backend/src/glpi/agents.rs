//! Agent check-in telemetry.
//!
//! Field names mirror GLPI's `Agent` schema exactly as declared in
//! src/Glpi/Api/HL/Controller/InventoryController.php, so a reader can diff
//! this against the GLPI source. Renaming them to something prettier would
//! make that impossible.
use serde::{Deserialize, Serialize};

/// One registered agent, as GLPI reports it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: i64,

    /// The agent's self-assigned identity, e.g. "nixos-2026-10-31-08-36-00".
    /// This, not the hostname, is what GLPI matches on between runs.
    pub deviceid: Option<String>,

    pub name: Option<String>,

    /// Last time this agent contacted GLPI. The whole point of this app:
    /// an agent that has gone quiet is invisible in GLPI's own asset list,
    /// because the asset it created is still sitting there looking fine.
    pub last_contact: Option<String>,

    /// Agent version, for spotting a fleet drifting apart.
    pub version: Option<String>,

    /// Optional grouping tag, set per client via services.glpi-agent.tag.
    pub tag: Option<String>,

    /// Where the submission came from. Useful for noticing a machine that
    /// moved network, or one reporting through something unexpected.
    pub remote_address: Option<String>,

    /// What the agent is attached to, e.g. "Computer", and its id.
    pub itemtype: Option<String>,
    pub items_id: Option<i64>,
}

/// An agent plus the one judgement this app adds: is it still reporting?
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    #[serde(flatten)]
    pub agent: Agent,

    /// Hours since last contact, or None if it has never reported.
    pub hours_since_contact: Option<i64>,

    /// Stale by the caller's threshold. GLPI itself draws no such line -
    /// it has no notion of an agent being overdue, which is precisely the
    /// gap this app exists to fill.
    pub stale: bool,
}

impl Agent {
    /// Hours since `last_contact`.
    ///
    /// The HL API emits RFC3339 WITH an offset - verified against the live
    /// endpoint, which returned "2026-10-31T08:36:04+01:00". An earlier
    /// version of this parsed "%Y-%m-%d %H:%M:%S" instead, guessed from the
    /// schema's column type, and that fails on every row: the error is
    /// swallowed into None, None is treated as never-contacted, and a
    /// perfectly healthy fleet reports as entirely stale. A wrong format here
    /// does not look like a parse bug, it looks like an outage.
    ///
    /// The naive fallback is kept deliberately. GLPI's own column is a
    /// datetime with no offset, and the legacy /apirest.php API serves it
    /// unformatted, so a different endpoint or version may still hand over a
    /// bare timestamp. That path interprets it as LOCAL time, which is what
    /// the column means on the server.
    pub fn hours_since_contact(&self) -> Option<i64> {
        let raw = self.last_contact.as_deref()?;

        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            return Some((chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_hours());
        }

        let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").ok()?;
        let local = chrono::Local.from_local_datetime(&naive).single()?;
        Some((chrono::Local::now() - local).num_hours())
    }

    pub fn status(self, stale_after_hours: i64) -> AgentStatus {
        let hours = self.hours_since_contact();
        AgentStatus {
            // Never-contacted counts as stale: an agent registered but silent
            // is a worse sign than a late one, not a neutral unknown.
            stale: hours.is_none_or(|h| h >= stale_after_hours),
            hours_since_contact: hours,
            agent: self,
        }
    }
}

use chrono::TimeZone;

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_last_seen(last_contact: Option<&str>) -> Agent {
        Agent {
            id: 1,
            deviceid: Some("nixos-2026-10-31-08-36-00".into()),
            name: Some("nixos".into()),
            last_contact: last_contact.map(str::to_string),
            version: Some("1.20".into()),
            tag: None,
            remote_address: Some("127.0.0.1".into()),
            itemtype: Some("Computer".into()),
            items_id: Some(2),
        }
    }

    /// The format the live HL API actually returns, offset included.
    #[test]
    fn a_contact_just_now_is_fresh_rfc3339() {
        let now = chrono::Local::now().to_rfc3339();
        let status = agent_last_seen(Some(&now)).status(26);
        assert_eq!(status.hours_since_contact, Some(0), "parsed as local time");
        assert!(!status.stale);
    }

    #[test]
    fn an_agent_past_the_threshold_is_stale() {
        let then = (chrono::Local::now() - chrono::Duration::hours(30)).to_rfc3339();
        let status = agent_last_seen(Some(&then)).status(26);
        assert_eq!(status.hours_since_contact, Some(30));
        assert!(status.stale);
    }

    /// 26 and not 24 because the agent timer carries a RandomizedDelaySec: a
    /// daily run that drifts past the hour is healthy, not overdue.
    #[test]
    fn a_daily_run_that_drifted_is_not_stale() {
        let then = (chrono::Local::now() - chrono::Duration::hours(25)).to_rfc3339();
        assert!(!agent_last_seen(Some(&then)).status(26).stale);
    }

    /// Registered but never reporting is a worse sign than late, so it must
    /// not come back as a neutral unknown.
    #[test]
    fn never_contacted_counts_as_stale() {
        let status = agent_last_seen(None).status(26);
        assert_eq!(status.hours_since_contact, None);
        assert!(status.stale);
    }

    /// The literal string the live endpoint returned, so a future format
    /// change in GLPI breaks a test rather than the dashboard.
    #[test]
    fn the_format_the_live_api_actually_sent_parses() {
        let a = agent_last_seen(Some("2026-10-31T08:36:04+01:00"));
        assert!(
            a.hours_since_contact().is_some(),
            "RFC3339 with offset must parse"
        );
    }

    /// The fallback path, for endpoints that serve the bare column.
    #[test]
    fn a_bare_timestamp_still_parses_as_local() {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(agent_last_seen(Some(&now)).hours_since_contact(), Some(0));
    }

    /// A shape GLPI should never send, but if it does the row must degrade to
    /// "unknown" rather than panic the request.
    #[test]
    fn an_unparseable_timestamp_does_not_panic() {
        let status = agent_last_seen(Some("not a date")).status(26);
        assert_eq!(status.hours_since_contact, None);
        assert!(status.stale);
    }
}
