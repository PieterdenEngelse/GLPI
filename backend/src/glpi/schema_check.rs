//! Field-level rights suppression: the API's own quiet failure mode.
//!
//! WHY THIS EXISTS. `gl-upgrade.md` section 6 used to say the database schema
//! was the only coupling that fails quietly, and contrasted it with the API:
//! "an API that drops a field names the field; SQL does not." That contrast
//! holds for GLPI 11.0.12 and is FALSE for 12.0.1.
//!
//! GLPI's HL schemas carry `x-rights-conditions`, and they degrade rather than
//! refuse. In two shapes:
//!
//!   * object-level, filtering ROWS - `User` joins `glpi_profiles_users` and
//!     adds a WHERE on the session's entities when the caller cannot view all
//!     entities. Present in both versions.
//!   * field-level, dropping COLUMNS - a field the caller may not read is
//!     simply absent from the response. NEW in 12.0.1: in
//!     AdministrationController, `x-rights-conditions` goes from 4 to 16, with
//!     `User::READAUTHENT` and `Notification::canView()` appearing for the
//!     first time.
//!
//! An absent field is indistinguishable from a null one, because every field
//! on the frontend is `Option` by design so a drifting backend degrades to a
//! blank cell rather than a failed parse. That design is right, and it is
//! exactly what makes this invisible: a suppressed field renders as an empty
//! cell that looks like missing data in GLPI rather than missing permission
//! here.
//!
//! WHAT MAKES IT CHECKABLE. GLPI serves its own OpenAPI document at
//! `/api.php/v2.3/doc.json` - `security_level: SECURITY_NONE`, introduced at
//! API 2.0, so it is available on the deployed 11.0.12 as well. The document
//! is generated from the schema declarations and is NOT filtered by the
//! caller's rights, while the response IS. The difference between them is the
//! suppression, and it can be named.
//!
//! Measured against a 12.0.1 instance with this app's own service account:
//!
//!   Administration/Entity   37 declared, 37 received  ->  nothing suppressed
//!   Administration/User     32 declared, 29 received  ->  date_sync, and
//!                                                         password/password2
//!
//! `date_sync` is the real finding: it carries a `read` condition requiring
//! `User::READAUTHENT` (2048) and the account holds `user: 1`. The two
//! password fields are `writeOnly` and are never returned to anyone, so they
//! are excluded rather than reported - see `readable_fields`.
use serde::Serialize;

use super::posture::State;

/// Why a declared field did not arrive, or an arrived field was not declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// Declared and readable, but absent from the response. The caller is
    /// almost certainly missing the right its `x-rights-conditions` wants.
    Suppressed,
    /// Present in the response but not in the schema document. The app's
    /// understanding of this endpoint is behind the server's - the signal an
    /// upgrade leaves.
    Undeclared,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldGap {
    pub field: String,
    pub kind: GapKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaReport {
    /// The schema's name in GLPI's OpenAPI document, e.g. "User".
    pub schema: String,
    pub state: State,
    /// Every property the document declares, including write-only ones.
    pub declared: usize,
    /// Those a reader could expect: `declared` minus write-only.
    pub readable: usize,
    /// Distinct keys seen across the sampled rows.
    pub received: usize,
    pub gaps: Vec<FieldGap>,
    /// Write-only fields, excluded from the comparison and listed so the
    /// exclusion is visible rather than implicit.
    pub write_only: Vec<String>,
    /// What this check cannot establish, stated with the result for the same
    /// reason `posture::Tile` does it: a reader's trust should match the
    /// check's actual reach.
    pub cannot_check: &'static str,
}

/// What `SchemaReport::cannot_check` always says. Pulled out so the three
/// construction sites cannot drift apart.
const CANNOT_CHECK: &str = "Top-level fields only - a suppressed key inside a \
nested object is not seen. Cannot distinguish a field suppressed by rights \
from one GLPI stopped sending, since both are simply absent; it reports that \
the schema promised a field the response did not carry, not why.";

/// The properties of an OpenAPI schema object that a reader may expect back.
///
/// `writeOnly` is the whole subtlety. `User.password` and `User.password2` are
/// declared and are never returned to anybody, so counting them as suppressed
/// would report a permanent false positive on every call. There are only ten
/// such properties across all 330 schemas in 12.0.1's document, so excluding
/// them is cheap and precise rather than a broad heuristic.
fn readable_fields(schema: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return (Vec::new(), Vec::new());
    };

    let mut readable = Vec::new();
    let mut write_only = Vec::new();
    for (name, spec) in props {
        if spec.get("writeOnly").and_then(serde_json::Value::as_bool) == Some(true) {
            write_only.push(name.clone());
        } else {
            readable.push(name.clone());
        }
    }
    readable.sort();
    write_only.sort();
    (readable, write_only)
}

