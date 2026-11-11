//! The complement of the Agents page.
//!
//! That page asks which agents stopped reporting. This asks which machines have
//! no agent at all - a failure GLPI hides even more completely, because an
//! orphaned asset does not go quiet, it simply never changes again, which
//! looks exactly like hardware that has not changed.
use crate::api::{self, ComputerStatus, Named};
use crate::components::{InfoLink, InfoSection, Panel, RefreshControl, StatTile};
use dioxus::prelude::*;

const REFRESH_MS: u32 = 5 * 60 * 1000;
const REFRESH_LABEL: &str = "5m";

#[component]
pub fn Computers() -> Element {
    let mut rows = use_resource(|| async move { api::fetch_computers().await });

    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(REFRESH_MS).await;
            rows.restart();
        }
    });

    rsx! {
        match &*rows.read_unchecked() {
            Some(Err(e)) => rsx! {
                div { class: "alert alert-error",
                    div {
                        div { class: "font-semibold", "Could not reach the backend." }
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                    }
                }
            },
            Some(Ok(report)) => {
                let list = &report.computers;
                let orphans = list.iter().filter(|c| c.orphaned).count();
                rsx! {
                    div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
                        StatTile {
                            title: "Computers".to_string(),
                            value: list.len().to_string(),
                            desc: "assets in GLPI".to_string(),
                            body: "Machines GLPI holds as assets. Most are created by an \
                                   inventory agent the first time it reports, which then \
                                   keeps the record current; a record can also be created by \
                                   hand, in which case nothing will ever update it unless an \
                                   agent is later installed and matched to it. The count can \
                                   surprise you, because GLPI keeps an asset after the \
                                   machine is gone - retiring hardware does not remove it, \
                                   somebody has to - and a rebuilt machine can produce a \
                                   second record rather than updating the first, if the \
                                   identity the agent reports has changed."
                                .to_string(),
                            links: vec![InfoLink::new("Computers", "front/computer.php")],
                        }
                        StatTile {
                            title: "Without an agent".to_string(),
                            value: orphans.to_string(),
                            alarm: orphans > 0,
                            desc: "nothing will update these".to_string(),
                            body: "Computers that no agent reports for. Their details are frozen at \
                                   whatever was last recorded, and nothing will ever correct them."
                                .to_string(),
                            sections: vec![
                                InfoSection::new(
                                    "Why this is hard to spot",
                                    "An agent that stops reporting at least goes quiet, and the Agents \
                                     page can measure that silence. A machine with no agent never goes \
                                     quiet - it simply never changes again, which looks exactly like \
                                     hardware that has not changed. GLPI shows it beside every other \
                                     asset with nothing to distinguish it.",
                                ),
                                InfoSection::new(
                                    "Common causes",
                                    "The agent was never installed; it was removed or its package \
                                     uninstalled; the machine was retired and the asset kept; or the \
                                     record was created by hand and never matched to a real agent.",
                                ),
                                InfoSection::new(
                                    "How this is worked out",
                                    "Each agent in GLPI points at the item it inventories. This counts \
                                     computers that no agent points at, matching on the record GLPI \
                                     itself stores - so it reflects GLPI's own linkage, not a guess \
                                     based on names.",
                                ),
                            ],
                            links: vec![
                                InfoLink::new("Computers", "front/computer.php"),
                                InfoLink::new("Agents", "front/agent.php"),
                            ],
                        }
                    }

                    Panel {
                        title: "Computers".to_string(),
                        subtitle: "Without an agent first, then least recently inventoried".to_string(),
                        info: "The counterpart to the Agents page. That page asks which \
                               agents stopped reporting; this asks which machines have no \
                               agent at all. Agent says whether anything is keeping the \
                               record up to date. Last inventory is when GLPI last changed \
                               it, and Age is the same figure in plain words. Serial comes \
                               from the machine's firmware, so it survives a rebuild and is \
                               the most reliable way to tell two records apart. Entity is \
                               which part of GLPI the record lives in. Machines without an \
                               agent come first, then least recently inventoried: both are \
                               reasons to look, and having no agent is the worse of the two \
                               because it will never resolve on its own."
                            .to_string(),
                        info_links: vec![InfoLink::new("Computers", "front/computer.php")],
                        actions: rsx! {
                            RefreshControl {
                                interval_label: REFRESH_LABEL.to_string(),
                                on_refresh: move |_| rows.restart(),
                            }
                        },

                        // What this list leaves out, stated rather than
                        // implied. GLPI returns soft-deleted machines unless
                        // asked not to, and a filtered list that does not say
                        // it is filtered is the same problem as an empty list
                        // that does not say it failed to look.
                        match report.deleted_excluded {
                            Some(0) => rsx! {},
                            Some(n) => rsx! {
                                div { class: "mb-3 text-sm text-base-content/70",
                                    "{n} deleted machine(s) are in GLPI's trash and are not listed. "
                                    "They are excluded because a deleted machine has no agent "
                                    "reporting for it, so it would appear here as an orphan."
                                }
                            },
                            None => rsx! {
                                div { class: "mb-3 text-sm text-base-content/70",
                                    "Deleted machines are excluded from this list, but how many "
                                    "could not be established - so this is not a claim that none "
                                    "were hidden."
                                }
                            },
                        }

                        if list.is_empty() {
                            div { class: "text-base-content/60",
                                "No computers in GLPI yet. That is different from an error: "
                                "nothing has been inventoried."
                            }
                        } else {
                            ComputerTable { computers: list.clone() }
                        }
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

#[component]
fn ComputerTable(computers: Vec<ComputerStatus>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "Agent" }
                        th { "Computer" }
                        th { "Type" }
                        th { "Age" }
                        th { "Serial" }
                        th { "Entity" }
                    }
                }
                tbody {
                    for c in computers.iter() {
                        tr { class: if c.orphaned { "bg-error/5" } else { "" },
                            td {
                                if c.orphaned {
                                    span { class: "badge badge-error badge-sm", "none" }
                                } else {
                                    span { class: "badge badge-success badge-sm", "yes" }
                                }
                            }
                            td {
                                div { class: "font-medium", "{c.name.clone().unwrap_or_else(|| \"(unnamed)\".into())}" }
                                // Skipped entirely when GLPI has neither, rather
                                // than leaving a blank line under the name.
                                if !Named::label(&c.manufacturer).is_empty()
                                    || !Named::label(&c.model).is_empty()
                                {
                                    div { class: "text-xs text-base-content/60",
                                        "{Named::label(&c.manufacturer)} {Named::label(&c.model)}"
                                    }
                                }
                            }
                            td { class: "text-base-content/80", "{Named::label(&c.kind)}" }
                            // Exact timestamp on hover, as on the Agents page: Age
                            // answers the question, the raw value was the widest
                            // column here and answered it worst.
                            td {
                                class: "whitespace-nowrap",
                                title: "Last inventory: {c.last_inventory_update.clone().unwrap_or_else(|| \"never\".into())}",
                                match c.hours_since_inventory {
                                    Some(h) if h < 1 => "just now".to_string(),
                                    Some(h) if h < 48 => format!("{h}h ago"),
                                    Some(h) => format!("{}d ago", h / 24),
                                    None => "never".to_string(),
                                }
                            }
                            td { class: "font-mono text-xs text-base-content/70", "{c.serial.clone().unwrap_or_default()}" }
                            td { class: "text-base-content/70", "{Named::label(&c.entity)}" }
                        }
                    }
                }
            }
        }
    }
}
