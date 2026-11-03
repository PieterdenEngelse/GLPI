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
use crate::components::{InfoButton, InfoSection};
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
