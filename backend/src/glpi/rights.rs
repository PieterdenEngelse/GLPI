//! The rights of every profile in GLPI, decoded.
//!
//! WHY THIS DOES NOT USE v2.3. The HL API cannot answer this, in two separate
//! ways, both verified against GLPI 11.0.12 rather than assumed:
//!
//!   * `Administration/Profile` lists every profile but carries no rights -
//!     its schema is id, name, comment, is_default, helpdesk_hardware,
//!     helpdesk_item_type, managed_domainrecordtypes and mfa_enforced
//!     (AdministrationController.php:645-699). `ProfileRight` has no HL route
//!     at all.
//!   * `GET /session` does hold real rights, but only for the one profile
//!     GLPI activated. A `GLPI-Profile` header can re-point it
//!     (Router.php:780), yet `Session::changeProfile` is a silent no-op for
//!     any profile the account is not authorised for (Session.php:592, which
//!     has no else), so asking about Super-Admin returns the service account's
//!     own rights under 200 OK. Confirmed live for ids 4, 8 and a nonexistent
//!     999.
//!
//! So v1's `ProfileRight` collection is used instead: 899 rows covering all
//! nine profiles on a stock install, in one request, with no profile the
//! account happens not to hold being quietly misreported. See glpi::legacy for
//! why a second API is tolerated.
//!
//! Those 899 rows are not nine equal sets. GLPI stores only the rows that
//! exist, so a right can be absent from one profile and present on another -
//! `rule_change` and `rule_problem` have no row on five of the nine. The
//! report therefore counts every profile against the union of all right names
//! and marks the gaps, rather than letting each profile carry its own
//! denominator. See `RightsReport::from_legacy`.
//!
//! IT ALSO FIXES THE RIGHT NAMES. The session blob's rights map is not a list
//! of rights. CoreController.php:626-633 walks every column of the profile row
//! and zeroes anything that is not a right:
//!
//!   foreach ($active_profile as $key => $value) {
//!       if (in_array($key, $all_right_names, true)) {
//!           $session['active_profile']['rights'][$key] = (int) $value;
//!       } else {
//!           $session['active_profile']['rights'][$key] = 0;
//!
//! On this install that yields 122 keys of which only 101 are rights. The
//! other 21 - id, name, comment, interface, date_mod, is_default, entities,
//! ticket_status, helpdesk_hardware, 2fa_enforced and the rest - are ordinary
//! columns reported as a right with value 0, which reads on screen as "this
//! permission is not granted". v1 returns rights and nothing else.
use serde::{Deserialize, Serialize};

/// GLPI's global right bits, from src/autoload/constants.php:67-79 in
/// 12.0.1. Read straight out of the release rather than remembered:
///
///   define("READ", 1);      define("UPDATE", 2);   define("CREATE", 4);
///   define("DELETE", 8);    define("PURGE", 16);   define("READNOTE", 32);
///   define("UPDATENOTE", 64); define("UNLOCK", 128);
///   define("READ_ASSIGNED", 256);  define("UPDATE_ASSIGNED", 512);
///   define("READ_OWNED", 1024);    define("UPDATE_OWNED", 2048);
///
/// ALLSTANDARDRIGHT (31) is deliberately absent: it is the OR of the first
/// five, not a bit of its own, and listing it would double-count.
///
/// ONLY THE FIRST EIGHT ARE GLOBAL, and that is the whole reason this is
/// split into three tables instead of one. A right's bitmask is stored per
/// right NAME - glpi_profilerights is unique on (profiles_id, name) - so the
/// meaning of a bit is a property of the name, not of the number. Bits 0-7
/// are the generic set GLPI offers on essentially every right. Bits 8-11 are
/// not: `grep -rE 'const [A-Z_]+ *= *(1024|2048) *;' src/` finds bit 10
/// carrying 15 different right constants and bit 11 carrying 9, and they are
/// not even consistent with each other - READALL is 1024 under `ticket` and
/// 2048 under `planning`, exactly inverted.
///
/// Reading bits 8-11 globally is therefore wrong for most rights, and wrong
/// in the direction this file must never err in: `inventory` 1024 is
/// IMPORTFROMFILE and `user` 1024 is IMPORTEXTAUTHUSERS - both mutations -
/// yet READ_OWNED is not in `WRITE_BITS`, so both would be presented as
/// read-only. See the `unnamed_bits` doc for the understatement rule this
/// upholds.
const GLOBAL_BITS: [(i64, &str); 8] = [
    (1, "READ"),
    (2, "UPDATE"),
    (4, "CREATE"),
    (8, "DELETE"),
    (16, "PURGE"),
    (32, "READNOTE"),
    (64, "UPDATENOTE"),
    (128, "UNLOCK"),
];

/// Bits 8-11, named only for the rights where they verifiably mean these.
const ASSIGNABLE_BITS: [(i64, &str); 4] = [
    (256, "READ_ASSIGNED"),
    (512, "UPDATE_ASSIGNED"),
    (1024, "READ_OWNED"),
    (2048, "UPDATE_OWNED"),
];

/// The rights whose items implement `AssignableItemInterface`, so bits 8-11
/// are the assignable-item rights and `ASSIGNABLE_BITS` applies.
///
/// Derived by listing the classes implementing that interface in GLPI 12.0.1
/// and reading each one's `$rightname`, rather than by guessing which types
/// look asset-shaped. Detection by the interface and not by `use
/// AssignableItem` matters: Computer, Monitor and Printer take the trait with
/// an aliasing block (`use AssignableItem { ... }`), which a search for `use
/// AssignableItem;` misses entirely - it finds six classes instead of
/// twenty-odd.
///
/// `Asset` implements it too but computes `$rightname` at runtime as
/// `asset_<system_name>`, which is why `is_assignable` also takes that
/// prefix: every custom asset type is assignable.
const ASSIGNABLE_RIGHTS: &[&str] = &[
    "appliance",
    "cable_management",
    "cartridge",
    "certificate",
    "cluster",
    "computer",
    "consumable",
    "database",
    "datacenter",
    "domain",
    "enclosure",
    "license",
    "line",
    "monitor",
    "networking",
    "peripheral",
    "phone",
    "printer",
    "software",
    "unmanaged",
];

