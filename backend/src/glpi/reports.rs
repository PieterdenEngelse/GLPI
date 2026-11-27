//! Report definitions, read and written in GLPI's `glpi_plugin_swcat_reports`.
//!
//! A GLPI report is normally a PHP file. The `swcat` plugin turns it into a
//! row instead: one parameterised renderer serves every definition, and the
//! plugin rebuilds its Tools > Reports menu from this table on every request.
//! So a row written here becomes a report in GLPI immediately - no plugin
//! file, and on this host no `nixos-rebuild`, because nothing new has to land
//! in /nix/store.
//!
//! **Why writing here is allowed.** `CLAUDE.md` used to say this app reads and
//! must keep reading; that blanket rule was lifted on 2026-11-26, and the
//! distinction that replaced it is the one this module sits on. The hazard was
//! never "the `glpi` schema" - it was writing into tables GLPI *manages*, where
//! an UPDATE skips the history, entity scoping and derived state GLPI
//! maintains for its own data. A plugin's own `glpi_plugin_*` table has none of
//! those: GLPI neither logs it nor derives anything from it, so there is
//! nothing behind this app's back to corrupt. Writes into GLPI's own tables
//! still go through the API or GLPI's code, never a query.
//!
//! Gated on `GLPI_INV_REPORT_WRITES` regardless, like every other write path
//! here, and for the reason `config.rs` records: one flag per capability,
//! because reusing one silently widened a write's scope once before.
use anyhow::{Context, Result};
use mysql_async::prelude::*;
use serde::{Deserialize, Serialize};

use super::refused::Db;

/// Guards against a definition set growing until `plugin_init_swcat()` - which
/// reads this table on EVERY GLPI request - stops being a cheap lookup.
const MAX_DEFINITIONS: usize = 200;

/// One saved report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportDef {
    pub id: i64,
    pub name: String,
    /// CI classes to include. `None` means every type the field exists on.
    pub itemtypes: Option<Vec<String>>,
    /// The dropdown foreign key to group by, e.g. `locations_id`.
    pub group_by: String,
    /// Aggregate a tree to its root. Ignored by the renderer, with a notice,
    /// when the field is not a tree.
    pub rollup: bool,
    pub is_active: bool,
}

/// What the builder submits. No `id`: creating and updating are separate.
#[derive(Debug, Clone, Deserialize)]
pub struct NewReport {
    pub name: String,
    #[serde(default)]
    pub itemtypes: Option<Vec<String>>,
    pub group_by: String,
    #[serde(default)]
    pub rollup: bool,
}

/// Why a definition was refused, in words the UI can show as-is.
#[derive(Debug)]
pub enum Invalid {
    EmptyName,
    BadGroupBy,
    TooMany,
}

impl Invalid {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyName => "A report needs a name: it is what appears in GLPI's \
                                Tools > Reports menu."
                .into(),
            Self::BadGroupBy => "The group-by field must look like a GLPI foreign key - \
                                 lowercase, ending in `_id`, for example `locations_id` \
                                 or `states_id`."
                .into(),
            Self::TooMany => format!(
                "There are already {MAX_DEFINITIONS} definitions. The plugin reads this \
                 table on every GLPI page load, so the list is capped deliberately."
            ),
        }
    }
}

