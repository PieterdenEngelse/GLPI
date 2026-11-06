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

/// Mirrors the backend's ComputerReport.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ComputerReport {
    pub computers: Vec<ComputerStatus>,
    /// Soft-deleted machines GLPI was asked to leave out. `None` means the
    /// count could not be established, which is NOT the same as zero - the
    /// page says so rather than implying nothing was hidden.
    pub deleted_excluded: Option<usize>,
}

/// Every live computer, orphans first, with a count of what was left out.
///
/// Soft-deleted machines are excluded by GLPI rather than here. They used to
/// come through: GLPI applies no default `is_deleted` restriction, and a
/// deleted machine has no agent reporting for it, so it arrived flagged as
/// orphaned - the one state this page exists to raise.
pub async fn fetch_computers() -> Result<ComputerReport, String> {
    reqwest::get(api_url("/api/computers"))
        .await
        .map_err(|e| e.to_string())?
        .json::<ComputerReport>()
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

/// One field the API's schema promised that the response did not carry, or
/// carried without promising. Mirrors the backend's `schema_check::FieldGap`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FieldGap {
    pub field: String,
    /// "suppressed" or "undeclared". A string for the same reason
    /// `Finding::state` is: a kind this build does not know should render as
    /// itself, not fail the page.
    pub kind: String,
}

/// One audited endpoint's comparison of declared fields against received
/// ones. Mirrors the backend's `schema_check::SchemaReport`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SchemaReport {
    pub schema: String,
    pub state: String,
    pub declared: usize,
    pub readable: usize,
    pub received: usize,
    pub gaps: Vec<FieldGap>,
    pub write_only: Vec<String>,
    pub cannot_check: String,
}

/// One schema in GLPI's document, counted rather than compared. Mirrors the
/// backend's `schema_check::SchemaSummary`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SchemaSummary {
    pub schema: String,
    pub declared: usize,
    pub readable: usize,
    pub write_only: usize,
    pub audited: bool,
}

/// How far RSQL filtering reaches. Mirrors the backend's
/// `schema_check::FilterReach`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct FilterReach {
    pub operations: usize,
    pub operations_total: usize,
    pub paths: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FieldGapReport {
    /// False means GLPI served no usable schema document, so nothing could be
    /// compared. Distinct from "compared and found nothing".
    pub doc_available: bool,
    pub reports: Vec<SchemaReport>,
    /// Every schema the document declares. Empty when `doc_available` is
    /// false, and `#[serde(default)]` so a backend that predates the field
    /// degrades to an empty catalogue rather than a failed parse - the same
    /// rule the rest of this file follows.
    #[serde(default)]
    pub catalogue: Vec<SchemaSummary>,
    /// Zeroed, not absent, when the document could not be read - the RSQL tab
    /// shows its reference either way and only withholds the counts.
    #[serde(default)]
    pub filter_reach: FilterReach,
}

/// Fields GLPI's own schema declares that this account does not receive.
///
/// Slow by nature - the backend fetches GLPI's whole OpenAPI document, about
/// 4.6 MB on 12.0.1 - so this is fetched on demand from the Api tab rather
/// than with the rest of the page.
pub async fn fetch_field_gaps() -> Result<FieldGapReport, String> {
    reqwest::get(api_url("/api/field-gaps"))
        .await
        .map_err(|e| e.to_string())?
        .json::<FieldGapReport>()
        .await
        .map_err(|e| e.to_string())
}

/// One service in GLPI's own status check. Mirrors the backend's Service.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StatusService {
    pub name: String,
    /// "ok", "attention" or "unknown".
    pub state: String,
    /// GLPI's own word, carried unmapped so a value this build does not know
    /// still reaches the screen.
    pub raw: Option<String>,
}

/// GLPI's own health. Mirrors the backend's system_status::Report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SystemStatus {
    /// The rollup across every service. Note this is GLPI's `glpi.status`,
    /// which is the global figure and NOT the GLPI service's own health.
    pub overall: String,
    /// GLPI's schema-version check - its actual health.
    pub schema_state: String,
    pub schema_defined: Option<String>,
    pub schema_installed: Option<String>,
    /// GLPI served the anonymous view, so the schema fields above mean
    /// nothing. A finding in itself: this app is reading with less authority
    /// than it should have.
    pub redacted: bool,
    pub stuck: Vec<String>,
    pub errored: Vec<String>,
    pub crontask_state: String,
    pub crontask_summary: Option<String>,
    pub services: Vec<StatusService>,
    pub cannot_check: String,
}

