//! Computers, as GLPI holds them - the assets agents create.
//!
//! The counterpart to agents.rs. An agent that stopped reporting is one
//! failure; a computer that never had an agent is the other, and GLPI shows
//! both identically: a row in the asset list, looking fine.
use serde::{Deserialize, Serialize};

/// GLPI returns related records as `{"id": 1, "name": "..."}` rather than a
/// bare string. Kept as a struct rather than flattened on parse so the id
/// survives - a later page may want to link to the thing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Named {
    pub id: Option<i64>,
    pub name: Option<String>,
}

/// One computer, with field names mirroring GLPI's schema exactly so this can
/// be diffed against the source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Computer {
    pub id: i64,
    pub name: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,

    /// When inventory last updated this asset. The computer-side equivalent of
    /// an agent's last_contact, and not always the same: an agent can be
    /// reporting while its submissions change nothing here.
    pub last_inventory_update: Option<String>,
    pub last_boot: Option<String>,

    /// True when inventory created this record rather than a human. A
    /// non-dynamic computer was typed in by hand and no agent will ever
    /// update it.
    pub is_dynamic: Option<bool>,
    pub is_deleted: Option<bool>,

    pub manufacturer: Option<Named>,
    pub model: Option<Named>,
    #[serde(rename = "type")]
    pub kind: Option<Named>,
    pub entity: Option<Named>,
}

/// A computer plus the two judgements this app adds.
#[derive(Debug, Clone, Serialize)]
pub struct ComputerStatus {
    #[serde(flatten)]
    pub computer: Computer,

    pub hours_since_inventory: Option<i64>,

    /// No agent is registered against this computer.
    ///
    /// The quiet failure this page exists for. GLPI lists such a machine
    /// beside every other, with no indication that nothing will ever update
    /// it again - it simply stops changing, which looks identical to a
    /// machine whose hardware has not changed.
    pub orphaned: bool,
}

impl Computer {
    /// Hours since inventory last touched this asset.
    ///
    /// Same parsing contract as Agent::hours_since_contact - RFC3339 with an
    /// offset, falling back to a bare naive timestamp read as LOCAL time,
    /// because GLPI's column carries no offset and a different endpoint may
    /// still serve it unformatted.
    pub fn hours_since_inventory(&self) -> Option<i64> {
        let raw = self.last_inventory_update.as_deref()?;

        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            return Some((chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_hours());
        }

        let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").ok()?;
        let local = chrono::Local.from_local_datetime(&naive).single()?;
        Some((chrono::Local::now() - local).num_hours())
    }

    /// `agent_item_ids` is the set of items_id values from agents whose
    /// itemtype is Computer.
    pub fn status(self, agent_item_ids: &std::collections::HashSet<i64>) -> ComputerStatus {
        ComputerStatus {
            orphaned: !agent_item_ids.contains(&self.id),
            hours_since_inventory: self.hours_since_inventory(),
            computer: self,
        }
    }
}

use chrono::TimeZone;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn computer(id: i64, last: Option<&str>) -> Computer {
        Computer {
            id,
            name: Some("nixos".into()),
            serial: Some("7WFPM21819001313".into()),
            uuid: None,
            last_inventory_update: last.map(str::to_string),
            last_boot: None,
            is_dynamic: Some(true),
            is_deleted: Some(false),
            manufacturer: None,
            model: None,
            kind: None,
            entity: None,
        }
    }

    /// The failure this page exists to surface.
    #[test]
    fn a_computer_without_an_agent_is_orphaned() {
        let agents = HashSet::new();
        assert!(computer(1, None).status(&agents).orphaned);
    }

    #[test]
    fn a_computer_with_an_agent_is_not_orphaned() {
        let agents: HashSet<i64> = [1].into_iter().collect();
        assert!(!computer(1, None).status(&agents).orphaned);
    }

    /// Matching is by id, so an agent attached to a different computer must
    /// not clear this one.
    #[test]
    fn another_computers_agent_does_not_count() {
        let agents: HashSet<i64> = [2].into_iter().collect();
        assert!(computer(1, None).status(&agents).orphaned);
    }

    #[test]
    fn inventory_age_parses_the_format_the_live_api_sends() {
        let c = computer(1, Some("2026-11-01T00:03:52+01:00"));
        assert!(c.hours_since_inventory().is_some());
    }

    #[test]
    fn an_unparseable_inventory_date_is_unknown_rather_than_a_panic() {
        assert_eq!(
            computer(1, Some("not a date")).hours_since_inventory(),
            None
        );
    }
}