/// Shape check only.
///
/// Deliberately NOT a check that the field exists: the plugin resolves
/// `group_by` at render time and already reports an unknown field clearly, and
/// duplicating that here would be a second copy of GLPI's own table-name rules
/// that could disagree with the first. This rejects what cannot be a foreign
/// key at all, which is enough to keep nonsense out of the table.
fn valid_group_by(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.ends_with("_id")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Db {
    /// Every definition, newest last, for the builder's list.
    pub async fn report_defs(&self) -> Result<Vec<ReportDef>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to GLPI's database")?;

        // Absent until the swcat plugin has been installed. An empty list is
        // the honest answer then - the builder says so rather than erroring,
        // because "no reports yet" and "plugin not installed" look the same
        // to someone who has not opened GLPI.
        let exists: Option<String> = conn
            .query_first("SHOW TABLES LIKE 'glpi_plugin_swcat_reports'")
            .await
            .context("looking for glpi_plugin_swcat_reports")?;
        if exists.is_none() {
            return Ok(Vec::new());
        }

        let rows: Vec<(i64, String, Option<String>, String, i8, i8)> = conn
            .query(
                "SELECT id, name, itemtypes, group_by, rollup, is_active \
                 FROM glpi_plugin_swcat_reports \
                 ORDER BY name",
            )
            .await
            .context("reading report definitions")?;

        Ok(rows
            .into_iter()
            .map(
                |(id, name, itemtypes, group_by, rollup, is_active)| ReportDef {
                    id,
                    // Invalid JSON is treated as "no restriction", matching the
                    // renderer. A definition is not worth hiding over a field
                    // the report itself would shrug at.
                    itemtypes: itemtypes
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                        .filter(|v| !v.is_empty()),
                    name,
                    group_by,
                    rollup: rollup != 0,
                    is_active: is_active != 0,
                },
            )
            .collect())
    }

    /// Save a new definition. Returns its id.
    pub async fn create_report_def(&self, def: &NewReport) -> Result<Result<i64, Invalid>> {
        if def.name.trim().is_empty() {
            return Ok(Err(Invalid::EmptyName));
        }
        if !valid_group_by(&def.group_by) {
            return Ok(Err(Invalid::BadGroupBy));
        }

        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to GLPI's database")?;

        let count: Option<i64> = conn
            .query_first("SELECT COUNT(*) FROM glpi_plugin_swcat_reports")
            .await
            .context("counting report definitions")?;
        if count.unwrap_or(0) as usize >= MAX_DEFINITIONS {
            return Ok(Err(Invalid::TooMany));
        }

        // Bound parameters, not formatting: `name` is free text straight from
        // the builder. The ids elsewhere in this crate are formatted because
        // they are i64 from GLPI's own API; this is not that.
        let itemtypes = def
            .itemtypes
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(serde_json::to_string)
            .transpose()
            .context("encoding itemtypes")?;

        conn.exec_drop(
            "INSERT INTO glpi_plugin_swcat_reports \
                 (name, itemtypes, group_by, rollup, is_active, date_creation, date_mod) \
             VALUES (:name, :itemtypes, :group_by, :rollup, 1, NOW(), NOW())",
            params! {
                "name" => def.name.trim(),
                "itemtypes" => itemtypes,
                "group_by" => &def.group_by,
                "rollup" => i8::from(def.rollup),
            },
        )
        .await
        .context("inserting report definition")?;

        let id = conn.last_insert_id().unwrap_or_default() as i64;
        Ok(Ok(id))
    }

    /// Delete one definition. `false` when no such row.
    ///
    /// A real DELETE rather than clearing `is_active`: GLPI keeps no history
    /// of this table, so a soft delete would leave a row nothing displays and
    /// nothing can explain.
    pub async fn delete_report_def(&self, id: i64) -> Result<bool> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("connecting to GLPI's database")?;

        conn.exec_drop(
            "DELETE FROM glpi_plugin_swcat_reports WHERE id = :id",
            params! { "id" => id },
        )
        .await
        .context("deleting report definition")?;

        Ok(conn.affected_rows() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::valid_group_by;

    #[test]
    fn accepts_real_foreign_keys() {
        for s in [
            "locations_id",
            "states_id",
            "manufacturers_id",
            "computertypes_id",
        ] {
            assert!(valid_group_by(s), "{s} should be accepted");
        }
    }

    #[test]
    fn rejects_what_cannot_be_one() {
        // Trailing `_id` is the cheap half; the rest is what keeps a quoted
        // string or a path out of a column name.
        for s in [
            "",
            "locations",
            "Locations_id",
            "locations id",
            "locations_id; DROP TABLE x",
            "../etc/passwd_id",
        ] {
            assert!(!valid_group_by(s), "{s:?} should be rejected");
        }
    }
}
