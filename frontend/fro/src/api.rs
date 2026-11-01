//! Fetch helpers for every backend endpoint, as in RERAG's api.rs.
//!
//! Components never call reqwest themselves. One module knows the paths and
//! the response shapes, so a backend change lands in one file rather than in
//! whichever component happened to be fetching.
use serde::Deserialize;

/// Mirrors the backend's AgentStatus.
///
/// Deliberately duplicated rather than shared through a fourth crate: the
/// layout here follows RERAG, which has no shared crate. The cost is that
/// this struct and backend/src/glpi/agents.rs must be changed together -
/// every field is Option so a backend that adds or drops one degrades to a
/// blank cell instead of a failed parse.
// PartialEq is required, not cosmetic: Dioxus 0.7 memoizes components by
// comparing props, so a prop type without it fails to compile with
// "binary operation `==` cannot be applied" pointing at the component
// rather than at the struct.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AgentStatus {
    pub name: Option<String>,
    pub deviceid: Option<String>,
    pub last_contact: Option<String>,
    pub version: Option<String>,
    pub tag: Option<String>,
    pub remote_address: Option<String>,
    pub hours_since_contact: Option<i64>,
    pub stale: bool,
}

/// Absolute URL for a backend path, resolved against the page's own origin.
///
/// reqwest compiled to wasm32 goes through the browser's fetch, but it still
/// parses the URL with `url::Url` first, and that REJECTS a relative path -
/// there is no base. The failure surfaces as the single phrase "builder
/// error", with no mention of the URL, so it reads like a misconfigured
/// client rather than a malformed address.
///
/// The backend serves this bundle, so its origin is always the right base and
/// nothing needs configuring. Same origin also means no CORS. RERAG's api.rs
/// guesses a port here because its frontend can run under `dx serve` on an
/// origin the backend does not own; this app has no such split, and inventing
/// one would add a way to misconfigure it.
pub fn api_url(path: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    format!("{origin}{path}")
}

/// Every registered agent with this app's staleness judgement, most overdue
/// first - the backend sorts, because the reason to open the page is to find
/// what stopped reporting.
///
/// The error is a String because the only thing the UI does with it is show
/// it. It carries GLPI's own message through the backend's 502, which is the
/// difference between "GLPI refused the token" and "no agents exist".
pub async fn fetch_agents() -> Result<Vec<AgentStatus>, String> {
    reqwest::get(api_url("/api/agents"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<AgentStatus>>()
        .await
        .map_err(|e| e.to_string())
}

/// One entity this account may read. Mirrors the backend's Entity.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Entity {
    pub id: i64,
    pub name: Option<String>,
    pub completename: Option<String>,
}

impl Entity {
    /// What to show for this entity.
    ///
    /// The full path first: once entities nest, two sites can each hold a
    /// "Workstations" and the bare name identifies neither. Falling back to
    /// the id rather than an empty string keeps a malformed row visible - a
    /// scope line with a blank in it is worse than one admitting "entity 7".
    pub fn label(&self) -> String {
        self.completename
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| format!("entity {}", self.id))
    }
}

/// The entities this account can read. Everything else the app shows is
/// filtered to these, invisibly - see components/scope_note.rs.
pub async fn fetch_entities() -> Result<Vec<Entity>, String> {
    reqwest::get(api_url("/api/entities"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Entity>>()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Health {
    pub status: String,
    pub version: String,
    /// Where GLPI itself lives, so info panels can link into its screens.
    pub glpi_url: String,
}

/// Liveness, plus the GLPI address.
///
/// The status dot and the error bar only care whether this succeeded; the
/// body matters just enough to build links, and every field is read through
/// serde so a shape change degrades rather than being treated as "down".
pub async fn fetch_health() -> Result<Health, String> {
    reqwest::get(api_url("/api/health"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Health>()
        .await
        .map_err(|e| e.to_string())
}

/// GLPI returns related records as objects, not strings.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Named {
    pub name: Option<String>,
}

impl Named {
    /// The name, or an empty string. Used in table cells, where a missing
    /// relation should leave a blank rather than the word "None".
    pub fn label(v: &Option<Named>) -> String {
        v.as_ref().and_then(|n| n.name.clone()).unwrap_or_default()
    }
}

/// Mirrors the backend's ComputerStatus.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ComputerStatus {
    pub id: i64,
    pub name: Option<String>,
    pub serial: Option<String>,
    pub last_inventory_update: Option<String>,
    pub hours_since_inventory: Option<i64>,
    pub is_dynamic: Option<bool>,
    pub manufacturer: Option<Named>,
    pub model: Option<Named>,
    #[serde(rename = "type")]
    pub kind: Option<Named>,
    pub entity: Option<Named>,

    /// No agent reports for this machine, so nothing will ever update it.
    pub orphaned: bool,
}

/// Every computer, orphans first.
pub async fn fetch_computers() -> Result<Vec<ComputerStatus>, String> {
    reqwest::get(api_url("/api/computers"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<ComputerStatus>>()
        .await
        .map_err(|e| e.to_string())
}

/// Mirrors the backend's SessionInfo.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionInfo {
    pub profile_name: Option<String>,
    pub profile_interface: Option<String>,
    pub entity_name: Option<String>,
    pub entity_recursive: Option<bool>,
    pub right_agent: Option<i64>,
    pub right_entity: Option<i64>,
}

/// The GLPI identity this app is using.
pub async fn fetch_session() -> Result<SessionInfo, String> {
    reqwest::get(api_url("/api/session"))
        .await
        .map_err(|e| e.to_string())?
        .json::<SessionInfo>()
        .await
        .map_err(|e| e.to_string())
}

/// One GLPI right. Mirrors the backend's RightEntry.
///
/// `value` is the authoritative figure and `bits` is a reading of it; both are
/// shown, because a decoded name is only as good as the constant table behind
/// it and the raw number is what GLPI's own form and source use.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RightEntry {
    pub name: String,
    pub value: i64,
    pub bits: Vec<String>,

    /// Bits beyond GLPI's twelve global constants. Genuine permissions this
    /// app cannot name - modules define their own - so they are shown as a
    /// number rather than hidden. See backend/src/glpi/rights.rs.
    pub unnamed_bits: i64,
    pub writes: bool,
}

impl RightEntry {
    /// Granted at all. A right GLPI lists with a value of 0 is a right the
    /// profile does not have.
    pub fn granted(&self) -> bool {
        self.value != 0
    }
}

/// Mirrors the backend's RightsBreakdown.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RightsBreakdown {
    pub profile_name: Option<String>,
    pub profile_interface: Option<String>,
    pub entries: Vec<RightEntry>,
}

/// Every right the active profile holds.
pub async fn fetch_rights() -> Result<RightsBreakdown, String> {
    reqwest::get(api_url("/api/rights"))
        .await
        .map_err(|e| e.to_string())?
        .json::<RightsBreakdown>()
        .await
        .map_err(|e| e.to_string())
}

/// One authorization: a profile this account may use, on one entity.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Authorization {
    pub entity: String,
    pub profile: String,
    pub interface: String,
    pub is_recursive: bool,
    /// GLPI activates exactly one profile per session; this is that one.
    pub is_active: bool,
    pub right_agent: i64,
    pub right_entity: i64,
}

/// Every authorization the account holds, active one first.
pub async fn fetch_authorizations() -> Result<Vec<Authorization>, String> {
    reqwest::get(api_url("/api/authorizations"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Authorization>>()
        .await
        .map_err(|e| e.to_string())
}
