//! Every right of every profile in GLPI, decoded.
//!
//! The Status page answers "can this dashboard work at all" with the two
//! rights it needs. This one answers the questions an administrator asks when
//! setting that account up or auditing it afterwards: what does each profile
//! actually grant, which profile is this app running under, and does that
//! profile grant more than a read-only dashboard has any business holding?
//!
//! None of it is visible from GLPI's own screens without opening a profile
//! form and reading several tabs of checkboxes, one profile at a time.
//!
//! The data comes from v1's ProfileRight collection, not v2.3 - see
//! backend/src/glpi/rights.rs for why v2.3 cannot answer this, and why the
//! obvious v2.3 workaround silently answers it wrongly.
use crate::api::{self, CallStatus, ProfileRights, RightEntry, RightsReport};
use crate::components::{InfoButton, InfoLink, InfoSection, Panel, StatTile};
use dioxus::prelude::*;

#[component]
pub fn Rights() -> Element {
    let rights = use_resource(|| async move { api::fetch_rights().await });

    rsx! {
        match &*rights.read_unchecked() {
            // An unreachable backend must never render as "no rights" - on a
            // permissions page that reads as a catastrophic misconfiguration
            // rather than as a failure to look.
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not read the profiles' rights." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },

            Some(Ok(r)) => rsx! { Report { report: r.clone() } },

            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Loading…"
                }
            },
        }
    }
}

#[component]
fn Report(report: RightsReport) -> Element {
    // Open on the profile this app is actually using. That is the one a reader
    // arriving from a 403 needs, and it makes the page useful without a click.
    let initial = report
        .active_profile_id
        .or_else(|| report.profiles.first().map(|p| p.id));
    let mut selected = use_signal(move || initial);

    // Two halves of one question, separated because they are consulted at
    // different moments: Profiles answers "what may this account do", Api
    // answers "did the asking itself work". Profiles opens first - a reader
    // reaches this page because of a permission, not because of a request.
    let mut tab = use_signal(|| 0usize);

    let profiles = report.profiles.clone();

    // Resolved without indexing, so an empty list cannot panic here. The
    // empty case is handled inside the Profiles tab rather than by returning
    // early: returning would also remove the Api tab, which is precisely the
    // one worth reading when GLPI returned no profiles.
    let current = profiles
        .iter()
        .find(|p| Some(p.id) == selected())
        .cloned()
        .or_else(|| profiles.first().cloned());

    rsx! {
        div { role: "tablist", class: "tabs tabs-border mb-5",
            button {
                role: "tab",
                class: if tab() == 0 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(0),
                "Profiles"
            }
            button {
                role: "tab",
                class: if tab() == 1 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(1),
                "Api"
            }
            button {
                role: "tab",
                class: if tab() == 2 { "tab text-base tab-active" } else { "tab text-base" },
                onclick: move |_| tab.set(2),
                "Db"
            }
        }

        if tab() == 2 {
            // Fetches on its own, inside the tab, so opening Profiles or Api
            // costs no database round trip.
            DbRightsPanel {}
        } else if tab() == 1 {
            CallPanel { calls: report.calls.clone() }
        } else if profiles.is_empty() {
            div { class: "alert alert-warning",
                div {
                    div { class: "font-semibold", "GLPI returned no profiles." }
                    div { class: "text-sm",
                        "Nothing to show. The account this app signs in with needs read access "
                        "to profiles for this page to work; every other page is unaffected. "
                        "The Api tab shows which call was refused."
                    }
                }
            }
        } else {
            // The profile picker doubles as the overview: each button carries its
            // own granted count, so the comparison an auditor wants - which
            // profiles are permissive - is readable without selecting each one.
            Panel {
                title: "Profiles".to_string(),
                subtitle: format!("{} in this GLPI", profiles.len()),
                info: "Every profile GLPI holds, not only the ones this app's account may use. \
                       Select one to see its rights in full."
                    .to_string(),
                info_sections: vec![
                    InfoSection::new(
                        "Why every profile is listed",
                        "A permissions question is rarely about the profile you are already in. It \
                         is usually either comparing one against another - this profile works and \
                         that one does not - or checking that a profile nobody is watching has not \
                         been given more than it needs.",
                    ),
                    InfoSection::new(
                        "The active one",
                        "Marked below. GLPI activates exactly one profile per session, so that is \
                         the only one in force for this app; an account authorised for others gains \
                         nothing from them until one becomes its default. The Status page shows the \
                         authorisations side of this.",
                    ),
                    InfoSection::new(
                        "Where this comes from",
                        "GLPI's older API. Its current one cannot report the rights of any profile \
                         except the one the request is already running as, and the header that \
                         looks like it should change that is ignored for any profile the account is \
                         not authorised for - returning the account's own rights, successfully, \
                         under the other profile's name. Reading the rights directly avoids that \
                         trap entirely.",
                    ),
                ],
                info_links: vec![InfoLink::new("Profiles", "front/profile.php")],

                div { class: "flex flex-wrap gap-2",
                    for p in profiles.iter() {
                        button {
                            key: "{p.id}",
                            class: if Some(p.id) == selected() {
                                "btn btn-sm btn-primary"
                            } else {
                                "btn btn-sm btn-ghost border border-base-300"
                            },
                            onclick: {
                                let id = p.id;
                                move |_| selected.set(Some(id))
                            },
                            span { "{p.label()}" }
                            span { class: "badge badge-sm", "{p.granted()}" }
                            if p.writes() > 0 {
                                span { class: "badge badge-warning badge-sm", "w" }
                            }
                            if p.is_active {
                                span { class: "badge badge-success badge-sm", "active" }
                            }
                        }
                    }
                }
            }

            // Keyed on the profile id, so selecting another profile rebuilds this
            // subtree rather than reusing it. That resets the filter signals in
            // the table below as a consequence of identity: a filter left over
            // from the previous profile would otherwise read as that profile
            // having fewer rights than it does.
            div { class: "mt-5",
                if let Some(cur) = current.clone() {
                    ProfilePanel {
                        key: "{cur.id}",
                        profile: cur,
                        grouped: report.taxonomy_rights > 0,
                        form_only: report.form_only_rights.clone(),
                    }
                }
            }
        }
    }
}

