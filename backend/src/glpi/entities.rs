//! The entities this account is allowed to read.
//!
//! Not decoration. GLPI appends an entity restriction to every collection
//! query - Search.php:570 in 11.0.12:
//!
//!   $criteria['WHERE'][] = ['AND' => $entity_restrict];
//!
//! so every list this app shows is already filtered, and says nothing about
//! what lies outside the filter. An agent in an entity this account cannot
//! reach is simply absent, and absent is indistinguishable from
//! decommissioned.
//!
//! This is the one gap no amount of backend code closes. The 206 guard works
//! because GLPI reports the total it withheld; entity-filtered rows never
//! enter that total, so there is no number to compare against. All that can
//! be done is to state the scope, where a reader who knows the estate will
//! notice when it is too narrow.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: i64,
    pub name: Option<String>,

    /// GLPI's full path, e.g. "Root entity > Site A". Preferred over `name`
    /// for display: once entities nest, two sites can each hold a
    /// "Workstations" and the bare name stops identifying anything.
    pub completename: Option<String>,
}
