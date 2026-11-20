//! Fetch helpers for every backend endpoint, as in RERAG's api.rs.
//!
//! Components never call reqwest themselves. One module knows the paths and
//! the response shapes, so a backend change lands in one file rather than in
//! whichever component happened to be fetching.
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};

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
/// Every GET goes through here, so the three outcomes are decided once.
///
/// STATUS BEFORE BODY, and that order is the whole point. Calling `.json()`
/// straight on the response - which all 21 of these did until 2026-11-18 -
/// makes every non-2xx a decode failure, because an error envelope is never
/// the struct the caller wanted. That labelled two unrelated things as
/// version skew: a 404 for a path this build calls and the backend does not
/// have, which is roughly right, and a 502 because GLPI is down, which sends
/// the reader off to rebuild the wrong thing entirely.
async fn get_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let r = reqwest::get(api_url(path))
        .await
        .map_err(|e| ApiError::Unreachable(e.to_string()))?;

    let status = r.status();
    if !status.is_success() {
        let body = r.text().await.unwrap_or_default();
        return Err(ApiError::upstream(status, body));
    }

    let v = r.json::<T>().await.map_err(ApiError::unreadable)?;
    SKEW.store(false, Ordering::Relaxed);
    Ok(v)
}

/// Whether the LAST decode outcome was a failure. Set on one, cleared by any
/// success.
///
/// It latched until 2026-11-18 and the argument for that was wrong in one
/// direction: when the stale half is the deployed backend, a redeploy fixes
/// the mismatch under a page that is still open, and a latch goes on
/// accusing it for ever. Clearing on success makes the banner say what is
/// true now rather than what was ever true, and a skew that is still real
/// re-arms within a poll because the failing endpoint keeps failing.
///
/// Checking status before decoding (see `get_json`) is what makes this safe:
/// without it a 502 from GLPI being down also landed here, and flapping a
/// *wrong* label is worse than latching it.
static SKEW: AtomicBool = AtomicBool::new(false);

/// True once any fetch anywhere has failed to decode.
///
/// A plain atomic rather than a `GlobalSignal`, and that is not a style
/// choice: a signal write from inside a fetch future runs outside the Dioxus
/// runtime, which kills the task. The first attempt did exactly that and left
/// every page stuck on "Loading..." for ever - a worse failure than the one
/// being reported, and invisible to the type system.
///
/// The cost is that this is not reactive: a dot already on screen picks it up
/// on its next render rather than the instant it flips. That is enough. The
/// page whose own fetch failed re-renders immediately and says so itself, and
/// the status light re-renders on its 30-second poll and whenever its panel
/// is opened.
pub fn skew_seen() -> bool {
    SKEW.load(Ordering::Relaxed)
}

/// Why a fetch failed, because the two reasons are opposite news.
///
/// `Unreachable` is the backend: the request never got out, or nothing
/// answered it. `Unreadable` is THIS APP: the backend answered, the status
/// was accepted, and the body did not fit the struct waiting for it. That is
/// version skew between the bundle in the browser and the binary serving it -
/// every upstream is healthy.
///
/// WHICH OF THE TWO IS OLDER IS NOT KNOWABLE FROM HERE, and pretending
/// otherwise cost a wrong instruction on the day this was written: the text
/// told the reader to reload while the stale half was the deployed backend,
/// where reloading can never help. A backend older than the bundle answers a
/// path it does not have with the SPA fallback - index.html, 200 - and
/// parsing a web page as JSON lands here. So the wording names the
/// disagreement and gives both remedies rather than guessing a direction.
///
/// They were one `String` until 2026-11-18, and the cost was not cosmetic.
/// Every page rendered either as "Could not reach the backend", which is
/// false when the backend just answered with 200; and the status light
/// counted a decode failure as the DATABASE being degraded, so a stale
/// bundle presented as a fault in MariaDB and sent the reader to the wrong
/// system entirely. A skew is this app's fault and has to say so.
#[derive(Clone, PartialEq, Debug)]
pub enum ApiError {
    /// Nothing answered.
    Unreachable(String),
    /// It answered with a status this app did not ask for, and said why.
    /// Its own sentence, not a guess made from the number.
    Upstream(u16, String),
    /// It answered 2xx; this bundle could not read the reply.
    Unreadable(String),
}

