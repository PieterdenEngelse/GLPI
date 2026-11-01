//! Who this app is logged in to GLPI as, and what it may read.
//!
//! Exists because that question cost hours. A 403 from the HL API says only
//! "You don't have permission to perform this action" with a null detail - it
//! never names the missing right - and the account's effective profile is not
//! visible anywhere in GLPI's own UI without clicking through three screens.
//!
//! GLPI activates exactly ONE profile per session (Session::init), chosen by
//! the user's default profile, so holding several authorizations does not
//! grant their union. That is the detail this surfaces.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub profile_name: Option<String>,
    pub profile_interface: Option<String>,
    pub entity_name: Option<String>,

    /// Whether the active entity selection includes child entities. Without
    /// it, agents in sub-entities are absent with no trace in any response.
    pub entity_recursive: Option<bool>,

    /// The two rights this app actually needs, as GLPI reports them. 0 means
    /// absent; READ is bit 1.
    pub right_agent: Option<i64>,
    pub right_entity: Option<i64>,
}

impl SessionInfo {
    /// Pull the handful of fields worth showing out of GLPI's session blob,
    /// which is large and mostly irrelevant here.
    pub fn from_value(v: &serde_json::Value) -> Self {
        let profile = v.get("active_profile");
        let entity = v.get("active_entity");
        let rights = profile.and_then(|p| p.get("rights"));
        let right = |name: &str| rights.and_then(|r| r.get(name)).and_then(|x| x.as_i64());

        Self {
            profile_name: profile
                .and_then(|p| p.get("name"))
                .and_then(|x| x.as_str())
                .map(str::to_string),
            profile_interface: profile
                .and_then(|p| p.get("interface"))
                .and_then(|x| x.as_str())
                .map(str::to_string),
            entity_name: entity
                .and_then(|e| e.get("complete_name").or_else(|| e.get("short_name")))
                .and_then(|x| x.as_str())
                .map(str::to_string),
            // GLPI sends 1/0 here rather than a JSON bool.
            entity_recursive: entity.and_then(|e| e.get("recursive")).map(|x| {
                x.as_bool()
                    .unwrap_or_else(|| x.as_i64().map(|n| n != 0).unwrap_or(false))
            }),
            right_agent: right("agent"),
            right_entity: right("entity"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shape_the_live_api_returns() {
        let v = serde_json::json!({
            "active_profile": {
                "name": "inv",
                "interface": "central",
                "rights": { "agent": 1, "entity": 33 }
            },
            "active_entity": { "complete_name": "Root entity", "recursive": 1 }
        });
        let s = SessionInfo::from_value(&v);
        assert_eq!(s.profile_name.as_deref(), Some("inv"));
        assert_eq!(s.right_agent, Some(1));
        // recursive arrives as 1, not true
        assert_eq!(s.entity_recursive, Some(true));
    }

    /// A GLPI that changes this blob must degrade to blanks, never panic.
    #[test]
    fn a_missing_session_shape_yields_nothing_rather_than_panicking() {
        let s = SessionInfo::from_value(&serde_json::json!({}));
        assert!(s.profile_name.is_none() && s.right_agent.is_none());
    }
}
