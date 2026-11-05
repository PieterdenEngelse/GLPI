//! Financial and administrative dates for one asset.
//!
//! CORRECTING SOMETHING THIS PROJECT WROTE DOWN WRONG. The 12.0.1 upgrade
//! notes described "four generic `{itemtype}/{id}/Infocom` routes" as new
//! capability in HL v3.0. That is misleading, and the mistake is recorded in
//! `~/docs/gl-upgrade.md` and in commit 77c82a0's message.
//!
//! What is actually true, read from the 12.0.1 release:
//!
//!   * The four routes introduced at 3.0 are `ManagementController`'s, and
//!     their itemtype is restricted to `getManagementInfocomTypes30()` -
//!     `DatabaseInstance`, `Domain`, `License`, `Line`. No assets at all.
//!   * `AssetController` has carried `/{itemtype}/{id}/Infocom` since
//!     **`RouteVersion(introduced: '2.0')`**, and `getAssetInfocomTypes()`
//!     includes `Computer`, `Monitor`, `NetworkEquipment`, `Printer`,
//!     `Software` and the rest.
//!
//! So Infocom for the assets this app cares about was reachable on 11.0.12,
//! before the upgrade, on v2.3. It is not a v3 feature and the upgrade did not
//! unlock it. That is why `fetch` below uses the default version rather than
//! `HL_V3`.
//!
//! WHY 404 IS NOT AN ERROR HERE. `AssetController::getItemInfocom` ends:
//!
//! ```php
//! if (empty($results)) {
//!     return self::getNotFoundErrorResponse();
//! }
//! ```
//!
//! So an asset with no financial record answers 404 while the route is present
//! and working - which is the normal state of a machine created by inventory
//! rather than by a purchase. Measured: `Assets/Computer/1/Infocom` on this
//! install answers 404, and the computer exists. A missing `infocom` right is
//! a different branch of the same handler and answers access-denied, so `None`
//! never hides a permission problem.
//!
//! WHY THERE IS NO FLEET-WIDE SWEEP. One request per asset. CLAUDE.md's rule
//! against exactly this shape - "a fleet-wide view would cost ~10 requests per
//! machine" - is why the inventory pages read v1 collections instead. Infocom
//! has no collection route under Assets, so a fleet view would be one request
//! per machine with no way to batch it. Offered per-asset only, deliberately.
use anyhow::Result;
use serde::Serialize;

use super::Client;

/// The dates GLPI tracks against an asset's purchase and lifecycle.
///
/// Every field optional, as everywhere else here: a field GLPI stops sending
/// should leave a blank cell rather than fail the parse. Names mirror GLPI's
/// schema exactly, per CLAUDE.md, so this can be diffed against the source.
#[derive(Debug, Clone, Serialize)]
pub struct Infocom {
    pub id: Option<i64>,
    pub itemtype: Option<String>,
    pub items_id: Option<i64>,
    pub comment: Option<String>,

    pub date_buy: Option<String>,
    pub date_use: Option<String>,
    pub date_order: Option<String>,
    pub date_delivery: Option<String>,
    pub date_inventory: Option<String>,
    pub date_warranty: Option<String>,
    pub date_decommission: Option<String>,
}

/// What came back for one asset.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub itemtype: String,
    pub items_id: i64,
    /// `None` means GLPI has no financial record for this asset - the ordinary
    /// case for a machine that arrived through inventory. Distinct from a
    /// failed call, which surfaces as an error instead.
    pub infocom: Option<Infocom>,
    pub cannot_check: &'static str,
}

const CANNOT_CHECK: &str = "Per asset, one request each: Infocom has no \
collection route under Assets, so there is no fleet-wide view that would not \
cost a request per machine. An absent record is reported as absent rather \
than as an error, because an asset created by inventory has no purchase to \
record - that is normal and not a gap.";

/// Decode one Infocom object.
pub fn parse(itemtype: &str, items_id: i64, body: Option<&serde_json::Value>) -> Report {
    let infocom = body.map(|v| {
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .filter(|x| !x.is_empty())
                .map(str::to_string)
        };
        Infocom {
            id: v.get("id").and_then(|x| x.as_i64()),
            itemtype: s("itemtype"),
            items_id: v.get("items_id").and_then(|x| x.as_i64()),
            comment: s("comment"),
            date_buy: s("date_buy"),
            date_use: s("date_use"),
            date_order: s("date_order"),
            date_delivery: s("date_delivery"),
            date_inventory: s("date_inventory"),
            date_warranty: s("date_warranty"),
            date_decommission: s("date_decommission"),
        }
    });

    Report {
        itemtype: itemtype.to_string(),
        items_id,
        infocom,
        cannot_check: CANNOT_CHECK,
    }
}

/// Fetch one asset's Infocom, if it has one.
///
/// The default HL version, not v3: see the module note - this route has
/// existed since API 2.0 and the v3 Infocom routes do not accept assets.
pub async fn fetch(client: &Client, itemtype: &str, items_id: i64) -> Result<Report> {
    let path = format!("Assets/{itemtype}/{items_id}/Infocom");
    let body = client.get_optional_at(super::HL_DEFAULT, &path).await?;
    Ok(parse(itemtype, items_id, body.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_is_decoded() {
        let body = serde_json::json!({
            "id": 4,
            "itemtype": "Computer",
            "items_id": 1,
            "date_buy": "2026-01-15",
            "date_warranty": "2029-01-15",
            "comment": "",
            "date_decommission": null
        });
        let r = parse("Computer", 1, Some(&body));

        let i = r.infocom.expect("a record was supplied");
        assert_eq!(i.id, Some(4));
        assert_eq!(i.date_buy.as_deref(), Some("2026-01-15"));
        assert_eq!(i.date_warranty.as_deref(), Some("2029-01-15"));
        // An empty string is None, so the UI shows a blank rather than "".
        assert_eq!(i.comment, None);
        assert_eq!(i.date_decommission, None);
    }

    /// The ordinary case on an inventory-built fleet, and the distinction this
    /// module exists to preserve: no record is not an error.
    #[test]
    fn no_record_is_absence_not_failure() {
        let r = parse("Computer", 1, None);
        assert!(r.infocom.is_none());
        assert_eq!(r.itemtype, "Computer");
        assert_eq!(r.items_id, 1);
    }

    #[test]
    fn the_asset_is_identified_even_when_glpi_omits_the_back_reference() {
        // GLPI's response need not echo itemtype/items_id; the report carries
        // what was asked for regardless, so a row is never anonymous.
        let r = parse("Monitor", 7, Some(&serde_json::json!({ "id": 9 })));
        assert_eq!(r.itemtype, "Monitor");
        assert_eq!(r.items_id, 7);
        assert_eq!(r.infocom.unwrap().itemtype, None);
    }
}