/// Every distinct key across the sampled rows.
///
/// The union across rows, not the keys of the first one. GLPI omits a field
/// per row where it is null in some serialisations, so the first row alone
/// would manufacture suppression findings for fields that simply happened to
/// be empty on that record.
fn received_fields(received: &serde_json::Value) -> Vec<String> {
    let rows: Vec<&serde_json::Value> = match received {
        serde_json::Value::Array(rows) => rows.iter().collect(),
        other => vec![other],
    };

    let mut keys: Vec<String> = rows
        .iter()
        .filter_map(|row| row.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Compare what the schema document promises against what a response carried.
///
/// Pure, over values the caller has already fetched, so the judgement is
/// testable without a GLPI - the same split `posture` uses.
pub fn compare(
    schema_name: &str,
    declared_schema: &serde_json::Value,
    received: &serde_json::Value,
) -> SchemaReport {
    let (readable, write_only) = readable_fields(declared_schema);
    let got = received_fields(received);

    // Nothing to compare against is NOT a pass.
    //
    // An endpoint that returned no rows, or a schema this document does not
    // describe, tells us nothing about suppression - and `Unknown` is how this
    // app refuses to let "could not tell" read as "fine". Without this an
    // empty collection would report a clean bill of health for every field.
    if readable.is_empty() || got.is_empty() {
        return SchemaReport {
            schema: schema_name.to_string(),
            state: State::Unknown,
            declared: readable.len() + write_only.len(),
            readable: readable.len(),
            received: got.len(),
            gaps: Vec::new(),
            write_only,
            cannot_check: CANNOT_CHECK,
        };
    }

    let mut gaps: Vec<FieldGap> = readable
        .iter()
        .filter(|f| !got.contains(f))
        .map(|f| FieldGap {
            field: f.clone(),
            kind: GapKind::Suppressed,
        })
        .collect();

    gaps.extend(
        got.iter()
            .filter(|f| !readable.contains(f) && !write_only.contains(f))
            .map(|f| FieldGap {
                field: f.clone(),
                kind: GapKind::Undeclared,
            }),
    );

    SchemaReport {
        schema: schema_name.to_string(),
        state: if gaps.is_empty() {
            State::Ok
        } else {
            State::Attention
        },
        declared: readable.len() + write_only.len(),
        readable: readable.len(),
        received: got.len(),
        gaps,
        write_only,
        cannot_check: CANNOT_CHECK,
    }
}

/// The v2.3 paths this app reads, each paired with the schema it returns.
///
/// Spelled out rather than derived. A path does not imply its schema name -
/// `Administration/User` returns `User`, `Inventory/Agent` returns `Agent` -
/// and guessing wrong would silently report a whole endpoint as `Unknown`,
/// which is the one outcome this module exists to avoid.
pub const AUDITED: &[(&str, &str)] = &[
    ("Inventory/Agent", "Agent"),
    ("Assets/Computer", "Computer"),
    ("Administration/Entity", "Entity"),
    ("Administration/User", "User"),
    ("session", "Session"),
];

/// One schema in GLPI's document, counted rather than compared.
///
/// The catalogue exists because `AUDITED` is five paths and the document
/// declares hundreds: without it, a reader has no way to see how small the
/// audited slice is, and "five endpoints are clean" reads as "the API is
/// clean".
#[derive(Debug, Clone, Serialize)]
pub struct SchemaSummary {
    pub schema: String,
    /// Every declared property, write-only ones included.
    pub declared: usize,
    /// Those a reader could expect back: `declared` minus write-only.
    pub readable: usize,
    pub write_only: usize,
    /// Whether this schema is one `AUDITED` compares against a real response.
    pub audited: bool,
}

/// Every schema the document declares, with field counts and nothing more.
///
/// Deliberately no `received` column. Getting one would cost a request per
/// schema - hundreds of them against one install - which is the whole reason
/// `AUDITED` names five paths instead of walking the document. A count that
/// cannot be had cheaply is left out rather than faked from the first page of
/// a collection.
///
/// Sorted by name, because the audited rows are already listed in full
/// alongside this and the only thing a reader does with hundreds of names is
/// look one up.
pub fn catalogue(doc: &serde_json::Value) -> Vec<SchemaSummary> {
    let audited: std::collections::HashSet<&str> =
        AUDITED.iter().map(|(_, schema)| *schema).collect();

    let Some(schemas) = doc
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|s| s.as_object())
    else {
        return Vec::new();
    };

    let mut out: Vec<SchemaSummary> = schemas
        .iter()
        .map(|(name, spec)| {
            let (readable, write_only) = readable_fields(spec);
            SchemaSummary {
                schema: name.clone(),
                declared: readable.len() + write_only.len(),
                readable: readable.len(),
                write_only: write_only.len(),
                audited: audited.contains(name.as_str()),
            }
        })
        .collect();
    out.sort_by(|a, b| a.schema.cmp(&b.schema));
    out
}

/// Pull one schema's declaration out of a whole OpenAPI document.
///
/// Returns `None` rather than an error for an unknown name: a schema the
/// document does not describe is a legitimate finding for `compare` to render
/// as `Unknown`, not a failure of the fetch.
pub fn schema_from_doc<'a>(
    doc: &'a serde_json::Value,
    schema_name: &str,
) -> Option<&'a serde_json::Value> {
    doc.get("components")?.get("schemas")?.get(schema_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `User` case, reduced: `date_sync` is declared, readable and
    /// absent because the account lacks READAUTHENT; the two password fields
    /// are declared, write-only and absent from every response by design.
    fn user_schema() -> serde_json::Value {
        serde_json::json!({
            "properties": {
                "id":        { "type": "integer", "readOnly": true },
                "name":      { "type": "string" },
                "date_sync": { "type": "string", "readOnly": true },
                "password":  { "type": "string", "writeOnly": true },
                "password2": { "type": "string", "writeOnly": true },
            }
        })
    }

    #[test]
    fn write_only_fields_are_not_reported_as_suppressed() {
        let received = serde_json::json!([{ "id": 7, "name": "inv", "date_sync": null }]);
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Ok);
        assert!(r.gaps.is_empty(), "unexpected gaps: {:?}", r.gaps);
        assert_eq!(r.write_only, vec!["password", "password2"]);
        assert_eq!(r.declared, 5);
        assert_eq!(r.readable, 3);
    }

    #[test]
    fn a_rights_suppressed_field_is_named() {
        // What the service account actually gets: no date_sync.
        let received = serde_json::json!([{ "id": 7, "name": "inv" }]);
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Attention);
        assert_eq!(r.gaps.len(), 1);
        assert_eq!(r.gaps[0].field, "date_sync");
        assert_eq!(r.gaps[0].kind, GapKind::Suppressed);
    }

    #[test]
    fn a_field_the_schema_does_not_declare_is_named_as_drift() {
        let received = serde_json::json!([
            { "id": 7, "name": "inv", "date_sync": null, "invented_by_upstream": 1 }
        ]);
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Attention);
        assert_eq!(r.gaps.len(), 1);
        assert_eq!(r.gaps[0].field, "invented_by_upstream");
        assert_eq!(r.gaps[0].kind, GapKind::Undeclared);
    }

    /// The rule the rest of this app applies to an empty list versus a failed
    /// fetch, applied here: no rows means no knowledge, not a clean result.
    #[test]
    fn an_empty_collection_is_unknown_and_not_ok() {
        let received = serde_json::json!([]);
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Unknown);
        assert!(r.gaps.is_empty());
        assert_eq!(r.received, 0);
    }

    #[test]
    fn a_schema_the_document_does_not_describe_is_unknown() {
        let received = serde_json::json!([{ "id": 1 }]);
        let r = compare("Nonexistent", &serde_json::json!({}), &received);

        assert_eq!(r.state, State::Unknown);
        assert_eq!(r.readable, 0);
    }

    /// Fields are unioned across rows. GLPI can omit a null field on one
    /// record and send it on another; keying off row zero alone would invent
    /// a suppression finding for a field that is merely empty there.
    #[test]
    fn fields_are_unioned_across_rows_not_taken_from_the_first() {
        let received = serde_json::json!([
            { "id": 7, "name": "inv" },
            { "id": 8, "name": "other", "date_sync": "2026-11-04 10:00:00" },
        ]);
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Ok, "gaps: {:?}", r.gaps);
        assert_eq!(r.received, 3);
    }

    /// A single object, not a collection - `/session` shape.
    #[test]
    fn a_bare_object_is_compared_like_a_one_row_collection() {
        let received = serde_json::json!({ "id": 7, "name": "inv" });
        let r = compare("User", &user_schema(), &received);

        assert_eq!(r.state, State::Attention);
        assert_eq!(r.gaps.len(), 1);
        assert_eq!(r.gaps[0].field, "date_sync");
    }

    #[test]
    fn schema_is_looked_up_under_components() {
        let doc = serde_json::json!({
            "components": { "schemas": { "User": { "properties": { "id": {} } } } }
        });
        assert!(schema_from_doc(&doc, "User").is_some());
        assert!(schema_from_doc(&doc, "Missing").is_none());
        assert!(schema_from_doc(&serde_json::json!({}), "User").is_none());
    }
    #[test]
    fn the_catalogue_counts_every_schema_and_marks_the_audited_ones() {
        let doc = serde_json::json!({
            "components": { "schemas": {
                "Zebra": { "properties": { "id": {} } },
                "User":  { "properties": {
                    "id": {},
                    "password": { "writeOnly": true },
                } },
                // A schema with no properties at all - an enum, say. It must
                // count as zero rather than be dropped, or the catalogue total
                // disagrees with the document.
                "Status": { "type": "string" },
            } }
        });

        let got = catalogue(&doc);

        // Sorted by name, not by document order.
        assert_eq!(
            got.iter().map(|s| s.schema.as_str()).collect::<Vec<_>>(),
            ["Status", "User", "Zebra"]
        );

        let user = &got[1];
        assert_eq!(user.declared, 2, "write-only fields are still declared");
        assert_eq!(user.readable, 1);
        assert_eq!(user.write_only, 1);
        assert!(user.audited, "User is in AUDITED");

        assert_eq!(
            got[0].declared, 0,
            "a schema with no properties counts zero"
        );
        assert!(!got[2].audited, "Zebra is not audited");
    }

    #[test]
    fn a_document_without_schemas_yields_an_empty_catalogue() {
        // Not a panic and not one bogus row: the caller already reports
        // doc_available: false for this case.
        assert!(catalogue(&serde_json::json!({})).is_empty());
        assert!(catalogue(&serde_json::json!({ "components": {} })).is_empty());
    }
}