#[component]
fn ProfilePanel(profile: ProfileRights, grouped: bool, form_only: Vec<String>) -> Element {
    let entries = profile.entries.clone();
    let total = entries.len();
    let granted: Vec<&RightEntry> = entries.iter().filter(|e| e.granted()).collect();
    let writes = granted.iter().filter(|e| e.writes).count();
    let read_only = granted.len() - writes;
    let module_defined = granted.iter().filter(|e| e.unnamed_bits != 0).count();

    // Only worth alarming about for the profile this app actually runs as. A
    // Super-Admin profile that can write is not a finding, it is the point of
    // it; the service account being able to is.
    let alarm_writes = profile.is_active && writes > 0;

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",

            StatTile {
                title: "Granted".to_string(),
                value: format!("{} of {}", granted.len(), total),
                desc: "rights this profile holds".to_string(),
                body: "GLPI divides permission into one right per area of the application - \
                       computers, tickets, agents, and so on. This counts how many of them \
                       this profile has been given at least some access to."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why the total matters too",
                        "The denominator is every right that exists anywhere in this GLPI, so \
                         the ones this profile does NOT have are listed below as well. That is \
                         deliberate: when GLPI refuses a request it never names the missing \
                         permission, so seeing that a right exists and is not granted is \
                         usually the answer.",
                    ),
                    InfoSection::new(
                        "The same total for every profile",
                        "GLPI does not record every right against every profile - it keeps only \
                         the entries that exist, and a few rights are recorded on some profiles \
                         and not others. Counting each profile against its own entries would \
                         give them different denominators and make two profiles look comparable \
                         when they are not, so every profile here is counted against the same \
                         full set.",
                    ),
                    InfoSection::new(
                        "“No entry” in the table",
                        "Means GLPI holds no setting for that right on this profile, as opposed \
                         to holding one that is switched off. The effect is identical - GLPI \
                         denies both - but only one of them is a decision somebody made, so the \
                         table does not call the other one “not granted”.",
                    ),
                ],
                links: vec![InfoLink::new("Profiles", "front/profile.php")],
            }

            StatTile {
                title: "Read-only".to_string(),
                value: read_only.to_string(),
                desc: "look, but cannot change".to_string(),
                body: "Rights that grant only reading. This is all a dashboard needs: every \
                       page in this app is a view of something GLPI already holds, and it \
                       never creates, changes or deletes anything."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Reading notes counts as reading",
                        "Several rights here are a combination like READ plus READNOTE - \
                         permission to see an item and to see the free-text notes attached to \
                         it. Neither changes anything, so both count as read-only.",
                    ),
                ],
            }

            StatTile {
                title: "Write-capable".to_string(),
                value: writes.to_string(),
                alarm: alarm_writes,
                desc: if profile.is_active { "more than this app needs".to_string() } else { "can change things".to_string() },
                body: "Rights that include permission to change something - create, update, \
                       delete, purge, or unlock a field."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "When this is worth acting on",
                        "For most profiles it is simply what they are for - an administrator \
                         profile that cannot write would be useless. It matters for the profile \
                         marked active, because that is the one this dashboard signs in with, \
                         and this app never writes to GLPI. Any write access there is privilege \
                         held by credentials sitting in a file on the machine running it.",
                    ),
                    InfoSection::new(
                        "What counts as a write",
                        "UPDATE, CREATE, DELETE and PURGE, plus UPDATENOTE for the notes on an \
                         item, UPDATE_ASSIGNED and UPDATE_OWNED for items assigned to or owned \
                         by the user, and UNLOCK - which clears the lock on a field imported by \
                         inventory so it can be edited by hand. That last is not an obvious \
                         write, but it does change what GLPI stores.",
                    ),
                ],
                links: vec![InfoLink::new("Profiles", "front/profile.php")],
            }

            StatTile {
                title: "Module-defined".to_string(),
                value: module_defined.to_string(),
                desc: "bits this app cannot name".to_string(),
                body: "Rights whose value contains permission bits beyond GLPI's twelve \
                       standard ones. They are real access; only their name is unavailable."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Where the extra bits come from",
                        "A right is a number whose individual bits each mean one permission. \
                         Twelve of those bits are defined globally and mean the same thing \
                         everywhere. Parts of GLPI - tickets, changes, problems, tasks - define \
                         further bits of their own for things that only exist there, like being \
                         allowed to assign a ticket.",
                    ),
                    InfoSection::new(
                        "Why they are shown as a number",
                        "GLPI resolves those names from the profile form's own definition, which \
                         is not available through either API. The alternative to showing the \
                         leftover as a number would be to drop it silently, which would \
                         understate what the profile can do - the one direction a permissions \
                         screen must not be wrong in.",
                    ),
                ],
            }
        }

        if alarm_writes {
            div { class: "alert alert-warning mb-5",
                div {
                    div { class: "font-semibold",
                        "The profile this app runs as can change things in GLPI."
                    }
                    div { class: "text-sm",
                        "{writes} of the rights below include write access, and this dashboard "
                        "only ever reads. The credentials it uses are stored on the machine "
                        "running it, so they are worth no more privilege than the app needs. "
                        "A profile made solely for this app would be read-only throughout."
                    }
                }
            }
        }

        // Keyed on the profile, so selecting another one rebuilds this subtree
        // rather than reusing it. That resets the filter signals inside as a
        // consequence of identity: a filter left over from the previous
        // profile would otherwise read as that profile having fewer rights.
        RightsTable { entries, profile, grouped, form_only }
    }
}

