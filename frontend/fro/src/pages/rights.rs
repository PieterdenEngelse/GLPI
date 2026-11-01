//! Every right the account this app signs in with actually holds.
//!
//! The Status page answers "can this dashboard work at all" with the two
//! rights it needs. This one answers the different question an administrator
//! asks when setting that account up or auditing it afterwards: what else did
//! the profile grant, and does it grant more than a read-only dashboard has
//! any business holding?
//!
//! Neither question is answerable from GLPI's own screens without opening a
//! profile form and reading several tabs of checkboxes, and GLPI's API does
//! not expose rights per profile at all - only for the session's own active
//! profile. See backend/src/glpi/rights.rs for that constraint.
use crate::api::{self, RightEntry, RightsBreakdown};
use crate::components::{InfoLink, InfoSection, Panel, StatTile};
use dioxus::prelude::*;

#[component]
pub fn Rights() -> Element {
    let rights = use_resource(|| async move { api::fetch_rights().await });

    rsx! {
        match &*rights.read_unchecked() {
            // An unreachable backend must never render as "this account has no
            // rights" - on a permissions page that reads as a catastrophic
            // misconfiguration rather than as a failure to look.
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not read the profile's rights." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },

            Some(Ok(b)) => rsx! { Breakdown { breakdown: b.clone() } },

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
fn Breakdown(breakdown: RightsBreakdown) -> Element {
    let entries = breakdown.entries.clone();

    let total = entries.len();
    let granted: Vec<&RightEntry> = entries.iter().filter(|e| e.granted()).collect();
    let writes = granted.iter().filter(|e| e.writes).count();
    let read_only = granted.len() - writes;
    let module_defined = granted.iter().filter(|e| e.unnamed_bits != 0).count();

    let profile = breakdown
        .profile_name
        .clone()
        .unwrap_or_else(|| "unknown".into());

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",

            StatTile {
                title: "Granted".to_string(),
                value: format!("{} of {}", granted.len(), total),
                desc: "rights this profile holds".to_string(),
                body: "GLPI divides permission into one right per area of the application - \
                       computers, tickets, agents, and so on. This counts how many of them \
                       the profile behind this session has been given at least some access \
                       to."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why the total matters too",
                        "The denominator is every right GLPI knows about, so the rights this \
                         profile does NOT have are listed below as well, with a value of zero. \
                         That is deliberate: when GLPI refuses a request it never names the \
                         missing permission, so being able to see that a right exists and is \
                         not granted is usually the answer.",
                    ),
                    InfoSection::new(
                        "One profile, not the account",
                        "An account can be authorised for several profiles, but GLPI activates \
                         exactly one per session - the account's default - so this is the set \
                         that is actually in force. Holding another profile adds nothing until \
                         it becomes the active one.",
                    ),
                ],
                links: vec![InfoLink::new("Profiles", "front/profile.php")],
            }

            StatTile {
                title: "Read-only".to_string(),
                value: read_only.to_string(),
                desc: "look, but cannot change".to_string(),
                body: "Rights that grant only reading. This is what a dashboard needs: every \
                       page in this app is a view of something GLPI already holds, and it \
                       never creates, changes or deletes anything."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Reading notes counts as reading",
                        "Several rights here are a combination like READ plus READNOTE - \
                         permission to see an item and to see the free-text notes attached to \
                         it. Neither one changes anything, so both are counted as read-only.",
                    ),
                ],
            }

            StatTile {
                title: "Write-capable".to_string(),
                value: writes.to_string(),
                alarm: writes > 0,
                desc: "more than this app needs".to_string(),
                body: "Rights that include permission to change something - create, update, \
                       delete, purge, or unlock a field. This app never writes to GLPI, so \
                       anything counted here is access the dashboard does not use."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why it is worth flagging",
                        "Not a fault in itself - the profile may be shared with a person who \
                         does need to change those things. But it is worth knowing: the \
                         credentials this app holds can do everything the profile allows, and \
                         they sit in a file on the machine running it. A profile created only \
                         for this dashboard should have none of these.",
                    ),
                    InfoSection::new(
                        "What counts as a write",
                        "UPDATE, CREATE, DELETE and PURGE, plus UPDATENOTE for the notes on an \
                         item, UPDATE_ASSIGNED and UPDATE_OWNED for items assigned to or owned \
                         by the user, and UNLOCK - which clears the lock on a field imported by \
                         inventory so it can be edited by hand. That last one is not an obvious \
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
                         is not available through the API in any form. The alternative to showing \
                         the leftover as a number would be to drop it silently, which would \
                         understate what the account can do - the one direction a permissions \
                         screen must not be wrong in.",
                    ),
                ],
            }
        }

        if writes > 0 {
            div { class: "alert alert-warning mb-5",
                div {
                    div { class: "font-semibold",
                        "This profile can change things in GLPI."
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

        RightsTable { entries, profile }
    }
}

#[component]
fn RightsTable(entries: Vec<RightEntry>, profile: String) -> Element {
    // A plain text filter and one toggle, because 122 rows sorted by name is
    // not something anyone reads top to bottom - they arrive knowing which
    // right they are looking for, usually from a 403 that would not say.
    let mut query = use_signal(String::new);
    let mut granted_only = use_signal(|| false);

    let q = query().to_lowercase();
    let visible: Vec<RightEntry> = entries
        .iter()
        .filter(|e| !granted_only() || e.granted())
        .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
        .cloned()
        .collect();

    rsx! {
        Panel {
            title: "Rights of profile “{profile}”".to_string(),
            subtitle: format!("{} of {} shown, by name", visible.len(), entries.len()),
            info: "Every right GLPI listed for this session's active profile, granted or not."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "The columns",
                    "Right is GLPI's own internal name for it, left exactly as GLPI spells it \
                     so it can be searched for in GLPI's source or matched against a profile \
                     form. Value is the raw number GLPI returned. Permissions is that number \
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
                    "Where to change any of this",
                    "In GLPI under Administration, then Profiles, on the profile named in this \
                     panel's title. Changing a right there affects every account that uses the \
                     profile, so a profile dedicated to this app is safer to edit than a shared \
                     one.",
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

            if visible.is_empty() {
                // Distinct from the error state above on purpose: this is an
                // empty filter result, not a failure to read anything.
                div { class: "py-6 text-center text-sm text-base-content/60",
                    if entries.is_empty() {
                        "GLPI returned no rights for this session. That is what an API older "
                        "than version 2.2 looks like - it does not include them at all."
                    } else {
                        "No right matches that filter."
                    }
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                th { "Right" }
                                th { "Value" }
                                th { "Permissions" }
                            }
                        }
                        tbody {
                            for e in visible {
                                tr {
                                    key: "{e.name}",
                                    class: if e.granted() { "" } else { "opacity-50" },

                                    td { class: "font-mono text-xs whitespace-nowrap", "{e.name}" }
                                    td { class: "font-mono text-xs tabular-nums", "{e.value}" }
                                    td {
                                        if e.granted() {
                                            div { class: "flex flex-wrap items-center gap-1",
                                                for bit in e.bits.iter() {
                                                    span {
                                                        key: "{bit}",
                                                        class: if bit.starts_with("READ") {
                                                            "badge badge-ghost badge-sm font-mono"
                                                        } else {
                                                            "badge badge-warning badge-sm font-mono"
                                                        },
                                                        "{bit}"
                                                    }
                                                }
                                                if e.unnamed_bits != 0 {
                                                    // Shown as the leftover value, not as a
                                                    // guess at its name.
                                                    span {
                                                        class: "badge badge-outline badge-sm font-mono",
                                                        title: "Bits beyond GLPI's twelve standard permissions. Real access, defined by one part of GLPI for its own use, and not resolvable to a name through the API.",
                                                        "+{e.unnamed_bits}"
                                                    }
                                                }
                                            }
                                        } else {
                                            span { class: "text-xs text-base-content/50", "not granted" }
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
}