impl ApiError {
    /// Build an `Unreadable` and record it app-wide.
    ///
    /// The recording is the point. The status light polls four endpoints of
    /// its own, so a skew met by any OTHER fetch - an agent list, a perf
    /// reading, the variables table - used to leave the dot green while the
    /// page under it said the bundle and the backend disagree. Same
    /// contradiction the two variants exist to end, one level up.
    fn unreadable<E: std::fmt::Display>(e: E) -> Self {
        SKEW.store(true, Ordering::Relaxed);
        ApiError::Unreadable(e.to_string())
    }

    /// Build an `Upstream`, preferring the backend's own words over the
    /// status number - "no such endpoint: /api/x" beats "404".
    ///
    /// Three fallbacks deep because a refusal can arrive with nothing in it:
    /// this server answers a GET on a POST-only route with 405 and a body of
    /// zero bytes, which would otherwise render as "405: " and say less than
    /// the number alone.
    fn upstream(status: reqwest::StatusCode, body: String) -> Self {
        let msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .filter(|m| !m.trim().is_empty())
            .or_else(|| {
                let t = body.trim();
                (!t.is_empty()).then(|| t.chars().take(200).collect())
            })
            .unwrap_or_else(|| {
                status
                    .canonical_reason()
                    .unwrap_or("no message")
                    .to_string()
            });
        ApiError::Upstream(status.as_u16(), msg)
    }

    /// True when the fault is this app's, not an upstream's.
    pub fn is_skew(&self) -> bool {
        matches!(self, ApiError::Unreadable(_))
    }

    /// True when the backend answered and refused.
    pub fn is_upstream(&self) -> bool {
        matches!(self, ApiError::Upstream(..))
    }
}

impl std::fmt::Display for ApiError {
    /// The bare message. Callers own the framing, because what to say about
    /// a skew depends on whether there is room to say "reload".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unreachable(m) | ApiError::Unreadable(m) => write!(f, "{m}"),
            ApiError::Upstream(status, m) => write!(f, "{status}: {m}"),
        }
    }
}