#[component]
fn RightsTable(
    entries: Vec<RightEntry>,
    profile: ProfileRights,
    grouped: bool,
    form_only: Vec<String>,
) -> Element {
    // A plain text filter and one toggle, because a hundred rows sorted by
    // name is not something anyone reads top to bottom - they arrive knowing
    // which right they are looking for, usually from a 403 that would not say.
    //
    // These start empty on every profile: ProfilePanel is keyed by profile id,
    // so switching profiles rebuilds this component rather than reusing it.
    let mut query = use_signal(String::new);
    let mut granted_only = use_signal(|| false);
    let mut sort_key = use_signal(|| SortKey::Name);
    let mut sort_desc = use_signal(|| false);

    // Which permission bits the Permissions sort ranks by. Empty means "rank
    // by how many bits a right grants at all", which is the useful default:
    // it brings the most permissive rights to the top.
    let mut picked = use_signal(std::collections::BTreeSet::<String>::new);

    // The bits that actually occur on THIS profile, read bits before write
    // bits. Taken from the data rather than from a list held here: the
    // backend already classifies each bit, and a second list in the frontend
    // is a second thing to get wrong when GLPI adds one.
    let bit_universe: Vec<(String, bool)> = {
        let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        for e in &entries {
            for b in &e.bits {
                seen.insert(b.name.clone(), b.writes);
            }
        }
        let mut v: Vec<(String, bool)> = seen.into_iter().collect();
        v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        v
    };

    let q = query().to_lowercase();
    let mut visible: Vec<RightEntry> = entries
        .iter()
        .filter(|e| !granted_only() || e.granted())
        .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
        .cloned()
        .collect();

    // Sorted before grouping, because group_rows preserves the order it is
    // given within each heading - so the sort applies inside each tab rather
    // than flattening the grouping the page just established.
    {
        let chosen = picked();
        let desc = sort_desc();
        visible.sort_by(|a, b| {
            let ord = match sort_key() {
                SortKey::Name => std::cmp::Ordering::Equal,
                SortKey::Value => a.value.cmp(&b.value),
                SortKey::Permissions => {
                    // With bits chosen, rank by how many of THOSE a right
                    // has; without, by how many it has in total. Ties fall
                    // through to the raw value, so two rights that match
                    // equally still order predictably.
                    let score = |e: &RightEntry| {
                        if chosen.is_empty() {
                            e.bits.len()
                        } else {
                            e.bits.iter().filter(|b| chosen.contains(&b.name)).count()
                        }
                    };
                    score(a).cmp(&score(b)).then(a.value.cmp(&b.value))
                }
            };
            // Name is the final tie-break, so the order is total and the
            // table never reshuffles between renders.
            //
            // The reverse is applied AFTER that tie-break, not before it.
            // Reversing first was a bug: the Name sort's primary comparison is
            // Equal, and Equal.reverse() is still Equal, so the name tie-break
            // ran ascending whatever the direction and clicking the Right
            // heading a second time did nothing at all.
            let ord = ord.then(a.name.cmp(&b.name));
            if desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }

    let interface = profile
        .interface
        .clone()
        .unwrap_or_else(|| "unknown".into());

    rsx! {
        Panel {
            title: format!("Rights of “{}”", profile.label()),
            subtitle: format!(
                "{} of {} shown · {} interface{}",
                visible.len(),
                entries.len(),
                interface,
                if profile.is_default { " · GLPI's default profile" } else { "" },
            ),
            info: "Every right GLPI records for this profile, granted or not.".to_string(),
            info_sections: vec![
                InfoSection::new(
                    "The columns",
                    "Right is GLPI's own internal name for it, left exactly as GLPI spells it \
                     so it can be searched for in GLPI's source or matched against a profile \
                     form. Value is the raw number GLPI stores. Permissions is that number \
                     decoded into the individual permissions it contains.",
                ),
                InfoSection::new(
                    "Why the raw value is shown",
                    "Because it is the authoritative figure and the decoding is an \
                     interpretation of it. The twelve standard bits mean the same thing \
                     throughout GLPI, but a module may reuse a higher bit for something of its \
                     own, so a name shown here is the standard meaning rather than a promise \
                     about that particular right. When the two could differ, the number is the \
                     one to trust.",
                ),
                InfoSection::new(
                    "The interface matters",
                    "A helpdesk profile cannot hold the rights this app needs at all - the \
                     section is absent from GLPI's form rather than merely unticked. If a \
                     profile looks inexplicably empty of the permissions you expect, check \
                     this line before hunting for the checkbox.",
                ),
                InfoSection::new(
                    "Where to change any of this",
                    "In GLPI under Administration, then Profiles, on the profile named above. \
                     Changing a right there affects every account that uses the profile, so a \
                     profile dedicated to this app is safer to edit than a shared one.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Profiles", "front/profile.php"),
                InfoLink::new("Users", "front/user.php"),
            ],

            div { class: "mb-3 flex flex-wrap items-center gap-3",
                input {
                    class: "input input-sm w-56",
                    r#type: "search",
                    placeholder: "Filter by name…",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                label { class: "flex cursor-pointer items-center gap-2 text-xs text-base-content/70",
                    input {
                        class: "checkbox checkbox-sm",
                        r#type: "checkbox",
                        checked: granted_only(),
                        onchange: move |e| granted_only.set(e.checked()),
                    }
                    "Granted only"
                }
            }

            // Only while sorting by Permissions. A row of twelve chips is a
            // lot of furniture to carry on a column nobody is ordering by,
            // and hiding it keeps the default table as plain as it was.
            if sort_key() == SortKey::Permissions && !bit_universe.is_empty() {
                div { class: "mb-3 flex flex-wrap items-center gap-2",
                    span { class: "text-xs text-base-content/60", "Rank by:" }
                    for (name, writes) in bit_universe.iter().cloned() {
                        {
                            let on = picked().contains(&name);
                            let label = name.clone();
                            rsx! {
                                button {
                                    key: "{label}",
                                    class: if on {
                                        if writes { "badge badge-warning badge-sm font-mono cursor-pointer" }
                                        else { "badge badge-neutral badge-sm font-mono cursor-pointer" }
                                    } else {
                                        "badge badge-outline badge-sm font-mono cursor-pointer"
                                    },
                                    onclick: move |_| {
                                        let mut set = picked();
                                        if !set.remove(&label) {
                                            set.insert(label.clone());
                                        }
                                        picked.set(set);
                                    },
                                    "{name}"
                                }
                            }
                        }
                    }
                    if !picked().is_empty() {
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| picked.set(std::collections::BTreeSet::new()),
                            "clear"
                        }
                    } else {
                        // Says what the empty state means, rather than
                        // leaving the reader to infer it from an unchanged
                        // table.
                        span { class: "text-xs text-base-content/50",
                            "nothing picked - ranking by how many permissions each right grants"
                        }
                    }
                }
            }

            if visible.is_empty() {
                // Distinct from the error state on purpose: this is an empty
                // filter result or a genuinely empty profile, not a failure to
                // read anything.
                div { class: "py-6 text-center text-sm text-base-content/60",
                    if entries.is_empty() {
                        "GLPI records no rights for this profile."
                    } else {
                        "No right matches that filter."
                    }
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                SortHeader {
                                    label: "Right".to_string(),
                                    column: SortKey::Name,
                                    active: sort_key(),
                                    desc: sort_desc(),
                                    on_pick: move |k| {
                                        if sort_key() == k { sort_desc.toggle() } else { sort_key.set(k); sort_desc.set(false) }
                                    },
                                }
                                SortHeader {
                                    label: "Value".to_string(),
                                    column: SortKey::Value,
                                    active: sort_key(),
                                    desc: sort_desc(),
                                    on_pick: move |k| {
                                        if sort_key() == k { sort_desc.toggle() } else { sort_key.set(k); sort_desc.set(false) }
                                    },
                                    info: rsx! {
                                        InfoButton {
                                            title: "Value".to_string(),
                                            body: "The raw number GLPI stores for this right on this \
                                                   profile. Every other column in the row is a reading \
                                                   of it, so this is the one figure here that cannot be \
                                                   wrong."
                                                .to_string(),
                                    sections: vec![
                                InfoSection::new(
                                    "Row of switches",
                                    "In binary, every 1 is one permission held. Positions are worth 1, \
                                     2, 4, 8, 16 and on, each double the last, so any set of switches \
                                     sums to a total only that set could make: 3072 is 1024 plus 2048, \
                                     and nothing else gives 3072. The shared names begin READ 1, UPDATE \
                                     2, CREATE 4, DELETE 8, PURGE 16 - and ALLSTANDARDRIGHT 31 is those \
                                     five together, not a sixth switch."
                                ),
                                InfoSection::new(
                                    "Per right",
                                    "The number says WHICH switch, never what it does - GLPI decides \
                                     that per right. Bit 10 carries fifteen distinct meanings and \
                                     bit 11 ten: \
                                     1024 is READALL under ticket, READGROUP under planning, \
                                     KNOWBASEADMIN under knowbase, READ_OWNED on an asset. ticket and \
                                     planning even swap the two names between the two bits, so a value \
                                     cannot be read without the name beside it."
                                ),
                                InfoSection::new(
                                    "Writes hide here",
                                    "Several bit-10 meanings mutate: IMPORTFROMFILE under inventory, \
                                     IMPORTEXTAUTHUSERS under user, KNOWBASEADMIN under knowbase, \
                                     PERSONAL and GENERAL under search_config. Through one global table \
                                     they all look like READ_OWNED, which is not a write, and the Read- \
                                     only count above absorbs them. Permissions is decoded per right \
                                     name for that reason; an unverified bit shows as +N rather than a \
                                     wrong label."
                                ),
                                InfoSection::new(
                                    "Where it lives",
                                    "One row per profile per right in glpi_profilerights, unique on \
                                     (profiles_id, name), the number in a plain signed int - so bits 0 \
                                     to 30 are usable and the highest GLPI ships is 65536. The meanings \
                                     are not in the database but in PHP, so SQL can read these values \
                                     and cannot interpret them. Read here over the v1 API: v2.3 carries \
                                     no profile rights at all."
                                ),
                                InfoSection::new(
                                    "Zero vs blank",
                                    "A stored 0 is a decision someone made in the profile form. No row \
                                     at all means the form never offered the right for this profile. \
                                     Both deny identically, so the two are tracked apart here rather \
                                     than both shown as 0. Custom assets make it visible: deactivating \
                                     a definition deletes its rights rows outright instead of zeroing \
                                     them."
                                ),
                            ],


                                            links: vec![
                                                InfoLink::new("Profiles", "front/profile.php"),
                                                InfoLink::new(
                                                    "Asset definitions",
                                                    "front/asset/assetdefinition.php",
                                                ),
                                            ],
                                        }
                                    },
                                }
                                SortHeader {
                                    label: "Permissions".to_string(),
                                    column: SortKey::Permissions,
                                    active: sort_key(),
                                    desc: sort_desc(),
                                    on_pick: move |k| {
                                        if sort_key() == k { sort_desc.toggle() } else { sort_key.set(k); sort_desc.set(false) }
                                    },
                                }
                            }
                        }
                        // One tbody per heading. With no taxonomy committed
                        // there is a single unheaded body, which is the flat
                        // list this page had before grouping existed.
                        // Grouping and sorting are competing orders, so an
                        // explicit sort flattens the headings. Sorting by
                        // Value inside 18 tab blocks leaves the first rows
                        // unchanged and reads as a sort that did nothing -
                        // which is exactly how it was reported. Name is the
                        // default and keeps the grouped view.
                        for (heading, rows) in group_rows(&visible, grouped && sort_key() == SortKey::Name) {
                            tbody { key: "{heading.clone().unwrap_or_default()}",
                                if let Some(h) = heading {
                                    tr {
                                        th {
                                            class: "bg-base-200 text-xs font-semibold",
                                            colspan: "3",
                                            "{h}"
                                        }
                                    }
                                }
                                for e in rows {
                                    RightRow { key: "{e.name}", entry: e }
                                }
                            }
                        }
                    }
                }

                if grouped && !form_only.is_empty() {
                    // Deliberately a quiet note and not a warning. GLPI ships
                    // rights that appear on the profile form with no stored
                    // value anywhere - defaultfilter is one, seeded nowhere -
                    // so this list is normally non-empty and alarming about it
                    // would train the reader to ignore the alarm.
                    div { class: "mt-4 text-xs text-base-content/60",
                        "On GLPI's profile form but not recorded against any profile: "
                        span { class: "font-mono", "{form_only.join(\", \")}" }
                        ". An unrecorded right is denied, so nobody holds these."
                    }
                }
            }
        }
    }
}

