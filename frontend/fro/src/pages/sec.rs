//! Security posture: four high-level questions about this install.
//!
//! Three states, never two. Most of what matters here cannot be fully
//! determined - a password cannot be verified, only observed never to have
//! changed - so "unknown" is a first-class answer and is never coloured like
//! "fine". That is the same rule the rest of this app applies to an empty
//! list versus a failed fetch.
//!
//! There is deliberately no overall score. One number hides which check
//! failed and invites tuning the number rather than the system.
use crate::api::{self, Finding, PostureTile};
use crate::components::{InfoButton, InfoSection, Panel};
use crate::glpi_link;
use dioxus::prelude::*;

#[component]
pub fn Sec() -> Element {
    let tiles = use_resource(|| async move { api::fetch_posture().await });

    rsx! {
        match &*tiles.read_unchecked() {
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not read the posture checks." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },
            Some(Ok(list)) => rsx! {
                p { class: "mb-5 max-w-prose text-sm text-base-content/70",
                    "What this app and the GLPI it reads are currently configured to allow. "
                    "Every check says what it cannot establish, because a check you trust "
                    "further than it reaches is worse than no check."
                }
                div { class: "grid gap-4 lg:grid-cols-2",
                    for t in list.iter() {
                        TileCard { key: "{t.id}", tile: t.clone() }
                    }
                }

                // Below the tiles, and each fetching on its own. The tiles are
                // the page's answer and must not wait on either of these.
                div { class: "mt-6 grid gap-4",
                    GlpiHealthPanel {}
                    LoginSessionPanel {}
                }
            },
            None => rsx! {
                div { class: "flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Loading…"
                }
            },
        }
    }
}

/// Amber for attention, not red: these are observations about how something
/// is configured, not something that has broken. Red here would compete with
/// the Agents page, where red means a machine has actually gone quiet.
fn badge_for(state: &str) -> (&'static str, &'static str) {
    match state {
        "ok" => ("badge badge-success badge-sm", "ok"),
        "attention" => ("badge badge-warning badge-sm", "attention"),
        _ => ("badge badge-ghost badge-sm", "unknown"),
    }
}

#[component]
fn TileCard(tile: PostureTile) -> Element {
    let (badge, label) = badge_for(&tile.state);
    let attention = tile
        .findings
        .iter()
        .filter(|f| f.state == "attention")
        .count();

    rsx! {
        section { class: "rounded-box border border-base-300 bg-base-100 shadow-sm",
            div { class: "flex items-center justify-between gap-3 border-b border-base-300 px-4 py-3",
                div { class: "flex items-center gap-2",
                    h2 { class: "text-sm font-semibold tracking-tight", "{tile.title}" }
                    InfoButton {
                        title: tile.title.clone(),
                        body: "What this tile checks, and the limit of what it can tell you."
                            .to_string(),
                        sections: vec![
                            InfoSection::new("What it cannot check", &tile.cannot_check),
                            InfoSection::new(
                                "Why there is no score",
                                "Each finding stands on its own. A single number would hide \
                                 which check failed, and would invite improving the number \
                                 rather than the thing it measures.",
                            ),
                            InfoSection::new(
                                "Unknown is not ok",
                                "A check that could not run is shown as unknown and never as \
                                 passing. Most of what matters here is only partly \
                                 observable, so the difference is the point.",
                            ),
                        ],
                    }
                }
                div { class: "flex items-center gap-2",
                    if attention > 0 {
                        span { class: "text-xs text-base-content/60", "{attention} to look at" }
                    }
                    span { class: "{badge}", "{label}" }
                }
            }

            ul { class: "divide-y divide-base-300",
                for f in tile.findings.iter() {
                    FindingRow { key: "{f.label}", finding: f.clone() }
                }
            }
        }
    }
}

#[component]
fn FindingRow(finding: Finding) -> Element {
    let (badge, _) = badge_for(&finding.state);
    let href = finding.link.as_deref().and_then(glpi_link::link);

    rsx! {
        li { class: "px-4 py-3",
            div { class: "flex items-start gap-2",
                span { class: "{badge} mt-0.5 shrink-0" }
                div { class: "min-w-0",
                    div { class: "text-sm font-medium", "{finding.label}" }
                    p { class: "mt-0.5 text-sm leading-relaxed text-base-content/70",
                        "{finding.detail}"
                    }
                    if let Some(h) = href {
                        a {
                            class: "link link-primary mt-1 inline-block text-xs",
                            href: "{h}",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "Change this in GLPI ↗"
                        }
                    }
                }
            }
        }
    }
}