pub async fn fetch_agents() -> Result<Vec<AgentStatus>, ApiError> {
    get_json::<Vec<AgentStatus>>("/api/agents").await
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
pub async fn fetch_entities() -> Result<Vec<Entity>, ApiError> {
    get_json::<Vec<Entity>>("/api/entities").await
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
pub async fn fetch_health() -> Result<Health, ApiError> {
    get_json::<Health>("/api/health").await
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
pub async fn fetch_computers() -> Result<ComputerReport, ApiError> {
    get_json::<ComputerReport>("/api/computers").await
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
pub async fn fetch_session() -> Result<SessionInfo, ApiError> {
    get_json::<SessionInfo>("/api/session").await
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
pub async fn fetch_rights() -> Result<RightsReport, ApiError> {
    get_json::<RightsReport>("/api/rights").await
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
pub async fn fetch_authorizations() -> Result<Vec<Authorization>, ApiError> {
    get_json::<Vec<Authorization>>("/api/authorizations").await
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
pub async fn fetch_posture() -> Result<Vec<PostureTile>, ApiError> {
    get_json::<Vec<PostureTile>>("/api/posture").await
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
pub async fn fetch_field_gaps() -> Result<FieldGapReport, ApiError> {
    get_json::<FieldGapReport>("/api/field-gaps").await
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
pub async fn fetch_system_status() -> Result<SystemStatus, ApiError> {
    get_json::<SystemStatus>("/api/system-status").await
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
pub async fn fetch_login_sessions() -> Result<LoginSessionReport, ApiError> {
    get_json::<LoginSessionReport>("/api/login-sessions").await
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
pub async fn fetch_refused() -> Result<RefusedReport, ApiError> {
    get_json::<RefusedReport>("/api/refused").await
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
pub async fn fetch_hardware() -> Result<Vec<HardwareRow>, ApiError> {
    get_json::<Vec<HardwareRow>>("/api/hardware").await
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
pub async fn fetch_software() -> Result<SoftwareReport, ApiError> {
    get_json::<SoftwareReport>("/api/software").await
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
pub async fn fetch_network() -> Result<Vec<NetworkRow>, ApiError> {
    get_json::<Vec<NetworkRow>>("/api/network").await
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
pub async fn fetch_log(file: &str) -> Result<LogResponse, ApiError> {
    get_json::<LogResponse>(&format!("/api/logs/{file}")).await
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
pub async fn fetch_logs() -> Result<LogListResponse, ApiError> {
    get_json::<LogListResponse>("/api/logs").await
}

/// What the MariaDB instance behind GLPI reports about itself.
///
/// Every field is `Option` or defaulted for the reason the whole file is: a
/// backend one version ahead must degrade to a blank cell, not a parse
/// failure that blanks the page.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct ServerFacts {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub flavour: String,
    #[serde(default)]
    pub uptime_seconds: u64,
    #[serde(default)]
    pub variables: Vec<ServerSetting>,
    #[serde(default)]
    pub engines: Vec<(String, String)>,
    #[serde(default)]
    pub glpi_tables: u64,
    #[serde(default)]
    pub glpi_bytes: u64,
    #[serde(default)]
    pub glpi_other_engines: Vec<(String, String)>,
}

/// One of the ten settings on the server panel. Mirrors the backend's
/// ServerSetting - `origin` is the field the panel is really for, since a
/// value equal to its default says nothing about whether anyone chose it.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct ServerSetting {
    pub name: String,
    pub value: String,
    /// None where the server publishes no default. Not a divergence - there
    /// is nothing on the other side to differ from.
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct MariadbReport {
    /// False when the app has no database URL. The page says so rather than
    /// rendering an empty table - "not configured" and "nothing found" are
    /// opposite answers.
    #[serde(default)]
    pub configured: bool,
    pub facts: Option<ServerFacts>,
}

/// One group of InnoDB settings. The grouping is the backend's, and the
/// order of the groups is the order they are rendered in - the panel does
/// not sort them, because the order is a reading order rather than
/// alphabetical.
/// One setting, with the server's own description of it when the server has
/// one. `help` is None for the two this server reports but does not
/// document; the panel says that rather than showing an empty button.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct InnodbSetting {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub help: Option<String>,
    /// Startup-only on this server - 31 of 137. The row renders as text.
    #[serde(default)]
    pub read_only: bool,
    /// Every value the server will accept, for booleans and enums. Empty for
    /// everything else, which is what makes it the test for "does this row
    /// get a dropdown".
    #[serde(default)]
    pub options: Vec<String>,
    /// The server's own word: BOOLEAN, ENUM, BIGINT UNSIGNED, DOUBLE,
    /// VARCHAR and so on. Anything this app does not recognise falls back to
    /// a text box rather than to nothing.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
    #[serde(default)]
    pub block: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
    /// What the startup file asks for, if it names this setting. None means
    /// a restart hands it back to my.cnf and the compiled default; a value
    /// differing from `value` means the file and the running server disagree.
    #[serde(default)]
    pub persisted: Option<String>,
}

#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct InnodbGroup {
    pub category: String,
    #[serde(default)]
    pub variables: Vec<InnodbSetting>,
}

#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct InnodbReport {
    #[serde(default)]
    pub configured: bool,
    /// Whether the backend will accept a change at all. False is the default
    /// and the panel says why rather than hiding its controls.
    #[serde(default)]
    pub writes_enabled: bool,
    /// Where persisted settings are written. None means no startup file is
    /// configured, and the panel says a change lasts until the next restart
    /// rather than offering to keep it.
    #[serde(default)]
    pub persist_path: Option<String>,
    #[serde(default)]
    pub groups: Vec<InnodbGroup>,
}

/// What a write did: what was asked for, and what the server reports now.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct InnodbChange {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub requested: String,
    #[serde(default)]
    pub value: String,
}

/// One system variable, with the tier the backend put it in.
///
/// `tier` is the backend's word and is rendered as given. Classifying here
/// too would mean two lists of names drifting apart, and the backend's is
/// the one with tests behind it.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct Variable {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub tier: String,
    /// What it is for, as against `tier`, which is what changing it costs.
    /// The backend's opinion - see its `vargroup` module. Defaulted like the
    /// rest, so a backend that has not got this field yet degrades to one
    /// unnamed group rather than failing to parse.
    #[serde(default)]
    pub group: String,
    /// Whether SET GLOBAL can touch this at all, which is NOT `!read_only`:
    /// a SESSION ONLY variable reports READ_ONLY = NO and still has no global
    /// value. 13 rows on this server sit in the live tier and cannot be set.
    #[serde(default)]
    pub settable: bool,
    /// The server's own word for the type, which decides the control.
    #[serde(default)]
    pub kind: String,
    /// Everything an enum or boolean accepts. Empty for everything else, and
    /// the reason a select can never propose a value the server refuses.
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
    /// Why this one needs confirming first, when it does. The backend decides
    /// and sends the text - duplicating the list here would be two lists.
    #[serde(default)]
    pub hazard: Option<String>,
    /// What the startup file asks for, when it names this variable.
    ///
    /// Beside `value`, not instead of it: a line in that file is what the
    /// NEXT start applies, and the running server can have been set to
    /// something else since. The two disagreeing is the finding.
    #[serde(default)]
    pub persisted: Option<String>,
}

/// Every system variable the server has, in one response.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct VariablesReport {
    #[serde(default)]
    pub configured: bool,
    /// Whether this backend accepts writes at all. False is the default, so a
    /// backend too old to say anything is read as refusing - the safe way for
    /// that guess to be wrong.
    #[serde(default)]
    pub writes_enabled: bool,
    /// Where a kept value is written, if anywhere. None means nothing can
    /// outlive a restart, and the page says so rather than offering a Keep
    /// control that writes to nothing.
    #[serde(default)]
    pub persist_path: Option<String>,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

/// Keep a variable across restarts, or stop keeping it.
///
/// `Some(value)` writes it into the file MariaDB replays at startup; None
/// drops the line and leaves the running server exactly as it is - what
/// changes is the next start, not this one.
pub async fn persist_variable(name: &str, value: Option<&str>) -> Result<(), String> {
    let req =
        reqwest::Client::new().post(api_url(&format!("/api/mariadb/variables/{name}/persist")));
    let req = match value {
        Some(v) => req.json(&serde_json::json!({ "value": v })),
        None => req,
    };

    let r = req.send().await.map_err(|e| e.to_string())?;
    if r.status().is_success() {
        return Ok(());
    }
    let status = r.status();
    match r.json::<serde_json::Value>().await {
        Ok(v) => Err(v
            .get("error")
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| status.to_string())),
        Err(_) => Err(status.to_string()),
    }
}

/// What came back from trying to set a variable.
///
/// The confirm case is not an error and must not render as one: nothing has
/// gone wrong, the server is intact, and what is being asked for is a second
/// press with the consequence in view.
pub enum SetOutcome {
    Done(VariableChange),
    NeedsConfirmation(String),
}

/// What a write did, and whether it survives a restart.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct VariableChange {
    #[serde(default)]
    pub value: String,
    /// Whether the startup file now carries it too. Setting keeps by
    /// default, so false means there was nowhere to keep it - which the row
    /// has to say, because the change itself did happen.
    #[serde(default)]
    pub kept: bool,
}

/// Set one system variable on the running server.
///
/// `confirm` is only consulted for the nine names the backend guards; for
/// everything else the first request is the only one. The 409 that asks for
/// it is deliberately distinguished from the 400 that means the value itself
/// is wrong, because they need opposite things from the reader.
pub async fn set_variable(name: &str, value: &str, confirm: bool) -> Result<SetOutcome, String> {
    let r = reqwest::Client::new()
        .post(api_url(&format!("/api/mariadb/variables/{name}")))
        .json(&serde_json::json!({ "value": value, "confirm": confirm }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if r.status().is_success() {
        return r
            .json::<VariableChange>()
            .await
            .map(SetOutcome::Done)
            .map_err(|e| e.to_string());
    }

    let status = r.status();
    let body = r.json::<serde_json::Value>().await.ok();
    if status == reqwest::StatusCode::CONFLICT {
        if let Some(w) = body
            .as_ref()
            .and_then(|v| v.get("warning"))
            .and_then(|w| w.as_str())
        {
            return Ok(SetOutcome::NeedsConfirmation(w.to_string()));
        }
    }
    Err(body
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| status.to_string()))
}

/// The whole variable space, about 700 rows. Its own request for the same
/// reason the InnoDB one is: only the tier pages read it, and the panels
/// that load with the page must not pay for it.
pub async fn fetch_variables() -> Result<VariablesReport, ApiError> {
    get_json::<VariablesReport>("/api/mariadb/variables").await
}

/// Every InnoDB setting the server has, grouped. Its own request rather than
/// part of the facts: 137 rows that only one panel reads.
pub async fn fetch_innodb() -> Result<InnodbReport, ApiError> {
    get_json::<InnodbReport>("/api/mariadb/innodb").await
}

/// Set one InnoDB setting, and report what it became.
///
/// The only request this app makes that is not a GET. A refusal comes back
/// as 400 with the reason in `error`, and that sentence is what the panel
/// shows: the server's own objection beats anything this layer could invent
/// about why a value was not taken.
pub async fn set_innodb(name: &str, value: &str) -> Result<InnodbChange, String> {
    let r = reqwest::Client::new()
        .post(api_url(&format!("/api/mariadb/innodb/{name}")))
        .json(&serde_json::json!({ "value": value }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if r.status().is_success() {
        return r.json::<InnodbChange>().await.map_err(|e| e.to_string());
    }
    // The body carries the reason; the status alone would reduce "takes one
    // of OFF, ON" to "400".
    let status = r.status();
    match r.json::<serde_json::Value>().await {
        Ok(v) => Err(v
            .get("error")
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| status.to_string())),
        Err(_) => Err(status.to_string()),
    }
}

/// Keep a setting across restarts, or stop keeping it.
///
/// `Some(value)` writes it into the file MariaDB replays at startup; None
/// drops the line, leaving the running server exactly as it is - what is
/// being changed is the next restart, not this one.
pub async fn persist_innodb(name: &str, value: Option<&str>) -> Result<(), String> {
    let url = api_url(&format!("/api/mariadb/innodb/{name}/persist"));
    let req = reqwest::Client::new().post(url);
    let req = match value {
        Some(v) => req.json(&serde_json::json!({ "value": v })),
        None => req,
    };

    let r = req.send().await.map_err(|e| e.to_string())?;
    if r.status().is_success() {
        return Ok(());
    }
    let status = r.status();
    match r.json::<serde_json::Value>().await {
        Ok(v) => Err(v
            .get("error")
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| status.to_string())),
        Err(_) => Err(status.to_string()),
    }
}

/// What scripts/db-restart.sh said.
///
/// `code` is the script's own exit status and the UI reads it rather than the
/// HTTP status: 0 done, 2 misuse, 3 not possible on this operating system, 4
/// tried and failed. 3 is not an error - on NixOS it is the normal answer for
/// the settings link, and what comes back is the instructions.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct RestartRun {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub script: String,
    #[serde(default)]
    pub preview: bool,
}

/// Ask the backend to run the restart script.
///
/// `preview` sets DRY_RUN in the script, which prints the commands and runs
/// nothing - that is what both links do first, so nothing restarts until a
/// second, separate click.
pub async fn db_restart(
    mode: &str,
    settings: &[String],
    preview: bool,
) -> Result<RestartRun, String> {
    let body = serde_json::json!({ "mode": mode, "settings": settings, "preview": preview });
    let res = reqwest::Client::new()
        .post(api_url("/api/db/restart"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        let msg = res
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str().map(String::from)))
            .unwrap_or_else(|| status.to_string());
        return Err(msg);
    }
    res.json::<RestartRun>().await.map_err(|e| e.to_string())
}

/// One timed thing: an endpoint of GLPI's, or a query of the kind its pages
/// run. Every field optional-by-default for the usual reason - a backend
/// that grows a field should not fail the page that has not learned it yet.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct Timing {
    #[serde(default)]
    pub what: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub best_ms: f64,
    #[serde(default)]
    pub median_ms: f64,
    #[serde(default)]
    pub worst_ms: f64,
    #[serde(default)]
    pub error: Option<String>,
}

/// A derived reading with the figures it came from. No verdicts: the
/// backend formats the value and names its inputs, and what counts as good
/// is the reader's call on their own workload.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct Gauge {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub detail: String,
}

/// One of GLPI's scheduled tasks, timed by GLPI itself.
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct CronTask {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub frequency: i64,
    /// GLPI's own column: 0 disabled, 1 waiting, 2 running.
    #[serde(default)]
    pub state: i64,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub avg_seconds: f64,
    #[serde(default)]
    pub max_seconds: f64,
    #[serde(default)]
    pub runs: i64,
}

#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct TableSize {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub rows: i64,
    #[serde(default)]
    pub kib: i64,
}

#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct PerfReport {
    #[serde(default)]
    pub glpi_url: String,
    #[serde(default)]
    pub php: Option<String>,
    #[serde(default)]
    pub http: Vec<Timing>,
    #[serde(default)]
    pub query: Option<Timing>,
    #[serde(default)]
    pub samples: usize,
    #[serde(default)]
    pub engine: Vec<Gauge>,
    #[serde(default)]
    pub cron: Vec<CronTask>,
    #[serde(default)]
    pub tables: Vec<TableSize>,
    #[serde(default)]
    pub total_kib: i64,
    #[serde(default)]
    pub total_tables: i64,
}

/// Measured on request, never cached: the page is for reloading after a
/// change, and a cached number answers the question the reader asked before
/// they made it.
pub async fn fetch_perf() -> Result<PerfReport, ApiError> {
    get_json::<PerfReport>("/api/perf").await
}

pub async fn fetch_mariadb() -> Result<MariadbReport, ApiError> {
    get_json::<MariadbReport>("/api/mariadb").await
}
