use crate::api::AgentStatus;
use dioxus::prelude::*;

/// The four numbers worth seeing before the table.
///
/// Derived here rather than sent by the backend: they are projections of the
/// same list the table already has, and a second endpoint could disagree with
/// the rows underneath it - a dashboard contradicting itself is worse than a
/// dashboard that computes twice.
#[component]
pub fn StatRow(agents: Vec<AgentStatus>) -> Element {
    let total = agents.len();
    let stale = agents.iter().filter(|a| a.stale).count();
    let reporting = total - stale;

    // The worst case, which is the number you actually came for.
    let oldest = agents
        .iter()
        .filter_map(|a| a.hours_since_contact)
        .max()
        .map(|h| {
            if h < 48 {
                format!("{h}h")
            } else {
                format!("{}d", h / 24)
            }
        })
        .unwrap_or_else(|| "—".into());

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
            div { class: "stat",
                div { class: "stat-title", "Agents" }
                div { class: "stat-value text-2xl", "{total}" }
                div { class: "stat-desc", "registered in GLPI" }
            }
            div { class: "stat",
                div { class: "stat-title", "Reporting" }
                div { class: "stat-value text-2xl text-success", "{reporting}" }
                div { class: "stat-desc", "seen within 26h" }
            }
            div { class: "stat",
                div { class: "stat-title", "Stale" }
                // Zero is not an alarm, so it is not coloured like one. A red
                // 0 trains the reader to ignore the colour.
                div {
                    class: if stale > 0 { "stat-value text-2xl text-error" } else { "stat-value text-2xl" },
                    "{stale}"
                }
                div { class: "stat-desc", "overdue or never seen" }
            }
            div { class: "stat",
                div { class: "stat-title", "Oldest contact" }
                div { class: "stat-value text-2xl", "{oldest}" }
                div { class: "stat-desc", "worst case in the fleet" }
            }
        }
    }
}