/// What this app asks GLPI for, and whether the active profile allows it.
///
/// First on the page because it is the actionable half. The profile tables
/// below answer "what does this profile grant"; this answers "which of this
/// dashboard's own requests will be refused, and because of which right" -
/// the question GLPI's 403 refuses to answer.
#[component]
fn CallPanel(calls: Vec<CallStatus>) -> Element {
    if calls.is_empty() {
        // Not an error. The backend declines to judge the calls when it cannot
        // tell which profile is active, rather than judging them against an
        // arbitrary one.
        return rsx! {
            Panel {
                title: "What this app asks GLPI for".to_string(),
                info: "Cannot be shown without knowing which profile is active.".to_string(),
                div { class: "py-4 text-sm text-base-content/60",
                    "GLPI did not report an active profile for this session, so there is "
                    "nothing to judge these requests against. The Status page shows the "
                    "session this app is using."
                }
            }
        };
    }

    let blocked = calls.iter().filter(|c| !c.satisfied).count();

    // Counted over features rather than calls, because a feature is what
    // breaks: one blocked request can take out two pages, and "1 blocked"
    // would understate that.
    let grouped = by_feature(&calls);
    let broken_features = grouped
        .iter()
        .filter(|(_, rows)| rows.iter().any(|c| !c.satisfied))
        .count();

    rsx! {
        Panel {
            title: "What this app asks GLPI for".to_string(),
            subtitle: if blocked == 0 {
                format!("{} features, {} requests, all permitted", grouped.len(), calls.len())
            } else {
                format!(
                    "{broken_features} of {} features affected · {blocked} of {} requests blocked",
                    grouped.len(),
                    calls.len(),
                )
            },
            info: "Every request this dashboard makes to GLPI, and whether the profile it \
                   signs in with is allowed to make it."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is the first thing here",
                    "When GLPI refuses a request it answers with one sentence - you do not \
                     have permission to perform this action - and never says which permission \
                     was missing. This table is that missing sentence: it names the request, \
                     the page that breaks without it, and the exact right to grant.",
                ),
                InfoSection::new(
                    "Permitted is not the same as working",
                    "A right is necessary, never sufficient. GLPI filters results by entity \
                     after it has checked permission, so a request can be fully permitted and \
                     still come back empty because the account cannot see that part of the \
                     estate. A green row here rules out permissions as the cause; it does not \
                     promise data.",
                ),
            ],
            info_links: vec![InfoLink::new("Profiles", "front/profile.php")],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "Request" }
                            // The explanation sits on the column whose badges
                            // it explains, not in the panel's own info, which
                            // is about the table as a whole. Same pairing as
                            // the labels on the Status page.
                            th {
                                div { class: "flex items-center gap-2",
                                    span { "API" }
                                    InfoButton {
                                        title: "API version".to_string(),
                                        body: "Which of GLPI's two interfaces this request uses. \
                                               The badge on each row below says v2.3 or v1."
                                            .to_string(),
                                        sections: vec![
                                            InfoSection::new(
                                                "v2.3 and v1",
                                                "GLPI serves two generations of interface side \
                                                 by side, and every request in this table is \
                                                 tagged with the one it uses. v2.3 is the \
                                                 current one; v1 is the older, lower-level one. \
                                                 GLPI lists five in all - v1, then v2.0, v2.1 \
                                                 and v2.2 all marked deprecated, then v2.3 - so \
                                                 these two are the only ones worth using: the \
                                                 newest, and the one that predates the series.",
                                            ),
                                            InfoSection::new(
                                                "Why both",
                                                "Not preference. The current interface cannot \
                                                 answer some questions at all: it has no way to \
                                                 report the rights of any profile except the one \
                                                 it is signed in as, and no way to report who \
                                                 may use which profile. GLPI says as much \
                                                 itself, describing the older interface as more \
                                                 powerful and able to do things the newer one \
                                                 cannot. So everything this app can get from \
                                                 v2.3 it does, and v1 covers only the requests \
                                                 with no newer equivalent. The cost is that GLPI \
                                                 attaches no stability promise to v1 and says it \
                                                 may change without warning - which is why it is \
                                                 confined to those few, and why the v1 rows are \
                                                 the first place to look if this app breaks \
                                                 after a GLPI upgrade.",
                                            ),
                                            InfoSection::new(
                                                "Same permissions",
                                                "Worth being explicit, because reaching for an \
                                                 older interface often is a way around \
                                                 something. This is not. Both sign in as the \
                                                 same account and are filtered by the same \
                                                 profile rights and the same entity scope - the \
                                                 rights listed against a v1 request are real and \
                                                 enforced, and it reads nothing the account \
                                                 could not see in GLPI's own screens. Reading \
                                                 GLPI's database directly would bypass all of \
                                                 that; this does not, which is why it was the \
                                                 acceptable option. The two do accept different \
                                                 credentials, so a sign-in failure usually hits \
                                                 the v2.3 rows or the v1 rows rather than both - \
                                                 which group fails tells you where to look.",
                                            ),
                                        ],
                                        links: vec![],
                                    }
                                }
                            }
                            th { "Needs" }
                        }
                    }
                    // One tbody per feature, so the top level is what a reader
                    // would notice breaking - "the Computers page is empty" -
                    // rather than an endpoint they have never heard of. A call
                    // serving two features appears under both.
                    for (feature, rows) in grouped.clone() {
                        tbody { key: "{feature}",
                            tr {
                                th {
                                    class: "bg-base-200 text-xs font-semibold",
                                    colspan: "3",
                                    "{feature}"
                                    // The feature's own verdict: it is broken
                                    // if ANY request it depends on is.
                                    if rows.iter().any(|c| !c.satisfied) {
                                        span { class: "badge badge-error badge-sm ml-2", "blocked" }
                                    } else {
                                        span { class: "badge badge-success badge-sm ml-2", "ok" }
                                    }
                                }
                            }
                            for c in rows {
                                tr { key: "{c.api}-{c.path}",
                                    td { class: "font-mono text-xs whitespace-nowrap",
                                        if !c.satisfied {
                                            span { class: "badge badge-error badge-sm mr-2", "blocked" }
                                        }
                                        "{c.path}"
                                        // Why this call is here, when it is not
                                        // the whole feature.
                                        if let Some(role) = role_in(&c, &feature) {
                                            span { class: "ml-2 text-xs text-base-content/60", "({role})" }
                                        }
                                    }
                                    td {
                                        span { class: "badge badge-ghost badge-sm font-mono", "{c.api}" }
                                    }
                                    td {
                                        if c.rights.is_empty() {
                                            // Needs nothing, which is a real
                                            // answer and not a gap.
                                            span { class: "text-xs text-base-content/50", "no right" }
                                        } else {
                                            div { class: "flex flex-wrap items-center gap-1",
                                                for r in c.rights.iter() {
                                                    span {
                                                        key: "{r.name}",
                                                        class: if r.satisfied {
                                                            "badge badge-ghost badge-sm font-mono"
                                                        } else {
                                                            "badge badge-error badge-sm font-mono"
                                                        },
                                                        title: match r.value {
                                                            Some(v) => format!("{} is {v} on this profile", r.name),
                                                            None => format!("GLPI records no entry for {} on this profile", r.name),
                                                        },
                                                        "{r.name}"
                                                    }
                                                }
                                                // Spelled out, because two
                                                // badges side by side read as
                                                // a choice.
                                                if c.rights.len() > 1 {
                                                    span { class: "text-xs text-base-content/50", "(both required)" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if blocked > 0 {
                div { class: "alert alert-error mt-4",
                    div {
                        div { class: "font-semibold",
                            "{blocked} of this app's requests will be refused, affecting "
                            "{broken_features} of its features."
                        }
                        div { class: "text-sm",
                            "Grant the rights marked in red to the active profile, in GLPI under "
                            "Administration → Profiles. Read is enough for every one of them - "
                            "this app never writes to GLPI."
                        }
                    }
                }
            }
        }
    }
}

/// Split rights into the headed blocks the table renders.
///
/// Returns `(heading, rows)` pairs: tab then group from GLPI's own profile
/// form, ordered by tab so two profiles read the same way down the page, with
/// the unplaceable rights last under their own heading.
///
/// `grouped` false yields exactly one unheaded block - the flat alphabetical
/// list. That is the state before `just right-taxonomy` has ever run, and it
/// is a deliberate fallback rather than an error: one enormous "ungrouped"
/// heading would be strictly worse than no headings at all.
fn group_rows(entries: &[RightEntry], grouped: bool) -> Vec<(Option<String>, Vec<RightEntry>)> {
    if !grouped {
        return vec![(None, entries.to_vec())];
    }

    // BTreeMap so the order is the tab/group name order and therefore stable
    // across profiles and across reloads.
    let mut placed: std::collections::BTreeMap<(String, String), Vec<RightEntry>> =
        std::collections::BTreeMap::new();
    let mut unplaced: Vec<RightEntry> = Vec::new();

    for e in entries {
        match (e.tab.clone(), e.group.clone()) {
            (Some(tab), Some(group)) => placed.entry((tab, group)).or_default().push(e.clone()),
            // Half a heading cannot be rendered as one. The backend asserts
            // this never happens; treating it as unplaced is the safe read.
            _ => unplaced.push(e.clone()),
        }
    }

    let mut out: Vec<(Option<String>, Vec<RightEntry>)> = placed
        .into_iter()
        .map(|((tab, group), rows)| {
            // "general" is GLPI's filler name for a tab's only block, so it
            // adds nothing to a heading that already names the tab.
            let heading = if group == "general" {
                tab
            } else {
                format!("{tab} · {group}")
            };
            (Some(heading), rows)
        })
        .collect();

    if !unplaced.is_empty() {
        // Last, and named for what it means rather than for the gap in our
        // map: most of these are rights GLPI's own form never offers.
        out.push((Some("Not on the profile form".to_string()), unplaced));
    }
    out
}

/// One right. Extracted so the grouped and flat paths render identical rows
/// rather than two copies of this markup drifting apart.
#[component]
fn RightRow(entry: RightEntry) -> Element {
    let e = entry;
    rsx! {
        tr { class: if e.granted() { "" } else { "opacity-50" },
            td { class: "font-mono text-xs whitespace-nowrap", "{e.name}" }
            td { class: "font-mono text-xs tabular-nums", "{e.value}" }
            td {
                if e.granted() {
                    div { class: "flex flex-wrap items-center gap-1",
                        for bit in e.bits.iter() {
                            span {
                                key: "{bit.name}",
                                // Amber marks the ability to CHANGE something,
                                // not the presence of a permission: reading is
                                // the baseline this app itself runs on, and
                                // colouring that too would leave every badge
                                // on a hundred-row table coloured and the
                                // colour carrying nothing.
                                //
                                // The classification is the backend's, from the
                                // same mask that decides the write-capable
                                // count, so a badge cannot contradict the tile
                                // above it.
                                class: if bit.writes {
                                    "badge badge-warning badge-sm font-mono"
                                } else {
                                    "badge badge-ghost badge-sm font-mono"
                                },
                                "{bit.name}"
                            }
                        }
                        if e.unnamed_bits != 0 {
                            // Shown as the leftover value, not as a guess at
                            // its name.
                            span {
                                class: "badge badge-outline badge-sm font-mono",
                                title: "Bits beyond GLPI's twelve standard permissions. Real access, defined by one part of GLPI for its own use, and not resolvable to a name through either API.",
                                "+{e.unnamed_bits}"
                            }
                        }
                    }
                } else if e.recorded {
                    span { class: "text-xs text-base-content/50", "not granted" }
                } else {
                    // No row at all, rather than a row holding zero. GLPI
                    // denies both identically, so this is not a worse state -
                    // just a different one, and saying "not granted" would
                    // imply a setting someone chose.
                    span {
                        class: "text-xs text-base-content/40 italic",
                        title: "GLPI stores no row for this right on this profile. It is denied, exactly as a stored zero would be, but it was never set either way here.",
                        "no entry"
                    }
                }
            }
        }
    }
}

/// Group the app's calls by the feature they serve.
///
/// Returns `(feature, calls)` in the order the backend lists them, which is
/// the order `api_calls()` declares - deliberate running order rather than
/// alphabetical, so the pages a reader is most likely to be looking at come
/// first. A call serving several features is returned under each, because the
/// point of this view is that one missing right can break more than one page.
fn by_feature(calls: &[CallStatus]) -> Vec<(String, Vec<CallStatus>)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, Vec<CallStatus>> =
        std::collections::HashMap::new();

    for c in calls {
        for f in &c.features {
            if !map.contains_key(&f.name) {
                order.push(f.name.clone());
            }
            map.entry(f.name.clone()).or_default().push(c.clone());
        }
    }

    order
        .into_iter()
        .filter_map(|name| map.remove(&name).map(|rows| (name, rows)))
        .collect()
}

/// What this call does for that feature, when it is not the whole of it.
fn role_in(call: &CallStatus, feature: &str) -> Option<String> {
    call.features
        .iter()
        .find(|f| f.name == feature)
        .and_then(|f| f.role.clone())
}

/// Which column the rights table is ordered by.
///
/// `Name` is the default and sorts on nothing of its own - every comparison
/// falls through to the name tie-break, which is the order the table had
/// before sorting existed.
#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Name,
    Value,
    Permissions,
}

/// A clickable column heading.
///
/// Carries its own arrow so the active column and direction are visible
/// without a legend. Clicking the active column reverses it; clicking another
/// switches to it ascending, rather than inheriting the previous direction -
/// a reversed sort you did not ask for reads as wrong data.
#[component]
fn SortHeader(
    label: String,
    column: SortKey,
    active: SortKey,
    desc: bool,
    on_pick: EventHandler<SortKey>,
    /// An explanation of what the column holds, shown as the usual info icon
    /// beside the label.
    ///
    /// Rendered as a SIBLING of the sort button rather than inside it, for
    /// two reasons: a button nested in a button is invalid HTML, and
    /// `InfoButton`'s trigger does not stop propagation, so a click on the
    /// icon would also re-sort the table underneath the panel that just
    /// opened.
    #[props(default = None)]
    info: Option<Element>,
) -> Element {
    let is_active = active == column;
    rsx! {
        th {
            div { class: "flex items-center gap-1",
            button {
                class: if is_active {
                    "flex items-center gap-1 font-semibold cursor-pointer"
                } else {
                    "flex items-center gap-1 cursor-pointer text-base-content/70 hover:text-base-content"
                },
                onclick: move |_| on_pick.call(column),
                "{label}"
                // Always a triangle, never the up-down arrow this used to
                // carry: at this size that glyph read as a colon after the
                // label rather than as a control. A faded triangle pointing
                // the way a click would sort says "sortable, not active"
                // without introducing a second shape to learn.
                //
                // text-2xl is twice text-xs, which is what it was.
                span {
                    class: if is_active {
                        "text-2xl leading-none"
                    } else {
                        "text-2xl leading-none text-base-content/30"
                    },
                    if is_active && desc {
                        "\u{25BC}"
                    } else {
                        "\u{25B2}"
                    }
                }
            }
            if let Some(i) = info {
                {i}
            }
            }
        }
    }
}

/// The third layer of permission this app depends on: a database grant.
///
/// Profiles covers what GLPI's own permission system allows, Api covers which
/// request needs which of those rights. Neither reaches this one, because it
/// is not GLPI's to enforce - it is MariaDB's, and it exists only because two
/// things GLPI holds are exposed by neither of its APIs.
#[component]
fn DbRightsPanel() -> Element {
    let report = use_resource(|| async move { api::fetch_refused().await });

    // Live state where there is any, documentation where there is not. The
    // grant below is what the app expects; whether it is actually in place is
    // answered by whether the query works.
    let state = match &*report.read_unchecked() {
        Some(Ok(r)) => Some((r.configured, r.entities.len(), r.rows.len(), None)),
        Some(Err(e)) => Some((true, 0, 0, Some(e.clone()))),
        None => None,
    };

    rsx! {
        Panel {
            title: "Database access".to_string(),
            subtitle: "Two tables, SELECT only".to_string(),
            info: "This app reads GLPI's database directly for two things, and only two. \
                   Everything else on every other page comes through GLPI's API."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why there is a database account at all",
                    "Two things GLPI records are exposed by neither of its APIs: the \
                     submissions it refused, and its own change history. Refused submissions \
                     are the failure this dashboard most wants to show - the agent reports \
                     success and exits clean, GLPI throws the submission away, and no asset \
                     appears. The table is the only place that is written down.",
                ),
                InfoSection::new(
                    "Why it is only two tables",
                    "Because a grant is easier to widen than to narrow, and because every \
                     table added is another piece of GLPI's internal layout this app depends \
                     on. The rule that refused a submission is shown by its id and linked \
                     into GLPI rather than by name, specifically so a third table does not \
                     have to be granted to print one string.",
                ),
                InfoSection::new(
                    "What it costs",
                    "Two things. A renamed column breaks this silently, where the API would \
                     have named the missing field - so every column is treated as optional \
                     and renders blank rather than failing. And SQL has no notion of GLPI's \
                     entities, so the account could read every row regardless of scope: the \
                     rows shown are restricted afterwards, to the entities the API says this \
                     account may see, so that \u{201c}everything\u{201d} means the same here \
                     as on every other page.",
                ),
                InfoSection::new(
                    "No password is stored",
                    "The account is expected to authenticate through the unix socket, which \
                     means there is no secret in this app's configuration to leak and nothing \
                     to rotate. Reading the config file tells an attacker the account name and \
                     nothing they could use from another machine.",
                ),
            ],

            match state {
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Checking…"
                    }
                },
                Some((false, _, _, _)) => rsx! {
                    // Not an error. The app runs perfectly well without this;
                    // only the refused view goes dark.
                    div { class: "alert alert-info",
                        div {
                            div { class: "font-semibold", "No database access is configured." }
                            div { class: "text-sm",
                                "Every other page works without it. The one thing that does not "
                                "is the refused view, which has no other source - so it says "
                                "\u{201c}cannot look\u{201d} rather than showing an empty table, "
                                "because no refusals and no access are opposite answers."
                            }
                        }
                    }
                },
                Some((true, _, _, Some(err))) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Configured, but the query failed." }
                            div { class: "text-sm",
                                "The grant below is the likely gap - a missing SELECT reads as "
                                "a failed query, not as an empty result."
                            }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{err}" }
                        }
                    }
                },
                Some((true, entities, rows, None)) => rsx! {
                    div { class: "mb-4 flex flex-wrap items-center gap-2 text-sm",
                        span { class: "badge badge-success badge-sm", "working" }
                        span { class: "text-base-content/70",
                            "The grant is in place: the refused table answered, "
                            "{rows} row(s) within the {entities} entity scope this account may see."
                        }
                    }
                },
            }

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "Table" }
                            th { "Right" }
                            th { "What it is for" }
                        }
                    }
                    tbody {
                        DbRightRow {
                            table_: "glpi_refusedequipments".to_string(),
                            right: "SELECT".to_string(),
                            why: "Submissions GLPI threw away. Exposed by neither API.".to_string(),
                            granted: true,
                        }
                        DbRightRow {
                            table_: "glpi_logs".to_string(),
                            right: "SELECT".to_string(),
                            why: "GLPI's own change history - what altered between runs.".to_string(),
                            granted: true,
                        }
                        DbRightRow {
                            table_: "glpi_rules".to_string(),
                            right: "not granted".to_string(),
                            why: "Would turn a rule id into its name. Deliberately left out -                                   the id is shown and linked into GLPI instead."
                                .to_string(),
                            granted: false,
                        }
                        DbRightRow {
                            table_: "everything else".to_string(),
                            right: "not granted".to_string(),
                            why: "Including every table the API already answers for. Entity                                   names come from /api/entities, not from glpi_entities."
                                .to_string(),
                            granted: false,
                        }
                    }
                }
            }

            div { class: "mt-4",
                div { class: "mb-1 text-xs text-base-content/60",
                    "The grant this expects, in full:"
                }
                pre { class: "overflow-x-auto rounded bg-base-200 p-3 text-xs",
                    "CREATE USER 'pde'@'localhost' IDENTIFIED VIA unix_socket;\n"
                    "GRANT SELECT ON glpi.glpi_refusedequipments TO 'pde'@'localhost';\n"
                    "GRANT SELECT ON glpi.glpi_logs              TO 'pde'@'localhost';\n"
                    "FLUSH PRIVILEGES;"
                }
            }
        }

        div { class: "mt-5", DbDefinedRights {} }
    }
}