/// GLPI's own health, with its automatic actions as the headline.
///
/// This belongs on this page rather than the Status page because it is about
/// the install's condition rather than about whether this dashboard can read
/// it. The headline is the scheduled work: a GLPI task that stopped running
/// is the same failure this app was built to find one layer down, and GLPI
/// reports it while nothing here read it before.
#[component]
fn GlpiHealthPanel() -> Element {
    let status = use_resource(|| async move { api::fetch_system_status().await });

    rsx! {
        Panel {
            title: "GLPI's own health".to_string(),
            subtitle: "What the install says about itself".to_string(),
            info: "GLPI runs scheduled work of its own, and a task that stops looks exactly \
                  like a task with nothing to do - the same blind spot this app exists to \
                  close for inventory agents. Why a stuck task matters here: GLPI's \
                  automatic actions are what keep its own data moving. One that has hung \
                  reports nothing and blocks nothing visibly; the only trace is in this \
                  status check. 12.0 added a separate list for actions that failed \
                  repeatedly and now need a human, which is a stronger signal than hung. Why \
                  the headline figure is not shown: GLPI publishes an overall status, and it \
                  is the rollup across every service written back into the GLPI entry - so \
                  it reads as a problem whenever anything at all is unhappy, including a \
                  stuck task that has nothing to do with GLPI's own health. The schema check \
                  below is GLPI's actual condition; the rollup is shown separately and \
                  labelled as such. Unknown is most of this install: LDAP, IMAP, CAS and \
                  mail collection all report no data here because none of them is \
                  configured. That is not a fault and is not shown as one - but it is also \
                  not health, so it reads as unknown."
                .to_string(),
            match &*status.read_unchecked() {
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Asking GLPI about itself…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Could not read GLPI's status." }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },
                Some(Ok(s)) => {
                    let (cron_badge, cron_label) = badge_for(&s.crontask_state);
                    let (schema_badge, schema_label) = badge_for(&s.schema_state);
                    rsx! {
                        // The automatic actions first: it is the only part of
                        // this that is this app's own subject.
                        div { class: "mb-4",
                            div { class: "mb-1 flex flex-wrap items-center gap-2",
                                span { class: "font-semibold", "Automatic actions" }
                                span { class: "{cron_badge}", "{cron_label}" }
                            }
                            if s.stuck.is_empty() && s.errored.is_empty() {
                                div { class: "text-sm text-base-content/70",
                                    "Nothing hung and nothing in error."
                                }
                            }
                            if !s.stuck.is_empty() {
                                div { class: "mt-1 text-sm",
                                    span { class: "text-base-content/70", "Hung: " }
                                    for n in s.stuck.iter() {
                                        span { class: "badge badge-warning badge-outline badge-sm font-mono mr-1",
                                            "{n}"
                                        }
                                    }
                                }
                            }
                            if !s.errored.is_empty() {
                                div { class: "mt-1 text-sm",
                                    span { class: "text-base-content/70",
                                        "Failed repeatedly, needs a human: "
                                    }
                                    for n in s.errored.iter() {
                                        span { class: "badge badge-warning badge-sm font-mono mr-1",
                                            "{n}"
                                        }
                                    }
                                }
                            }
                            if let Some(msg) = s.crontask_summary.as_ref() {
                                div { class: "mt-1 font-mono text-xs text-base-content/50", "{msg}" }
                            }
                        }

                        div { class: "mb-4",
                            div { class: "mb-1 flex flex-wrap items-center gap-2",
                                span { class: "font-semibold", "Database schema" }
                                span { class: "{schema_badge}", "{schema_label}" }
                            }
                            if s.redacted {
                                // Not a schema problem: GLPI hid the answer.
                                div { class: "text-sm text-base-content/70",
                                    "GLPI served the anonymous view of this check, which reports "
                                    "a current install as out of date regardless. Nothing can be "
                                    "concluded from it - and this app should not be seeing that "
                                    "view, so the reading itself is the finding."
                                }
                            } else if let Some(installed) = s.schema_installed.as_ref() {
                                div { class: "font-mono text-xs text-base-content/70",
                                    "{installed}"
                                }
                                if s.schema_defined.as_ref() != Some(installed) {
                                    div { class: "mt-1 text-sm",
                                        "The code expects "
                                        span { class: "font-mono", "{s.schema_defined.clone().unwrap_or_default()}" }
                                        " - a migration has not been run."
                                    }
                                }
                            }
                        }

                        div { class: "overflow-x-auto",
                            table { class: "table table-sm",
                                thead {
                                    tr {
                                        th { "Service" }
                                        th { "State" }
                                        th { "GLPI's word" }
                                    }
                                }
                                tbody {
                                    for svc in s.services.iter() {
                                        ServiceRow { key: "{svc.name}", svc: svc.clone(), overall: s.overall.clone() }
                                    }
                                }
                            }
                        }

                        p { class: "mt-3 max-w-prose text-xs text-base-content/60", "{s.cannot_check}" }
                    }
                },
            }
        }
    }
}