/// One bit of one right: its value, GLPI's constant name for it, and whether
/// holding it lets you change something.
type BitMeaning = (i64, &'static str, bool);

/// A right name paired with the bit meanings that differ from the tables
/// above.
type RightOverride = (&'static str, &'static [BitMeaning]);

/// Per-right bit meanings that differ from the tables above.
///
/// `(bit, GLPI's constant name, whether it grants mutation)`. An entry here
/// REPLACES whatever the global or assignable table would have said for that
/// bit, which is why `rssfeed_public` and `reminder_public` can override bit
/// 7: both define PERSONAL = 128, colliding with the global UNLOCK.
///
/// Every name was read from the defining class in GLPI 12.0.1. Not
/// exhaustive, and deliberately not: a right absent from here has its bits
/// 8-11 reported as unnamed rather than guessed at. Adding a row is how this
/// table grows - never by inferring a meaning from a bit's number.
const RIGHT_OVERRIDES: &[RightOverride] = &[
    // CommonITILObject.php:143 READALL; Ticket.php:106 READGROUP.
    (
        "ticket",
        &[(1024, "READALL", false), (2048, "READGROUP", false)],
    ),
    ("change", &[(1024, "READALL", false)]),
    ("problem", &[(1024, "READALL", false)]),
    // DisplayPreference.php:55-56. Both write: they store rows in
    // glpi_displaypreferences, GENERAL for every user at once.
    (
        "search_config",
        &[(1024, "PERSONAL", true), (2048, "GENERAL", true)],
    ),
    // Planning.php:108-109 - the inverse of `ticket`, which is the clearest
    // proof that a bit number carries no portable meaning.
    (
        "planning",
        &[(1024, "READGROUP", false), (2048, "READALL", false)],
    ),
    // KnowbaseItem.php:81-82.
    (
        "knowbase",
        &[(1024, "KNOWBASEADMIN", true), (2048, "READFAQ", false)],
    ),
    // Entity.php:72-73.
    (
        "entity",
        &[
            (1024, "READHELPDESK", false),
            (2048, "UPDATEHELPDESK", true),
        ],
    ),
    // Glpi/Inventory/Conf.php:151-152. Both mutate.
    (
        "inventory",
        &[(1024, "IMPORTFROMFILE", true), (2048, "UPDATECONFIG", true)],
    ),
    // ReservationItem.php:55 - creates a reservation, so a write.
    ("reservation", &[(1024, "RESERVEANITEM", true)]),
    // User.php:94-95. Importing users is a write; reading authent is not.
    (
        "user",
        &[
            (1024, "IMPORTEXTAUTHUSERS", true),
            (2048, "READAUTHENT", false),
        ],
    ),
    // ProjectTask.php:82, Project.php:65.
    ("projecttask", &[(1024, "UPDATEMY", true)]),
    ("project", &[(1024, "READALL", false)]),
    // TicketValidation.php:47-48 - both create a validation request.
    (
        "ticketvalidation",
        &[
            (1024, "CREATEREQUEST", true),
            (2048, "CREATEINCIDENT", true),
        ],
    ),
    // PlanningExternalEvent.php:55.
    ("externalevent", &[(1024, "MANAGE_BG_EVENTS", true)]),
    // RuleAsset.php:44 and RuleCommonITILObject.php:42 - PARENT, labelled
    // "Business rules (entity parent)".
    //
    // Classified a READ from how it is enforced rather than from its name:
    // RuleCommonITILObjectCollection.php:67 is
    // `haveRightsOr($rightname, [READ, PARENT])`, so PARENT stands in for
    // READ when deciding whether the collection may be viewed at all, and
    // line 85 uses it to show the inherited-rules tab. Neither mutates.
    //
    // RuleAssetCollection declares `$rightname = 'rule_asset'` but carries no
    // PARENT check of its own; the bit is still declared and offered in
    // RuleAsset::getRights, so it is grantable and worth naming.
    ("rule_asset", &[(1024, "PARENT", false)]),
    ("rule_ticket", &[(1024, "PARENT", false)]),
    ("rule_change", &[(1024, "PARENT", false)]),
    ("rule_problem", &[(1024, "PARENT", false)]),
    // RSSFeed.php:59 and Reminder.php:66 - PERSONAL = 128, not 1024, which
    // is why these override a bit the global table calls UNLOCK.
    ("rssfeed_public", &[(128, "PERSONAL", true)]),
    ("reminder_public", &[(128, "PERSONAL", true)]),
];

/// Whether bits 8-11 of this right are the assignable-item rights.
fn is_assignable(right: &str) -> bool {
    ASSIGNABLE_RIGHTS.contains(&right) || right.starts_with("asset_")
}

/// Every bit this code can put a name to for one right, lowest bit first.
///
/// Scoped by name on purpose - see `GLOBAL_BITS`. Anything this returns no
/// entry for lands in `unnamed_bits`.
fn named_bits(right: &str) -> Vec<BitMeaning> {
    let overrides: &'static [BitMeaning] = RIGHT_OVERRIDES
        .iter()
        .find(|(name, _)| *name == right)
        .map(|(_, bits)| *bits)
        .unwrap_or(&[]);

    let overridden = |bit: i64| overrides.iter().any(|(b, _, _)| *b == bit);

    let mut out: Vec<BitMeaning> = Vec::new();
    for (bit, label) in GLOBAL_BITS {
        if !overridden(bit) {
            out.push((bit, label, bit & WRITE_BITS != 0));
        }
    }
    if is_assignable(right) {
        for (bit, label) in ASSIGNABLE_BITS {
            if !overridden(bit) {
                out.push((bit, label, bit & WRITE_BITS != 0));
            }
        }
    }
    out.extend(overrides.iter().copied());
    out.sort_by_key(|(bit, _, _)| *bit);
    out
}

/// The bits that let the holder change something.
///
/// UNLOCK is counted as one. It is not a standard CRUD bit, but what it grants
/// is clearing the lock on a dynamically-imported field so it can be edited,
/// which is a mutation however it is named.
const WRITE_BITS: i64 = 2 | 4 | 8 | 16 | 64 | 128 | 512 | 2048;

/// One permission bit of a right, with the classification that decides how it
/// is presented.
///
/// `writes` travels with the bit deliberately. The UI colours a bit by whether
/// it grants mutation, and the only authority for that is `WRITE_BITS` here.
/// Letting the frontend infer it from the NAME - every write bit happens to be
/// the ones not called READ* - worked only by coincidence of GLPI's naming, and
/// would have split the moment a read-only bit arrived under some other name:
/// the badge would turn amber while the write-capable count ignored it, two
/// halves of one panel disagreeing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RightBit {
    /// GLPI's constant name, e.g. "READ_OWNED".
    pub name: String,

    /// This bit grants the ability to change something.
    pub writes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RightEntry {
    /// GLPI's own name for the right, e.g. "agent". Not prettified: it is the
    /// string to search for in GLPI's profile form and in its source.
    pub name: String,

    /// The raw bitmask. The authoritative value - everything below is a
    /// reading of it.
    pub value: i64,

    /// `value` decoded through GLPI's global constants, each carrying whether
    /// it is a write. Empty when the right is not granted at all.
    pub bits: Vec<RightBit>,

    /// Whatever `value` holds beyond the bits nameable for THIS right, 0 when
    /// there is none.
    ///
    /// Not an error and not padding. GLPI lets a module define its own bits on
    /// top of the global ones - Ticket and Problem both do, which is why a
    /// live `ticket` arrives as 138241 - and `Session::getRightNameForError`
    /// resolves those through `Profile::getRightsForForm()`, a form definition
    /// with no API route. So these bits are genuine permissions whose meaning
    /// is only knowable per module. They are reported rather than masked off:
    /// dropping them would understate what the account can do, which is the
    /// one direction a permissions screen must never err in.
    ///
    /// Bits 8-11 land here too for any right that is neither in
    /// `ASSIGNABLE_RIGHTS` nor in `RIGHT_OVERRIDES`. That is a deliberate
    /// trade: those bits were previously labelled READ_ASSIGNED,
    /// UPDATE_ASSIGNED, READ_OWNED and UPDATE_OWNED for every right, which is
    /// correct only for assignable items and a confident falsehood everywhere
    /// else. An unnamed bit the UI shows as `+N` says "there is permission
    /// here I cannot name"; a wrong name says something untrue with the same
    /// authority as a right one. The first is the honest failure.
    ///
    /// It does cost something: `writes` cannot see a write hiding in an
    /// unnamed bit, so a right whose only mutation bit is unnamed reads as
    /// read-only. That was already true of bits above 11 - a live `ticket`
    /// arrives as 138241 - and the panel counting these is what flags it.
    pub unnamed_bits: i64,

    /// A known write bit is set, so this right is not read-only.
    pub writes: bool,

    /// Where this right sits in GLPI's profile form: the tab, then the block
    /// within it. None means the committed taxonomy does not cover the right -
    /// see `TAXONOMY_JSON`. Not an error: a few rights genuinely have no row
    /// on the form at all, and after a GLPI upgrade a moved right lands here
    /// too, which is the point.
    pub tab: Option<String>,
    pub group: Option<String>,

    /// GLPI actually stores a `glpi_profilerights` row for this right on this
    /// profile.
    ///
    /// False means there is no row. That is not the same statement as
    /// `value == 0`, though it has the same effect: `Session::haveRight`
    /// (Session.php:1498-1502) is
    ///
    ///   if (isset($_SESSION["glpiactiveprofile"][$module])) {
    ///       return (int) $_SESSION["glpiactiveprofile"][$module] & $right;
    ///   }
    ///   return false;
    ///
    /// so an absent row denies exactly as a zero does. It is carried because
    /// the two differ to a reader: a stored zero is a decision someone made in
    /// the profile form, an absent row is a right that form never offered for
    /// this profile. On a stock install five of the nine profiles have no row
    /// for `rule_change` or `rule_problem`.
    pub recorded: bool,
}

impl RightEntry {
    /// A right GLPI has no row for on this profile. Denied, like a zero, but
    /// not a zero somebody chose.
    fn absent(name: String) -> Self {
        Self {
            name,
            value: 0,
            bits: Vec::new(),
            unnamed_bits: 0,
            writes: false,
            recorded: false,
            tab: None,
            group: None,
        }
    }

    fn new(name: String, value: i64) -> Self {
        // Scoped by right name, not a global table: the same bit means
        // different things under different names. See `GLOBAL_BITS`.
        let named = named_bits(&name);

        let bits: Vec<RightBit> = named
            .iter()
            .filter(|(bit, _, _)| value & bit != 0)
            .map(|(_, label, writes)| RightBit {
                name: (*label).to_string(),
                writes: *writes,
            })
            .collect();

        // Derived from the bits actually named rather than from a mask over
        // the raw value, which is what keeps a bit's colour and the right's
        // verdict from disagreeing now that a bit's write-ness depends on
        // which right it belongs to.
        let writes = bits.iter().any(|b| b.writes);

        let named_mask = named.iter().fold(0i64, |acc, (bit, _, _)| acc | bit);

        Self {
            name,
            value,
            bits,
            unnamed_bits: value & !named_mask,
            writes,
            recorded: true,
            tab: None,
            group: None,
        }
    }
}

/// GLPI's own right -> (tab, group) map, committed as data.
///
/// Generated by `just right-taxonomy`, which calls
/// `Profile::getRightsForForm()` - the function that renders the profile form.
/// It is not reachable through any API (not v2.3, not v1, not
/// /api.php/doc.json, no console command). It CAN be parsed out of
/// src/Profile.php - an earlier version of this comment claimed five classes
/// compute their $rightname at runtime, which is false; they are static
/// literals a naive regex misses, through double quotes, `self::class` and
/// one level of inheritance. The dump is preferred because it calls the
/// function that renders the form, so it cannot drift from what the form
/// shows. See scripts/dump-glpi-right-taxonomy.php.
///
/// Empty is a valid state. It means the dump has not been run, and the Rights
/// page falls back to the flat alphabetical list rather than showing one
/// enormous ungrouped heading.
///
/// This is the one piece of this app coupled to GLPI's source rather than its
/// API, so the coupling is made to announce itself: `RightsReport` compares
/// these keys against the right names GLPI actually reports and surfaces both
/// directions of mismatch.
const TAXONOMY_JSON: &str = include_str!("right_taxonomy.json");

#[derive(Debug, Clone, Deserialize)]
struct TaxonomyEntry {
    right: String,
    interface: String,
    tab: String,
    group: String,
}

/// Parsed once. A malformed committed file is a build-time mistake, but it must
/// not take the page down at runtime: it degrades to an empty taxonomy, which
/// is the flat fallback.
fn taxonomy() -> &'static [TaxonomyEntry] {
    static PARSED: std::sync::OnceLock<Vec<TaxonomyEntry>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| match serde_json::from_str(TAXONOMY_JSON) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                "right_taxonomy.json is not readable ({e}); the Rights page will not group. \
                 Regenerate it with `just right-taxonomy`."
            );
            Vec::new()
        }
    })
}

