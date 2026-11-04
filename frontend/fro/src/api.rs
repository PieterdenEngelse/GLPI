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

    /// Reporting, but nothing it sends is being stored. Defaults to false so
    /// a backend that predates this field degrades to the old two states
    /// rather than failing to parse.
    #[serde(default)]
    pub not_landing: bool,
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
/// origin the backend does not own.
///
/// This app does have that split under `just dev-frontend`, and still does not
/// guess: Dioxus.toml proxies /api from the dev server to the backend, so the
/// page origin is the right base in both cases. Keeping the guess out of here
/// means there is one way URLs are built and no dev-only branch to be wrong
/// about - the difference is config in one file rather than a condition in
/// this one.
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

/// One permission bit of a right. Mirrors the backend's RightBit.
///
/// `writes` comes from the backend rather than being inferred from `name`
/// here. The two agree for every bit GLPI defines today, but only because the
/// write bits happen to be the ones not called READ*; deciding it locally
/// would let a badge's colour contradict the write-capable count the moment
/// that stopped holding.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RightBit {
    pub name: String,
    pub writes: bool,
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
    pub bits: Vec<RightBit>,

    /// Bits beyond GLPI's twelve global constants. Genuine permissions this
    /// app cannot name - modules define their own - so they are shown as a
    /// number rather than hidden. See backend/src/glpi/rights.rs.
    pub unnamed_bits: i64,
    pub writes: bool,

    /// GLPI stores a row for this right on this profile. False means there is
    /// no row - which denies exactly as a stored zero does, but is a different
    /// statement: a zero is a decision taken in the profile form, an absent
    /// row is a right that form never offered here.
    pub recorded: bool,

    /// Where this right sits in GLPI's profile form. None when the committed
    /// taxonomy does not cover it - either the dump has not been run, or the
    /// right genuinely has no row on the form.
    pub tab: Option<String>,
    pub group: Option<String>,
}

impl RightEntry {
    /// Granted at all. A right GLPI lists with a value of 0 is a right the
    /// profile does not have.
    pub fn granted(&self) -> bool {
        self.value != 0
    }
}

/// One profile's rights. Mirrors the backend's ProfileRights.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProfileRights {
    pub id: i64,
    pub name: Option<String>,
    pub interface: Option<String>,
    pub is_default: bool,

    /// The profile this app's own session runs under - the one every other
    /// page is actually working through.
    pub is_active: bool,
    pub entries: Vec<RightEntry>,
}

impl ProfileRights {
    /// What to show for this profile. Falls back to the id rather than a blank:
    /// a selector with an empty entry in it is worse than one admitting
    /// "profile 7".
    pub fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("profile {}", self.id))
    }

    pub fn granted(&self) -> usize {
        self.entries.iter().filter(|e| e.granted()).count()
    }

    pub fn writes(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.granted() && e.writes)
            .count()
    }
}

/// One right a call depends on. Mirrors the backend's CallRight.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CallRight {
    pub name: String,

    /// None when GLPI records no entry for this right on the profile - which
    /// denies, like a zero, but is not a value somebody set.
    pub value: Option<i64>,
    pub satisfied: bool,
}

/// One feature a call serves. Mirrors the backend's CallFeature.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CallFeature {
    pub name: String,

    /// What the call contributes, where it is not the whole feature.
    pub role: Option<String>,
}

/// One of this app's calls, judged against the active profile. Mirrors the
/// backend's CallStatus.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CallStatus {
    pub path: String,
    pub api: String,

    /// Every feature this call serves. A call appears under each of them, so
    /// one blocked right visibly breaks more than one page.
    pub features: Vec<CallFeature>,
    pub rights: Vec<CallRight>,
    pub satisfied: bool,
}

/// Mirrors the backend's RightsReport.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RightsReport {
    pub active_profile_id: Option<i64>,
    pub profiles: Vec<ProfileRights>,
    pub calls: Vec<CallStatus>,

    /// Right names the committed taxonomy covers. Zero means grouping is off
    /// and the flat list is shown.
    pub taxonomy_rights: usize,

    /// Rights GLPI's form offers that no profile records a value for. Not a
    /// fault: GLPI ships at least one, and an unrecorded right is simply
    /// denied.
    pub form_only_rights: Vec<String>,
}