/// One service row.
#[component]
fn ServiceRow(svc: api::StatusService, overall: String) -> Element {
    let (badge, label) = badge_for(&svc.state);

    // The `glpi` row's status is the global rollup rather than that service's
    // own health, so it is labelled instead of being shown as a verdict about
    // GLPI. See glpi::system_status for why GLPI writes it there.
    let is_rollup = svc.name == "glpi";

    rsx! {
        tr {
            td { class: "font-mono text-xs",
                "{svc.name}"
                if is_rollup {
                    span { class: "ml-2 badge badge-ghost badge-xs", "rollup of all services" }
                }
            }
            td {
                if is_rollup {
                    span { class: "{badge_for(&overall).0}", "{badge_for(&overall).1}" }
                } else {
                    span { class: "{badge}", "{label}" }
                }
            }
            td { class: "font-mono text-xs text-base-content/50",
                "{svc.raw.clone().unwrap_or_default()}"
            }
        }
    }
}

/// Who is logged into GLPI, and whether this account may see all of them.
#[component]
fn LoginSessionPanel() -> Element {
    let report = use_resource(|| async move { api::fetch_login_sessions().await });

    rsx! {
        Panel {
            title: "Login sessions".to_string(),
            subtitle: "New in GLPI 12".to_string(),
            info: "Who holds a live session in GLPI, authenticating how, and from where. GLPI \
                  11 could not answer this at any API version. Read the scope before the \
                  list: GLPI narrows this list to the account's own sessions unless that \
                  account may change configuration - and it answers successfully either way, \
                  without saying it narrowed anything. So under a narrowed scope a short \
                  list means \u{201c}only mine\u{201d} and says nothing whatever about who \
                  else is signed in. The scope is stated above the table for that reason. \
                  Why this app's own sessions dominate: Each time this dashboard \
                  authenticates, GLPI records a session, and an API sign-in never explicitly \
                  ends - it expires. So these accumulate, and most of them are this app. \
                  GLPI 12 added a scheduled task to purge old login history, which is the \
                  intended answer to that growth. It starts at the upgrade: The table behind \
                  this is new in GLPI 12, so it holds nothing from before this install was \
                  upgraded. An empty or short history is not evidence about the past."
                .to_string(),
            match &*report.read_unchecked() {
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Reading sessions…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Could not read the login sessions." }
                            div { class: "text-sm",
                                "On a GLPI older than 12 this endpoint does not exist, which "
                                "reads as a 404 rather than as an empty list."
                            }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },
                Some(Ok(r)) => rsx! {
                    div { class: "mb-4 flex flex-wrap items-center gap-2 text-sm",
                        match r.scope.as_str() {
                            "fleet" => rsx! {
                                span { class: "badge badge-success badge-sm", "every user" }
                                span { class: "text-base-content/70",
                                    "This account may see all sessions, so the list is complete."
                                }
                            },
                            "self_only" => rsx! {
                                span { class: "badge badge-warning badge-sm", "this account only" }
                                span { class: "text-base-content/70",
                                    "GLPI narrowed this to this app's own sessions. It says "
                                    "nothing about who else is signed in."
                                }
                            },
                            _ => rsx! {
                                span { class: "badge badge-ghost badge-sm", "scope unknown" }
                                span { class: "text-base-content/70",
                                    "This account's own rights could not be read, so whether the "
                                    "list is complete is unknown - it is not assumed to be."
                                }
                            },
                        }
                    }

                    if r.sessions.is_empty() {
                        div { class: "text-sm text-base-content/70",
                            "No sessions recorded."
                            if r.scope != "fleet" {
                                " Under the scope above, that is not the same as nobody being signed in."
                            }
                        }
                    } else {
                        div { class: "overflow-x-auto",
                            table { class: "table table-sm",
                                thead {
                                    tr {
                                        th { "User" }
                                        th { "How" }
                                        th { "From" }
                                        th { "Signed in" }
                                        th { "Last used" }
                                        th { "Ended" }
                                    }
                                }
                                tbody {
                                    for s in r.sessions.iter() {
                                        tr { key: "{s.id.unwrap_or_default()}",
                                            td { "{s.user.clone().unwrap_or_default()}" }
                                            td { class: "text-xs", "{s.auth_label}" }
                                            td { class: "font-mono text-xs",
                                                "{s.ip_address.clone().unwrap_or_default()}"
                                            }
                                            td { class: "font-mono text-xs",
                                                "{s.logged_in_at.clone().unwrap_or_default()}"
                                            }
                                            td { class: "font-mono text-xs",
                                                "{s.last_activity_at.clone().unwrap_or_default()}"
                                            }
                                            td { class: "text-xs",
                                                match s.logged_out_at.as_ref() {
                                                    Some(t) => rsx! { span { class: "font-mono", "{t}" } },
                                                    None => rsx! {
                                                        span { class: "badge badge-ghost badge-sm", "still open" }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    p { class: "mt-3 max-w-prose text-xs text-base-content/60", "{r.cannot_check}" }
                },
            }
        }
    }
}