/// The tab and group for one right, preferring the profile's own interface.
///
/// A right can appear on both the central and helpdesk forms under different
/// headings, so the interface matters. Falling back to any interface is
/// deliberate: a heading from the wrong interface is still the right heading
/// for a reader looking for the setting, whereas no heading sends the right to
/// the ungrouped pile for no reason.
fn placement(right: &str, interface: Option<&str>) -> Option<(String, String)> {
    placement_in(taxonomy(), right, interface)
}

/// Split out from `placement` so it can be tested against a map of known
/// shape instead of whatever happens to be committed.
fn placement_in(
    all: &[TaxonomyEntry],
    right: &str,
    interface: Option<&str>,
) -> Option<(String, String)> {
    all.iter()
        .find(|t| t.right == right && Some(t.interface.as_str()) == interface)
        .or_else(|| all.iter().find(|t| t.right == right))
        .map(|t| (t.tab.clone(), t.group.clone()))
}

/// The rights of one GLPI profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileRights {
    pub id: i64,
    pub name: Option<String>,

    /// "central" or "helpdesk". Worth carrying: the rights this app needs
    /// cannot exist on a helpdesk profile at all - the section is absent from
    /// GLPI's form rather than merely unticked.
    pub interface: Option<String>,

    /// GLPI hands this profile to accounts that have no explicit default.
    pub is_default: bool,

    /// The profile this app's own session is running under, so the page can
    /// say which of these is the one actually in force.
    pub is_active: bool,

    /// Sorted by name. Includes rights whose value is 0 - "this right exists
    /// and this profile does not have it" is the answer to a 403, and omitting
    /// it leaves a reader unable to tell an ungranted right from one GLPI has
    /// never heard of.
    pub entries: Vec<RightEntry>,
}

/// One call this app makes to GLPI, and the rights it depends on.
///
/// This is the app describing itself, not a fact read out of GLPI: the call
/// list is `glpi.get(..)` and `legacy.get(..)` in api/mod.rs. What IS read out
/// of GLPI 11.0.12 is which right each call needs, and that chain is worth
/// recording because none of it is guessable from the endpoint name:
///
///   * HL authorises a collection with `$itemtype::canView()`
///     (ResourceAccessor.php:353), and each schema names its itemtype via
///     `x-itemtype`. Each itemtype declares its own right: Agent is `agent`,
///     Computer is `computer`, Entity is `entity`, Profile is `profile`,
///     User is `user`.
///   * `ProfileRight` declares no right of its own. It is a CommonDBChild of
///     Profile, and CommonDBChild::canView (CommonDBChild.php:130-136) falls
///     through to `canChild('canView')`, so it needs READ on `profile`.
///   * `Profile_User` is a CommonDBRelation between User and Profile, and
///     CommonDBRelation::canView (CommonDBRelation.php:634-641) calls
///     `canRelation('canView', true)` - the comment there reads "Always both
///     checks for view" - so it needs READ on `user` AND on `profile`. A
///     breakdown that named only one of them would send a reader to grant the
///     wrong half.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiCall {
    /// The path as this app requests it, e.g. "Inventory/Agent".
    pub path: String,

    /// Which GLPI API: "v2.3" or "v1".
    pub api: String,

    /// The features that stop working if this call fails.
    ///
    /// A list, not a sentence. One call commonly serves several - losing
    /// `agent` takes out the Agents page AND the orphan column on the
    /// Computers page - and that fan-out is the thing a reader needs to see.
    /// As prose it was invisible to the UI and could not be grouped by.
    pub features: Vec<CallFeature>,

    /// Every right this call needs READ on. More than one means ALL of them
    /// are required, not any.
    pub needs: Vec<String>,
}

/// One feature a call serves, and what it does for it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallFeature {
    /// Named as the reader would name it - the page or the element on it, not
    /// the handler. Someone arrives at this panel thinking "the Computers page
    /// is empty", never "Assets/Computer returned 403".
    pub name: String,

    /// What this call contributes to that feature, where it is not the whole
    /// of it. `Inventory/Agent` is the Agents page outright, but only the
    /// orphan check on the Computers page.
    pub role: Option<String>,
}