/// Every right of every profile in GLPI.
pub async fn fetch_rights() -> Result<RightsReport, String> {
    reqwest::get(api_url("/api/rights"))
        .await
        .map_err(|e| e.to_string())?
        .json::<RightsReport>()
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

/// One observation inside a posture tile.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Finding {
    /// "ok", "attention" or "unknown". Kept as a string rather than an enum:
    /// a state this build does not recognise should render as unknown, not
    /// fail the whole page to a parse error.
    pub state: String,
    pub label: String,
    pub detail: String,
    /// Path into GLPI where this is changed, relative to its base URL.
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PostureTile {
    pub id: String,
    pub title: String,
    pub state: String,
    pub findings: Vec<Finding>,
    pub cannot_check: String,
}

/// The four posture tiles.
pub async fn fetch_posture() -> Result<Vec<PostureTile>, String> {
    reqwest::get(api_url("/api/posture"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<PostureTile>>()
        .await
        .map_err(|e| e.to_string())
}

/// One submission GLPI refused.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Refused {
    pub id: i64,
    pub name: Option<String>,
    pub itemtype: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,
    pub ip: Option<String>,
    pub mac: Option<String>,
    pub method: Option<String>,
    /// The rule that refused it - the actionable field.
    pub rules_id: Option<i64>,
    pub agents_id: Option<i64>,
    pub entities_id: Option<i64>,
    pub date_creation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RefusedReport {
    /// False when no database access is configured. "No refusals" and
    /// "cannot look" are opposite answers and the page renders them
    /// differently.
    pub configured: bool,
    pub entities: Vec<i64>,
    pub rows: Vec<Refused>,
}

/// Submissions GLPI refused - the one thing neither API exposes.
pub async fn fetch_refused() -> Result<RefusedReport, String> {
    reqwest::get(api_url("/api/refused"))
        .await
        .map_err(|e| e.to_string())?
        .json::<RefusedReport>()
        .await
        .map_err(|e| e.to_string())
}

// --------------------------------------------------------------- inventory
//
// The three fleet-wide inventory views. Each mirrors a backend struct in
// glpi/inventory.rs, with the same deliberate duplication and the same rule:
// every field that GLPI can leave unrecorded is an Option, so a backend that
// gains or drops one degrades to a blank cell rather than failing the parse.

/// One processor in a machine.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Processor {
    pub designation: Option<String>,
    pub nbcores: Option<i64>,
    pub nbthreads: Option<i64>,
    /// MHz, as GLPI stores it.
    pub frequency: Option<i64>,
}

/// One memory module.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemoryModule {
    pub designation: Option<String>,
    /// MB, as GLPI stores it.
    pub size: Option<i64>,
}

/// One filesystem the agent found mounted.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Disk {
    pub name: Option<String>,
    pub device: Option<String>,
    pub mountpoint: Option<String>,
    pub filesystem: Option<String>,
    /// MB, as GLPI stores it.
    pub totalsize: Option<i64>,
    pub freesize: Option<i64>,
    pub encryption_status: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OperatingSystem {
    pub name: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: Option<String>,
}

/// One machine's hardware. Mirrors the backend's HardwareRow.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HardwareRow {
    pub items_id: i64,
    pub computer: Option<String>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    #[serde(default)]
    pub memory: Vec<MemoryModule>,
    #[serde(default)]
    pub memory_mb: i64,
    #[serde(default)]
    pub disks: Vec<Disk>,
    pub operatingsystem: Option<OperatingSystem>,
    /// Nothing was collected for this machine at all - the row worth looking
    /// at, and not the same as a machine whose disks were not reported.
    #[serde(default)]
    pub empty: bool,
}

/// What the agents found inside each machine, fleet-wide.
pub async fn fetch_hardware() -> Result<Vec<HardwareRow>, String> {
    reqwest::get(api_url("/api/hardware"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<HardwareRow>>()
        .await
        .map_err(|e| e.to_string())
}

/// One software title and how widely it is installed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoftwareRow {
    pub softwares_id: i64,
    pub name: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub installs: i64,
    #[serde(default)]
    pub computers: i64,
}

impl SoftwareRow {
    /// What to show for this title.
    ///
    /// Falling back to the id rather than a blank keeps an unnamed title
    /// findable: GLPI holds the row, so "software 473" is something that can
    /// be looked up, where an empty cell is not.
    pub fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("software {}", self.softwares_id))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoftwareReport {
    #[serde(default)]
    pub rows: Vec<SoftwareRow>,
    #[serde(default)]
    pub computers_covered: i64,
    #[serde(default)]
    pub installs: i64,
}

/// Every software title installed across the fleet, most widely first.
pub async fn fetch_software() -> Result<SoftwareReport, String> {
    reqwest::get(api_url("/api/software"))
        .await
        .map_err(|e| e.to_string())?
        .json::<SoftwareReport>()
        .await
        .map_err(|e| e.to_string())
}

/// One network port on a machine.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Port {
    pub id: i64,
    pub name: Option<String>,
    pub mac: Option<String>,
    pub instantiation_type: Option<String>,
    pub logical_number: Option<i64>,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub is_dynamic: bool,
}

impl Port {
    /// GLPI's instantiation_type without its "NetworkPort" prefix - Wifi
    /// rather than NetworkPortWifi. The prefix is on every value, so it
    /// distinguishes nothing and costs the column its width.
    pub fn kind(&self) -> String {
        self.instantiation_type
            .as_deref()
            .map(|t| t.strip_prefix("NetworkPort").unwrap_or(t).to_string())
            .unwrap_or_default()
    }
}

/// One machine's network ports. Mirrors the backend's NetworkRow.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NetworkRow {
    pub items_id: i64,
    pub computer: Option<String>,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub empty: bool,
}

/// Every machine's network ports and addresses.
pub async fn fetch_network() -> Result<Vec<NetworkRow>, String> {
    reqwest::get(api_url("/api/network"))
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<NetworkRow>>()
        .await
        .map_err(|e| e.to_string())
}
