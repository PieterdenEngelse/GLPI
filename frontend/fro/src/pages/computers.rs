//! The complement of the Fleet page.
//!
//! Fleet asks which agents stopped reporting. This asks which machines have
//! no agent at all - a failure GLPI hides even more completely, because an
//! orphaned asset does not go quiet, it simply never changes again, which
//! looks exactly like hardware that has not changed.
use crate::api::{self, ComputerStatus, Named};
use crate::components::{Panel, RefreshControl};
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
            Some(Ok(list)) => {
                let orphans = list.iter().filter(|c| c.orphaned).count();
                rsx! {
                    div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
                        div { class: "stat",
                            div { class: "stat-title", "Computers" }
                            div { class: "stat-value text-2xl", "{list.len()}" }
                            div { class: "stat-desc", "assets in GLPI" }
                        }
                        div { class: "stat",
                            div { class: "stat-title", "Without an agent" }
                            div {
                                class: if orphans > 0 { "stat-value text-2xl text-error" } else { "stat-value text-2xl" },
                                "{orphans}"
                            }
                            div { class: "stat-desc", "nothing will update these" }
                        }
                    }

                    Panel {
                        title: "Computers".to_string(),
                        subtitle: "Without an agent first, then least recently inventoried".to_string(),
                        actions: rsx! {
                            RefreshControl {
                                interval_label: REFRESH_LABEL.to_string(),
                                on_refresh: move |_| rows.restart(),
                            }
                        },

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
                        th { "Last inventory" }
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
                                div { class: "text-xs text-base-content/60",
                                    "{Named::label(&c.manufacturer)} {Named::label(&c.model)}"
                                }
                            }
                            td { class: "text-base-content/80", "{Named::label(&c.kind)}" }
                            td { class: "whitespace-nowrap text-base-content/80",
                                "{c.last_inventory_update.clone().unwrap_or_else(|| \"never\".into())}"
                            }
                            td { class: "whitespace-nowrap",
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