/// One required right, resolved against a profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallRight {
    pub name: String,

    /// The profile's value, or None when GLPI records no entry for it.
    pub value: Option<i64>,

    /// READ is held.
    pub satisfied: bool,
}

/// An `ApiCall` judged against one profile's rights.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallStatus {
    pub path: String,
    pub api: String,
    pub features: Vec<CallFeature>,
    pub rights: Vec<CallRight>,

    /// Every right this call needs is held, so the call should not 403.
    ///
    /// "Should not" and not "will not": a right is necessary, never
    /// sufficient. GLPI still filters by entity afterwards, so a call can
    /// return an empty collection with every right in place.
    pub satisfied: bool,
}

/// READ, the only bit any of this app's calls needs. It never writes.
const READ: i64 = 1;

/// Every GLPI call this app makes.
///
/// Hand-maintained deliberately, and the only honest way round: it is this
/// app's own behaviour, so no API can report it. Adding a call to api/mod.rs
/// without adding it here leaves the page understating what the app needs -
/// which is why it sits beside the rights code rather than in api/mod.rs,
/// where it would read as documentation.
pub fn api_calls() -> Vec<ApiCall> {
    // Feature order on the page follows first appearance here, so this list is
    // also the running order of the breakdown. One list to keep current rather
    // than a second ordering table that could disagree with it.
    let call = |path: &str, api: &str, needs: &[&str], features: &[(&str, Option<&str>)]| ApiCall {
        path: path.to_string(),
        api: api.to_string(),
        needs: needs.iter().map(|s| (*s).to_string()).collect(),
        features: features
            .iter()
            .map(|(name, role)| CallFeature {
                name: (*name).to_string(),
                role: role.map(str::to_string),
            })
            .collect(),
    };

    const AGENTS: &str = "Agents page";
    const COMPUTERS: &str = "Computers page";
    const SCOPE: &str = "Scope line under every table";
    const STATUS: &str = "Status page";
    const RIGHTS: &str = "Rights page";
    const AUTHS: &str = "Authorizations table";
    const SEC: &str = "Security page";

    vec![
        // Serves two features, which is exactly the fan-out the old prose
        // column could state but not structure.
        call(
            "Inventory/Agent",
            "v2.3",
            &["agent"],
            &[
                (AGENTS, None),
                (COMPUTERS, Some("orphan check")),
                (SEC, Some("the agent-version check")),
            ],
        ),
        call(
            "Assets/Computer",
            "v2.3",
            &["computer"],
            &[(COMPUTERS, None)],
        ),
        // Added once the Security page began reading it. This list is
        // hand-maintained, so this is exactly the drift the module doc warns
        // about: it was found by grepping every glpi.get in api/mod.rs
        // against this list, not by anything failing. The right it needs was
        // already required elsewhere, so nothing was broken - the page was
        // simply understating what the app asks for.
        call(
            "Administration/User",
            "v2.3",
            &["user"],
            &[(SEC, Some("the shipped-account and stale-login checks"))],
        ),
        call(
            "Administration/Entity",
            "v2.3",
            &["entity"],
            &[(SCOPE, None)],
        ),
        // No right: /session reports the caller's own session. Listed anyway,
        // because a reader counting the app's calls would wonder where it
        // went, and "needs nothing" is itself the answer.
        call(
            "session",
            "v2.3",
            &[],
            &[
                (STATUS, None),
                (RIGHTS, Some("which profile is active")),
                (SEC, Some("the active profile's own rights")),
            ],
        ),
        // Both of these are read twice over: once to build this page, once by
        // the authorizations handler. Verified against the call sites in
        // api/mod.rs, not assumed from their names.
        call(
            "Profile",
            "v1",
            &["profile"],
            &[(RIGHTS, Some("the profile list")), (AUTHS, None)],
        ),
        call(
            "ProfileRight",
            "v1",
            &["profile"],
            &[(RIGHTS, Some("every right shown")), (AUTHS, None)],
        ),
        call("Profile_User", "v1", &["user", "profile"], &[(AUTHS, None)]),
        call(
            "Entity",
            "v1",
            &["entity"],
            &[(AUTHS, Some("entity names"))],
        ),
        call(
            "getFullSession",
            "v1",
            &[],
            &[(AUTHS, Some("which account this app is"))],
        ),
    ]
}

/// Every profile in GLPI with its full rights.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RightsReport {
    pub active_profile_id: Option<i64>,
    pub profiles: Vec<ProfileRights>,

    /// This app's own calls, judged against the ACTIVE profile - the only one
    /// that affects whether the app works. Empty when no active profile could
    /// be identified, rather than judged against an arbitrary one.
    pub calls: Vec<CallStatus>,

    /// How many right names the committed taxonomy covers. Zero means the dump
    /// has never been run, and the page shows the flat list instead of one
    /// giant ungrouped heading.
    pub taxonomy_rights: usize,

    /// Rights GLPI's profile form offers that no profile records a value for.
    ///
    /// NOT a drift signal, though it looks like one. GLPI ships at least one
    /// such right: `DefaultFilter::$rightname = 'defaultfilter'` is on the
    /// form, yet `glpi-empty.sql` seeds zero `defaultfilter` rows and no
    /// migration adds any. `Session::haveRight` denies an unrecorded right, so
    /// these are rights nobody holds and nobody can be granted through a
    /// stored value - worth stating, not worth alarming about.
    ///
    /// The real drift signal is the opposite direction: rights GLPI reports
    /// that the map cannot place, which land under "Not on the profile form".
    /// A forgotten `just right-taxonomy` after an upgrade shows up there,
    /// because a newly added right gets its profilerights rows from the
    /// upgrade migration while the committed map still predates it.
    pub form_only_rights: Vec<String>,
}