/// GLPI's own health, including automatic actions that stopped running.
pub async fn fetch_system_status() -> Result<SystemStatus, String> {
    reqwest::get(api_url("/api/system-status"))
        .await
        .map_err(|e| e.to_string())?
        .json::<SystemStatus>()
        .await
        .map_err(|e| e.to_string())
}

/// One login session. Mirrors the backend's sessions::LoginSession.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LoginSession {
    pub id: Option<i64>,
    pub login_session_uid: Option<String>,
    pub user: Option<String>,
    pub user_id: Option<i64>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub auth_type: Option<i64>,
    /// Decoded by the backend, so both halves cannot disagree about what 9
    /// means.
    pub auth_label: String,
    pub logged_in_at: Option<String>,
    pub logged_out_at: Option<String>,
    /// When the session was last used. More telling than the sign-in time for
    /// an API client, whose sign-in never changes while it keeps working.
    pub last_activity_at: Option<String>,
    pub logout_reason: Option<String>,
    pub revoker: Option<String>,
}

/// Mirrors the backend's sessions::Report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LoginSessionReport {
    /// "fleet", "self_only" or "unknown" - how much of GLPI's session list
    /// this account was actually shown. A short list under "self_only" means
    /// nothing about who else is logged in.
    pub scope: String,
    pub sessions: Vec<LoginSession>,
    pub cannot_check: String,
}

/// Who is inside GLPI, with the scope the answer was given under.
pub async fn fetch_login_sessions() -> Result<LoginSessionReport, String> {
    reqwest::get(api_url("/api/login-sessions"))
        .await
        .map_err(|e| e.to_string())?
        .json::<LoginSessionReport>()
        .await
        .map_err(|e| e.to_string())
}

// No Infocom client here, deliberately. The backend serves
// /api/infocom/{itemtype}/{id}, but Infocom has no collection route in GLPI,
// so there is no fleet-wide view of it - and every page in this app is
// fleet-wide. Showing it would mean one request per machine, which is the
// shape CLAUDE.md rules out and the reason the inventory pages read v1
// collections instead. A per-asset panel needs a selected asset, and no page
// has a selection yet; adding one is a design change rather than wiring.

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

// -------------------------------------------------------------------- logs
//
// Two of GLPI's own log files, read off disk by the backend. Only
// access-errors.log and api.log; the backend holds the allowlist and this
// side never constructs a path.

/// Why a log has no lines. Mirrors `logs::State` in the backend, including
/// the tag/content shape, so the three "nothing to show" cases stay
/// distinguishable all the way to the screen - absent, empty and unreadable
/// lead to three different fixes.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum LogState {
    Ok,
    Absent,
    Empty,
    Unreadable(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LogResponse {
    /// False when the backend has no GLPI_INV_LOG_DIR. Distinct from a
    /// configured directory the kernel refuses - see LogState::Unreadable.
    pub configured: bool,
    #[serde(flatten)]
    pub tail: Option<LogTail>,
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LogTail {
    pub file: String,
    #[serde(flatten)]
    pub state: LogState,
    pub lines: Vec<String>,
    pub bytes: u64,
    /// The file was longer than the window read, so earlier lines are absent.
    pub truncated: bool,
}

/// The tail of one of GLPI's logs.
pub async fn fetch_log(file: &str) -> Result<LogResponse, String> {
    reqwest::get(api_url(&format!("/api/logs/{file}")))
        .await
        .map_err(|e| e.to_string())?
        .json::<LogResponse>()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LogEntry {
    pub file: String,
    pub bytes: u64,
    /// Whether the backend will serve this file's contents. Comes from the
    /// backend's own allowlist, so this side keeps no copy of it.
    pub readable: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LogListResponse {
    pub configured: bool,
    pub entries: Vec<LogEntry>,
    /// Set when the directory could not be listed. An empty `entries` with no
    /// error is an empty directory, which is a different answer.
    pub error: Option<String>,
}

/// What logs GLPI's directory actually holds.
pub async fn fetch_logs() -> Result<LogListResponse, String> {
    reqwest::get(api_url("/api/logs"))
        .await
        .map_err(|e| e.to_string())?
        .json::<LogListResponse>()
        .await
        .map_err(|e| e.to_string())
}