/// The other database angle on rights, and the one that is easy to miss: a
/// GLPI right whose NAME comes from a database row rather than from its code.
///
/// Distinct from the grant above, which is MariaDB's own permission system.
/// This is GLPI's, with the definition stored as data - so the set of right
/// names on the Profiles tab is not fixed, which is the part that matters to
/// anyone reading that tab.
#[component]
fn DbDefinedRights() -> Element {
    rsx! {
        Panel {
            title: "Rights GLPI defines from database rows".to_string(),
            subtitle: "Why the right names on the Profiles tab are not a fixed list"
                .to_string(),
            info: "Almost every GLPI right is named in its code. Custom asset and dropdown \
                   types are the exception: defining one creates a right named after it."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "How it works",
                    "A custom type is a row describing it. GLPI builds the class for that type \
                     as it starts and takes the right's name from the row, so a type whose \
                     internal name is kiosk gets a right called asset_kiosk. It then keeps the \
                     rights entries in step as the definition's profiles are edited. No code \
                     is changed anywhere in GLPI to make that happen.",
                ),
                InfoSection::new(
                    "Why it matters on this page",
                    "Because it means the Profiles tab is listing a set that can grow without \
                     anything being upgraded. That tab counts its right names from what GLPI \
                     reports rather than from a list held in this app, so a new type appears \
                     there by itself - but the grouping into tabs comes from a copy taken \
                     from GLPI, so a right GLPI invented today starts out under \
                     \u{201c}Not on the profile form\u{201d}. For an active custom asset type \
                     that is temporary and refreshing the copy places it. For two cases it is \
                     permanent, explained next.",
                ),
                InfoSection::new(
                    "Two kinds never get grouped",
                    "An INACTIVE custom asset type, and every custom DROPDOWN type. GLPI \
                     builds the profile form from its active asset definitions only, and does \
                     not add dropdown definitions to that form at all - while creating the \
                     rights for both regardless. So those rights exist, are granted per \
                     profile and are enforced, but GLPI never lists them on the form this \
                     page groups by. Refreshing the copy will not move them, however often it \
                     is run. They stay where they are, which remains the honest place: this \
                     app genuinely has no heading for them.",
                ),
                InfoSection::new(
                    "Not the same as the grant above",
                    "Two different permission systems. The grant above is MariaDB deciding \
                     what this app may read. This is GLPI deciding what a profile may do, with \
                     the definition of the right stored as data instead of written in code. \
                     Neither has anything to do with the other; they share only the word \
                     database.",
                ),
                InfoSection::new(
                    "Not observed here",
                    "This installation defines no custom types, so nothing on this page has \
                     ever come from one. The behaviour above was read in GLPI's source and \
                     confirmed only as far as the empty result - the addresses for custom \
                     types exist as templates with nothing behind them.",
                ),
            ],

            div { class: "overflow-x-auto",
                table { class: "table table-sm",
                    thead {
                        tr {
                            th { "Defined in" }
                            th { "Right name" }
                            th { "Which rights" }
                        }
                    }
                    tbody {
                        tr {
                            td { class: "text-sm", "GLPI's code" }
                            td { class: "font-mono text-xs", "fixed" }
                            td { class: "text-xs text-base-content/70",
                                "Almost all of them - agent, computer, entity, ticket and the rest.                                  A fixed property on the class behind each item."
                            }
                        }
                        tr {
                            td { class: "text-sm", "A database row" }
                            td { class: "font-mono text-xs", "asset_<name>" }
                            td { class: "text-xs text-base-content/70",
                                "One per custom asset type. Named from the type's own internal                                  name, with its rights entries created and kept in step                                  automatically."
                            }
                        }
                        tr {
                            td { class: "text-sm", "A database row" }
                            td { class: "font-mono text-xs", "dropdown_<name>" }
                            td { class: "text-xs text-base-content/70",
                                "The same arrangement for custom dropdown types."
                            }
                        }
                    }
                }
            }

            p { class: "mt-4 max-w-prose text-sm text-base-content/80",
                "So defining a custom asset type called Kiosk in GLPI produces three things at                  once with no change to this app or to GLPI's code: an address for it in the                  API, a right called "
                span { class: "font-mono", "asset_kiosk" }
                ", and that right's entries for every profile. The DB page covers the wider                  question of what else about GLPI is data and what is program."
            }
        }
    }
}

#[component]
fn DbRightRow(table_: String, right: String, why: String, granted: bool) -> Element {
    rsx! {
        tr { class: if granted { "" } else { "opacity-60" },
            td { class: "font-mono text-xs whitespace-nowrap", "{table_}" }
            td {
                span {
                    class: if granted {
                        "badge badge-success badge-sm font-mono"
                    } else {
                        "badge badge-ghost badge-sm font-mono"
                    },
                    "{right}"
                }
            }
            td { class: "text-xs text-base-content/70", "{why}" }
        }
    }
}