impl RightsReport {
    /// Build from v1's `Profile` and `ProfileRight` collections.
    ///
    /// `row_limit` is the number of rows the caller asked v1 for. v1 truncates
    /// a collection at the requested range and says so only in a header the
    /// legacy client does not surface, so reaching the limit is treated as
    /// truncation and refused. A short rights list is the worst possible
    /// output here: a right missing because it was never fetched looks exactly
    /// like a right the profile does not have.
    pub fn from_legacy(
        profiles: &[serde_json::Value],
        rights: &[serde_json::Value],
        active_profile_id: Option<i64>,
        row_limit: usize,
    ) -> anyhow::Result<Self> {
        if rights.len() >= row_limit {
            anyhow::bail!(
                "GLPI v1 returned {} ProfileRight rows for a requested limit of {}, so the \
                 collection is probably truncated. Refusing rather than showing part of a \
                 profile's rights as all of them - raise the range in the caller.",
                rights.len(),
                row_limit
            );
        }

        let num = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_i64());
        let text =
            |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);

        // The denominator, and it has to be the same one for every profile.
        //
        // GLPI does not store a row per profile per right - it stores only the
        // rows that exist, and five of the nine profiles on a stock install
        // have no row for `rule_change` or `rule_problem`. Counting each
        // profile's own rows would give them a quieter denominator than the
        // rest (99 against 101) and silently make the fractions
        // incomparable: "9 of 99" beside "100 of 101" invites a reader to
        // compare two things that are not the same measurement.
        //
        // So the union of every right name seen anywhere is used, and a
        // profile with no row for one of them gets an explicitly absent entry.
        // That is faithful to GLPI, which denies an absent right exactly as it
        // denies a zero - see RightEntry::recorded.
        let mut all_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for r in rights {
            if let Some(name) = text(r, "name") {
                all_names.insert(name);
            }
        }

        let mut out: Vec<ProfileRights> = profiles
            .iter()
            .filter_map(|p| {
                let id = num(p, "id")?;

                // This profile's own rows, by name.
                let mut stored: std::collections::HashMap<String, i64> =
                    std::collections::HashMap::new();
                for r in rights.iter().filter(|r| num(r, "profiles_id") == Some(id)) {
                    // A row with no name is not a right that can be shown.
                    let Some(name) = text(r, "name") else {
                        continue;
                    };
                    // v1 sends these as integers, but a quoted number survives
                    // a JSON round trip just as easily; parsing both avoids
                    // scoring a granted right as missing.
                    let value = r
                        .get("rights")
                        .and_then(|x| {
                            x.as_i64()
                                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
                        })
                        .unwrap_or(0);
                    stored.insert(name, value);
                }

                let interface = text(p, "interface");

                // One entry per right that exists anywhere, in name order -
                // BTreeSet is already sorted, and this list is read by someone
                // hunting one particular right, usually off the back of a 403
                // that would not name it.
                let entries: Vec<RightEntry> = all_names
                    .iter()
                    .map(|name| {
                        let mut e = match stored.get(name) {
                            Some(value) => RightEntry::new(name.clone(), *value),
                            None => RightEntry::absent(name.clone()),
                        };
                        if let Some((tab, group)) = placement(name, interface.as_deref()) {
                            e.tab = Some(tab);
                            e.group = Some(group);
                        }
                        e
                    })
                    .collect();

                Some(ProfileRights {
                    id,
                    name: text(p, "name"),
                    interface,
                    is_default: num(p, "is_default").unwrap_or(0) != 0,
                    is_active: Some(id) == active_profile_id,
                    entries,
                })
            })
            .collect();

        out.sort_by_key(|p| p.id);

        let calls = match out.iter().find(|p| p.is_active) {
            Some(active) => api_calls()
                .into_iter()
                .map(|c| {
                    let rights: Vec<CallRight> = c
                        .needs
                        .iter()
                        .map(|name| {
                            let entry = active.entries.iter().find(|e| &e.name == name);
                            // An absent entry is None, not Some(0): GLPI denies
                            // both, but only one is a value somebody set. See
                            // RightEntry::recorded.
                            let value = entry.filter(|e| e.recorded).map(|e| e.value);
                            CallRight {
                                name: name.clone(),
                                value,
                                satisfied: entry.map(|e| e.value & READ != 0).unwrap_or(false),
                            }
                        })
                        .collect();
                    CallStatus {
                        path: c.path,
                        api: c.api,
                        features: c.features,
                        // A call needing nothing is satisfied: `all` on an
                        // empty iterator is true, which is the right answer
                        // here rather than an accident of the fold.
                        satisfied: rights.iter().all(|r| r.satisfied),
                        rights,
                    }
                })
                .collect(),
            None => Vec::new(),
        };

        // Computed against the names GLPI actually reported rather than
        // against anything this app believes.
        let taxonomy_names: std::collections::BTreeSet<&str> =
            taxonomy().iter().map(|t| t.right.as_str()).collect();
        let taxonomy_rights = taxonomy_names.len();
        let form_only_rights: Vec<String> = taxonomy_names
            .iter()
            .filter(|n| !all_names.contains(**n))
            .map(|n| (*n).to_string())
            .collect();

        Ok(Self {
            active_profile_id,
            profiles: out,
            calls,
            taxonomy_rights,
            form_only_rights,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shapes copied from live v1 responses.
    fn profile(id: i64, name: &str, interface: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": name, "interface": interface, "is_default": 0 })
    }
    fn right(profiles_id: i64, name: &str, rights: i64) -> serde_json::Value {
        serde_json::json!({ "id": 1, "profiles_id": profiles_id, "name": name, "rights": rights })
    }

    const LIMIT: usize = 10_000;

    /// Bit names only, for assertions that are about decoding rather than
    /// classification.
    fn names(bits: &[RightBit]) -> Vec<&str> {
        bits.iter().map(|b| b.name.as_str()).collect()
    }

    /// Values taken from a live install, not invented, so this pins the
    /// decoding against the real thing.
    #[test]
    fn decodes_the_values_a_live_glpi_returns() {
        let profiles = vec![profile(9, "inv", "central")];
        let rights = vec![
            right(9, "agent", 1),
            right(9, "entity", 33),
            right(9, "computer", 1313),
            right(9, "config", 0),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, Some(9), LIMIT).unwrap();
        let p = &r.profiles[0];
        assert_eq!(p.id, 9);
        assert_eq!(p.interface.as_deref(), Some("central"));
        assert!(p.is_active);

        let by = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().clone();

        let agent = by("agent");
        assert_eq!(names(&agent.bits), vec!["READ"]);
        assert!(!agent.writes);
        assert_eq!(agent.unnamed_bits, 0);

        // 33 = READ | READNOTE, and a note is not a write.
        assert_eq!(names(&by("entity").bits), vec!["READ", "READNOTE"]);
        assert!(!by("entity").writes);

        // 1313 = 1 | 32 | 256 | 1024, all four of them read bits.
        assert_eq!(
            names(&by("computer").bits),
            vec!["READ", "READNOTE", "READ_ASSIGNED", "READ_OWNED"]
        );
        assert!(!by("computer").writes);

        // Not granted, and still listed.
        assert_eq!(by("config").value, 0);
        assert!(by("config").bits.is_empty());
    }

    /// The point of using v1: every profile, including ones this account is
    /// not authorised for and therefore cannot reach through /session.
    #[test]
    fn every_profile_gets_its_own_rights() {
        let profiles = vec![
            profile(9, "inv", "central"),
            profile(4, "Super-Admin", "central"),
            profile(1, "Self-Service", "helpdesk"),
        ];
        let rights = vec![
            right(9, "computer", 1),
            right(4, "computer", 31),
            right(1, "ticket", 1),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, Some(9), LIMIT).unwrap();

        // Sorted by id, so the order is stable for a reader.
        assert_eq!(
            r.profiles.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![1, 4, 9]
        );

        let by_id = |id: i64| r.profiles.iter().find(|p| p.id == id).unwrap().clone();

        // Each profile keeps its OWN value - the failure mode of the /session
        // route was every profile showing the active one's rights.
        assert_eq!(by_id(9).entries[0].value, 1);
        assert_eq!(by_id(4).entries[0].value, 31);
        assert!(by_id(4).entries[0].writes, "Super-Admin can write");
        assert!(!by_id(9).entries[0].writes);

        // Only the session's profile is marked active.
        assert!(by_id(9).is_active);
        assert!(!by_id(4).is_active);
        assert_eq!(by_id(1).interface.as_deref(), Some("helpdesk"));
    }

    /// A right's VALUE must never leak between profiles.
    ///
    /// Both profiles list both rights, because the denominator is the union -
    /// but each one's value belongs to it alone, and the right it has no row
    /// for reads as absent rather than borrowing the other's number.
    #[test]
    fn rights_are_not_mixed_between_profiles() {
        let profiles = vec![profile(1, "a", "central"), profile(2, "b", "central")];
        let rights = vec![right(1, "computer", 1), right(2, "ticket", 2)];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();

        let get = |pid: i64, n: &str| {
            r.profiles
                .iter()
                .find(|p| p.id == pid)
                .unwrap()
                .entries
                .iter()
                .find(|e| e.name == n)
                .unwrap()
                .clone()
        };

        // Profile 1 owns `computer` and has no row for `ticket`.
        assert_eq!(get(1, "computer").value, 1);
        assert!(get(1, "computer").recorded);
        assert_eq!(
            get(1, "ticket").value,
            0,
            "must not borrow profile 2's value"
        );
        assert!(!get(1, "ticket").recorded);

        // And the mirror image.
        assert_eq!(get(2, "ticket").value, 2);
        assert!(get(2, "ticket").recorded);
        assert_eq!(get(2, "computer").value, 0);
        assert!(!get(2, "computer").recorded);
    }

    /// Every profile must be counted against the same set of rights.
    ///
    /// GLPI stores only the rows that exist: on a stock install `rule_change`
    /// and `rule_problem` have no row on five of the nine profiles. Counting
    /// each profile's own rows would hand them a denominator of 99 against the
    /// others' 101 and make the fractions quietly incomparable.
    #[test]
    fn every_profile_is_counted_against_the_same_rights() {
        let profiles = vec![
            profile(1, "Self-Service", "helpdesk"),
            profile(4, "Super-Admin", "central"),
        ];
        let rights = vec![
            // Only Super-Admin has a row for rule_change.
            right(4, "rule_change", 1),
            right(4, "computer", 31),
            right(1, "computer", 0),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();

        let a = r.profiles.iter().find(|p| p.id == 1).unwrap();
        let b = r.profiles.iter().find(|p| p.id == 4).unwrap();
        assert_eq!(
            a.entries.len(),
            b.entries.len(),
            "the two profiles must share one denominator"
        );
        assert_eq!(a.entries.len(), 2, "the union of right names is the total");

        // The right with no row is present, denied, and marked unrecorded.
        let rc = a.entries.iter().find(|e| e.name == "rule_change").unwrap();
        assert!(
            !rc.recorded,
            "there is no row for this right on this profile"
        );
        assert_eq!(rc.value, 0, "an absent right denies exactly as a zero does");
        assert!(rc.bits.is_empty());
        assert!(!rc.writes);

        // A stored zero is a different statement from an absent row, even
        // though both deny.
        let cz = a.entries.iter().find(|e| e.name == "computer").unwrap();
        assert!(cz.recorded, "this profile does have a row, holding zero");
        assert_eq!(cz.value, 0);

        // And the profile that does have the row is unaffected.
        let rc_b = b.entries.iter().find(|e| e.name == "rule_change").unwrap();
        assert!(rc_b.recorded);
        assert_eq!(rc_b.value, 1);
    }

    /// Entries are in name order for every profile, so two profiles can be
    /// read side by side.
    #[test]
    fn entries_are_in_name_order() {
        let profiles = vec![profile(1, "a", "central")];
        let rights = vec![
            right(1, "ticket", 1),
            right(1, "agent", 1),
            right(1, "computer", 1),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let names: Vec<&str> = r.profiles[0]
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, vec!["agent", "computer", "ticket"]);
    }

    /// The app's calls are judged against the ACTIVE profile only - that is
    /// the one that decides whether the dashboard works.
    #[test]
    fn calls_are_judged_against_the_active_profile() {
        let profiles = vec![
            profile(4, "Super-Admin", "central"),
            profile(9, "inv", "central"),
        ];
        let rights = vec![
            // Super-Admin can do everything; inv can read agents but NOT
            // computers. The verdict must follow inv, which is active.
            right(4, "agent", 31),
            right(4, "computer", 31),
            right(9, "agent", 1),
            right(9, "computer", 0),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, Some(9), LIMIT).unwrap();

        let call = |p: &str| r.calls.iter().find(|c| c.path == p).unwrap().clone();
        assert!(call("Inventory/Agent").satisfied, "inv holds agent READ");
        assert!(
            !call("Assets/Computer").satisfied,
            "inv holds computer 0, so this call must be reported as blocked"
        );
    }

    /// A call needing several rights is satisfied only when ALL of them are
    /// held. Profile_User needs user AND profile - CommonDBRelation::canView
    /// checks both - and reporting it on one would send a reader to grant the
    /// wrong half.
    #[test]
    fn a_call_needing_two_rights_needs_both() {
        let profiles = vec![profile(1, "p", "central")];
        let partial = vec![right(1, "profile", 1), right(1, "user", 0)];
        let r = RightsReport::from_legacy(&profiles, &partial, Some(1), LIMIT).unwrap();
        let pu = r.calls.iter().find(|c| c.path == "Profile_User").unwrap();
        assert_eq!(pu.rights.len(), 2, "both rights must be reported");
        assert!(!pu.satisfied, "one of two held is not enough");
        // And the reader is told WHICH half is missing.
        let missing: Vec<&str> = pu
            .rights
            .iter()
            .filter(|x| !x.satisfied)
            .map(|x| x.name.as_str())
            .collect();
        assert_eq!(missing, vec!["user"]);

        let both = vec![right(1, "profile", 1), right(1, "user", 1)];
        let r = RightsReport::from_legacy(&profiles, &both, Some(1), LIMIT).unwrap();
        assert!(
            r.calls
                .iter()
                .find(|c| c.path == "Profile_User")
                .unwrap()
                .satisfied
        );
    }

    /// A call that needs no right is satisfied, not blocked. /session reports
    /// the caller's own session; an empty requirement list must not read as
    /// "nothing is granted".
    #[test]
    fn a_call_needing_no_right_is_satisfied() {
        let profiles = vec![profile(1, "p", "central")];
        let r =
            RightsReport::from_legacy(&profiles, &[right(1, "agent", 0)], Some(1), LIMIT).unwrap();
        let sess = r.calls.iter().find(|c| c.path == "session").unwrap();
        assert!(sess.rights.is_empty());
        assert!(sess.satisfied);
    }

    /// With no active profile there is nothing to judge against, so no verdict
    /// is invented from whichever profile happened to sort first.
    #[test]
    fn no_active_profile_yields_no_verdicts() {
        let profiles = vec![profile(1, "p", "central")];
        let r = RightsReport::from_legacy(&profiles, &[right(1, "agent", 1)], None, LIMIT).unwrap();
        assert!(r.calls.is_empty());
    }

    /// Every right the call list names must be a right GLPI actually has, or
    /// the page would report a dependency on something that cannot be granted.
    /// Guards against a typo in `api_calls` and against GLPI renaming one.
    #[test]
    fn every_right_the_call_list_names_is_a_real_right() {
        // The live names, from the install this was written against.
        let known = ["agent", "computer", "entity", "profile", "user"];
        for c in api_calls() {
            for name in &c.needs {
                assert!(
                    known.contains(&name.as_str()),
                    "call {} names right {:?}, which is not a known GLPI right",
                    c.path,
                    name
                );
            }
        }
    }

    fn tax(right: &str, interface: &str, tab: &str, group: &str) -> TaxonomyEntry {
        TaxonomyEntry {
            right: right.to_string(),
            interface: interface.to_string(),
            tab: tab.to_string(),
            group: group.to_string(),
        }
    }

    /// A right's heading is taken from the profile's OWN interface first: the
    /// same right sits under different headings on the central and helpdesk
    /// forms.
    #[test]
    fn placement_prefers_the_profiles_own_interface() {
        let map = vec![
            tax("ticket", "central", "tracking", "tickets"),
            tax("ticket", "helpdesk", "tracking", "general"),
        ];
        assert_eq!(
            placement_in(&map, "ticket", Some("central")),
            Some(("tracking".into(), "tickets".into()))
        );
        assert_eq!(
            placement_in(&map, "ticket", Some("helpdesk")),
            Some(("tracking".into(), "general".into()))
        );
    }

    /// With no match for the profile's interface, any interface is better than
    /// none: a heading from the other form still points a reader at the right
    /// setting, where no heading just hides it in the ungrouped pile.
    #[test]
    fn placement_falls_back_to_another_interface() {
        let map = vec![tax("agent", "central", "admin", "inventory")];
        assert_eq!(
            placement_in(&map, "agent", Some("helpdesk")),
            Some(("admin".into(), "inventory".into()))
        );
        assert_eq!(
            placement_in(&map, "agent", None),
            Some(("admin".into(), "inventory".into()))
        );
    }

    /// A right the map does not cover gets no heading - it must not be guessed
    /// into a neighbouring group.
    #[test]
    fn an_uncovered_right_gets_no_placement() {
        let map = vec![tax("agent", "central", "admin", "inventory")];
        assert_eq!(placement_in(&map, "backup", Some("central")), None);
        assert!(placement_in(&[], "agent", Some("central")).is_none());
    }

    /// The committed map must parse. An empty one is legal - it means the dump
    /// has not been run - but a malformed one would silently stop all
    /// grouping, so it is worth failing the build over.
    #[test]
    fn the_committed_taxonomy_parses() {
        let parsed: Result<Vec<TaxonomyEntry>, _> = serde_json::from_str(TAXONOMY_JSON);
        assert!(
            parsed.is_ok(),
            "backend/src/glpi/right_taxonomy.json is malformed: {:?}",
            parsed.err()
        );
    }

    /// With no taxonomy committed, nothing is grouped and nothing is claimed
    /// stale - the page falls back to the flat list. This is the state the repo
    /// ships in until `just right-taxonomy` is run.
    #[test]
    fn an_empty_taxonomy_groups_nothing_and_reports_no_drift() {
        let profiles = vec![profile(9, "inv", "central")];
        let rights = vec![right(9, "agent", 1)];
        let r = RightsReport::from_legacy(&profiles, &rights, Some(9), LIMIT).unwrap();
        if r.taxonomy_rights == 0 {
            assert!(r.profiles[0].entries.iter().all(|e| e.tab.is_none()));
            assert!(
                r.form_only_rights.is_empty(),
                "an empty map cannot offer a form-only right"
            );
        } else {
            // The dump has been run. Then every covered right must carry BOTH
            // halves of its heading, never one.
            for e in &r.profiles[0].entries {
                assert_eq!(
                    e.tab.is_some(),
                    e.group.is_some(),
                    "right {:?} has half a heading",
                    e.name
                );
            }
        }
    }

    /// The per-bit flag and the right's own verdict come from one mask, and
    /// this pins them together. The bug this replaces was the frontend
    /// deciding a bit's colour from whether its NAME began with READ, which
    /// agreed with the mask only by accident of GLPI's naming.
    #[test]
    fn per_bit_write_flags_agree_with_the_rights_verdict() {
        let profiles = vec![profile(1, "p", "central")];
        // Every value 0..4096 exercises every combination of the twelve
        // low bits. These names are in no override table and are not
        // assignable, so bits 8-11 arrive unnamed - which is itself the
        // property being pinned: the invariant must hold for a right this
        // code knows nothing about.
        let rights: Vec<serde_json::Value> =
            (0..4096).map(|v| right(1, &format!("r{v}"), v)).collect();
        let r = RightsReport::from_legacy(&profiles, &rights, Some(1), 8192).unwrap();

        for e in &r.profiles[0].entries {
            assert_eq!(
                e.bits.iter().any(|b| b.writes),
                e.writes,
                "right {:?} (value {}) classifies its bits and itself differently",
                e.name,
                e.value
            );
            // And no READ-named bit is ever a write, nor the reverse - the
            // property the old string test relied on, now asserted rather
            // than assumed.
            for b in &e.bits {
                assert_eq!(
                    !b.name.starts_with("READ"),
                    b.writes,
                    "bit {:?} is named and classified inconsistently",
                    b.name
                );
            }
        }
    }

    /// Every call must name at least one feature, or it appears in the
    /// breakdown under no heading and is effectively invisible.
    #[test]
    fn every_call_names_a_feature() {
        for c in api_calls() {
            assert!(
                !c.features.is_empty(),
                "call {} serves no named feature, so nothing would show it",
                c.path
            );
        }
    }

    /// The fan-out the prose column could not express: a call serving two
    /// features must appear under both, so one missing right visibly breaks
    /// more than one page.
    #[test]
    fn a_call_serving_two_features_is_listed_under_both() {
        let calls = api_calls();
        let agent = calls
            .iter()
            .find(|c| c.path == "Inventory/Agent")
            .expect("Inventory/Agent is the app's core call");
        let names: Vec<&str> = agent.features.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"Agents page") && names.contains(&"Computers page"),
            "the agent call feeds both pages; got {names:?}"
        );
        // And the secondary use says what it is, rather than implying the
        // whole page depends on it in the same way.
        let computers = agent
            .features
            .iter()
            .find(|f| f.name == "Computers page")
            .unwrap();
        assert_eq!(computers.role.as_deref(), Some("orphan check"));
        assert!(
            agent
                .features
                .iter()
                .find(|f| f.name == "Agents page")
                .unwrap()
                .role
                .is_none(),
            "the Agents page IS this call, so it needs no qualifier"
        );
    }

    /// A feature name used by two different calls must be spelled identically,
    /// or the breakdown splits one page into two headings.
    #[test]
    fn feature_names_are_consistent_across_calls() {
        let calls = api_calls();
        let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for c in &calls {
            for f in &c.features {
                *seen.entry(f.name.clone()).or_default() += 1;
            }
        }
        // Spot the near-miss spelling: any two names differing only by case or
        // trailing whitespace would be a split heading.
        let names: Vec<String> = seen.keys().cloned().collect();
        for (i, a) in names.iter().enumerate() {
            for b in names.iter().skip(i + 1) {
                assert_ne!(
                    a.trim().to_lowercase(),
                    b.trim().to_lowercase(),
                    "feature names {a:?} and {b:?} differ only cosmetically and would \
                     render as two headings for one page"
                );
            }
        }
        // The Rights page is served by several calls; if that collapsed to one
        // the grouping would have lost its point.
        assert!(
            seen.get("Rights page").copied().unwrap_or(0) >= 3,
            "the Rights page depends on session, Profile and ProfileRight"
        );
    }

    /// v1 truncates at the requested range and reports it only in a header the
    /// legacy client does not surface. A right missing because it was never
    /// fetched is indistinguishable from one the profile does not have, so
    /// hitting the limit must fail loudly.
    #[test]
    fn a_truncated_rights_collection_is_refused() {
        let profiles = vec![profile(1, "a", "central")];
        let rights: Vec<serde_json::Value> =
            (0..5).map(|i| right(1, &format!("r{i}"), 1)).collect();
        let err = RightsReport::from_legacy(&profiles, &rights, None, 5)
            .expect_err("reaching the row limit must not pass as a complete collection");
        let msg = format!("{err:#}");
        assert!(msg.contains("truncated"), "unhelpful message: {msg}");

        // One row below the limit is complete and must pass.
        assert!(RightsReport::from_legacy(&profiles, &rights, None, 6).is_ok());
    }

    /// The property that matters most in the decoding: a bit this code cannot
    /// name must be reported, never masked off. 132129 is a live `change`
    /// right - 1 | 32 | 1024 plus 131072, which belongs to the ITIL module's
    /// own definitions.
    #[test]
    fn a_bit_outside_the_global_constants_is_reported_not_dropped() {
        let profiles = vec![profile(9, "inv", "central")];
        let rights = vec![right(9, "change", 132129), right(9, "task", 8193)];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let by = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().clone();

        let change = by("change");
        assert_eq!(change.value, 132129);
        // Bit 10 is READALL here (CommonITILObject.php:143), NOT READ_OWNED.
        // This asserted READ_OWNED until the decode became name-scoped, which
        // is the clearest statement of what that bug was: a label that read
        // as authoritative and was wrong for every right but the assignable
        // ones.
        assert_eq!(names(&change.bits), vec!["READ", "READNOTE", "READALL"]);
        assert_eq!(
            change.unnamed_bits, 131072,
            "an unnameable bit must survive into the output"
        );

        // 8193 = READ plus 8192, which no global constant covers.
        assert_eq!(by("task").unnamed_bits, 8192);
        assert_eq!(names(&by("task").bits), vec!["READ"]);
    }

    /// The same bit, three different readings, decided by the right's name.
    #[test]
    fn a_bits_meaning_follows_the_right_name() {
        let profiles = vec![profile(1, "p", "central")];
        let rights = vec![
            right(1, "computer", 1024),      // assignable item
            right(1, "ticket", 1024),        // CommonITILObject::READALL
            right(1, "planning", 1024),      // Planning::READGROUP - inverted
            right(1, "knowbase", 1024),      // KnowbaseItem::KNOWBASEADMIN
            right(1, "asset_vehicle", 1024), // custom asset, by prefix
            right(1, "somethingelse", 1024), // unknown: must not be guessed
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let by = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().clone();

        assert_eq!(names(&by("computer").bits), vec!["READ_OWNED"]);
        assert_eq!(names(&by("asset_vehicle").bits), vec!["READ_OWNED"]);
        assert_eq!(names(&by("ticket").bits), vec!["READALL"]);
        assert_eq!(names(&by("knowbase").bits), vec!["KNOWBASEADMIN"]);

        // planning and ticket put READALL and READGROUP on opposite bits, so
        // one table could never serve both.
        assert_eq!(names(&by("planning").bits), vec!["READGROUP"]);

        // An unrecognised right gets no name for bit 10 at all. Reporting it
        // as unnamed is the honest reading; naming it would be a guess with
        // the authority of a fact.
        assert!(by("somethingelse").bits.is_empty());
        assert_eq!(by("somethingelse").unnamed_bits, 1024);
    }

    /// `rule_asset` is the trap in the `asset_` prefix rule: it contains
    /// "asset", does not start with "asset_", and is a rule right rather than
    /// an assignable one. Its bit 10 is PARENT, not READ_OWNED.
    #[test]
    fn the_rule_rights_are_not_mistaken_for_asset_rights() {
        let profiles = vec![profile(1, "p", "central")];
        let rights = vec![
            right(1, "rule_asset", 1024),
            right(1, "rule_ticket", 1024),
            right(1, "rule_change", 1024),
            right(1, "rule_problem", 1024),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let by = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().clone();

        for name in ["rule_asset", "rule_ticket", "rule_change", "rule_problem"] {
            assert_eq!(names(&by(name).bits), vec!["PARENT"], "{name}");
            assert!(
                !by(name).writes,
                "{name} PARENT stands in for READ, so it is not a write"
            );
            assert_eq!(by(name).unnamed_bits, 0, "{name} has nothing left over");
        }
    }

    /// Bit 10 is a WRITE under several rights, and the old global table hid
    /// that: READ_OWNED is absent from WRITE_BITS, so every one of these read
    /// as read-only on a page whose whole job is saying what an account can
    /// do. Understating is the one direction this must never err in.
    #[test]
    fn writes_hiding_in_bit_ten_are_not_called_read_only() {
        let profiles = vec![profile(1, "p", "central")];
        let rights = vec![
            right(1, "inventory", 1024),        // IMPORTFROMFILE
            right(1, "user", 1024),             // IMPORTEXTAUTHUSERS
            right(1, "knowbase", 1024),         // KNOWBASEADMIN
            right(1, "reservation", 1024),      // RESERVEANITEM
            right(1, "search_config", 3072),    // PERSONAL | GENERAL
            right(1, "projecttask", 1024),      // UPDATEMY
            right(1, "ticketvalidation", 1024), // CREATEREQUEST
            right(1, "externalevent", 1024),    // MANAGE_BG_EVENTS
            right(1, "computer", 1024),         // READ_OWNED - a genuine read
            right(1, "ticket", 1024),           // READALL - a genuine read
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let writes = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().writes;

        for name in [
            "inventory",
            "user",
            "knowbase",
            "reservation",
            "search_config",
            "projecttask",
            "ticketvalidation",
            "externalevent",
        ] {
            assert!(writes(name), "{name} bit 10 mutates and must count as one");
        }

        assert!(!writes("computer"), "READ_OWNED really is a read");
        assert!(!writes("ticket"), "READALL really is a read");
    }

    /// An override replaces the global reading of a bit rather than adding to
    /// it, which bit 7 is the only live case of: RSSFeed.php:59 and
    /// Reminder.php:66 both define PERSONAL = 128, where the global table
    /// says UNLOCK.
    #[test]
    fn an_override_replaces_a_global_bit_rather_than_doubling_it() {
        let profiles = vec![profile(1, "p", "central")];
        let rights = vec![
            right(1, "rssfeed_public", 128),
            right(1, "reminder_public", 128),
            right(1, "computer", 128),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let by = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().clone();

        assert_eq!(names(&by("rssfeed_public").bits), vec!["PERSONAL"]);
        assert_eq!(names(&by("reminder_public").bits), vec!["PERSONAL"]);
        assert_eq!(by("rssfeed_public").unnamed_bits, 0, "no bit left over");

        // Everything else still reads bit 7 as UNLOCK.
        assert_eq!(names(&by("computer").bits), vec!["UNLOCK"]);
    }

    #[test]
    fn write_bits_are_recognised() {
        let profiles = vec![profile(1, "a", "central")];
        let rights = vec![
            right(1, "a", 31),   // ALLSTANDARDRIGHT
            right(1, "b", 2),    // UPDATE alone
            right(1, "c", 128),  // UNLOCK alone
            right(1, "d", 1057), // READ | READNOTE | READ_OWNED - reads only
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        let p = &r.profiles[0];
        let writes = |n: &str| p.entries.iter().find(|e| e.name == n).unwrap().writes;
        assert!(writes("a"), "ALLSTANDARDRIGHT contains UPDATE");
        assert!(writes("b"));
        assert!(writes("c"), "UNLOCK mutates and must count as a write");
        assert!(!writes("d"), "read bits alone must not be called a write");

        // ALLSTANDARDRIGHT is an alias for five bits, not a thirteenth bit, so
        // it must decode to those five and leave nothing unnamed.
        let all = p.entries.iter().find(|e| e.name == "a").unwrap();
        assert_eq!(
            names(&all.bits),
            vec!["READ", "UPDATE", "CREATE", "DELETE", "PURGE"]
        );
        assert_eq!(all.unnamed_bits, 0);
    }

    /// A quoted number must not score as ungranted - that would read as
    /// "permission missing" on screen.
    #[test]
    fn a_quoted_value_is_parsed_rather_than_treated_as_zero() {
        let profiles = vec![profile(1, "a", "central")];
        let rights = vec![serde_json::json!({
            "profiles_id": 1, "name": "agent", "rights": "1"
        })];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        assert_eq!(r.profiles[0].entries[0].value, 1);
        assert_eq!(names(&r.profiles[0].entries[0].bits), vec!["READ"]);
    }

    /// Malformed rows are skipped, not fatal, and an empty upstream yields an
    /// empty report rather than a panic.
    #[test]
    fn malformed_input_degrades_rather_than_panicking() {
        let r = RightsReport::from_legacy(&[], &[], None, LIMIT).unwrap();
        assert!(r.profiles.is_empty());

        // A profile with no id cannot be shown; a right with no name cannot be
        // either.
        let profiles = vec![
            serde_json::json!({ "name": "no id" }),
            profile(1, "a", "central"),
        ];
        let rights = vec![
            serde_json::json!({ "profiles_id": 1, "rights": 1 }),
            right(1, "agent", 1),
        ];
        let r = RightsReport::from_legacy(&profiles, &rights, None, LIMIT).unwrap();
        assert_eq!(r.profiles.len(), 1);
        assert_eq!(r.profiles[0].entries.len(), 1);
        assert_eq!(r.profiles[0].entries[0].name, "agent");
    }
}
