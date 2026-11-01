use crate::api::AgentStatus;
use crate::components::StatTile;
use dioxus::prelude::*;

/// The four numbers worth seeing before the table.
///
/// Derived here rather than sent by the app's API: they are projections of
/// the same list the table already shows, and a second source could disagree
/// with the rows underneath it. A dashboard contradicting itself is worse
/// than one that computes twice.
#[component]
pub fn StatRow(agents: Vec<AgentStatus>) -> Element {
    let total = agents.len();
    let stale = agents.iter().filter(|a| a.stale).count();
    let reporting = total - stale;

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
            StatTile {
                title: "Agents".to_string(),
                value: total.to_string(),
                desc: "registered in GLPI".to_string(),
                info: "Every inventory agent GLPI knows about. An agent is a small program \
                       running on a machine that collects what hardware and software it has \
                       and sends that to GLPI, normally once a day. One agent is usually one \
                       machine.".to_string(),
            }
            StatTile {
                title: "Reporting".to_string(),
                value: reporting.to_string(),
                desc: "seen within 26h".to_string(),
                info: "Agents that have checked in during the last 26 hours. Twenty-six and \
                       not twenty-four because the agent runs daily on a timer that is \
                       deliberately given a few minutes of random delay, so a strict day \
                       would flag perfectly healthy machines whose run happened to drift.".to_string(),
            }
            StatTile {
                title: "Stale".to_string(),
                value: stale.to_string(),
                alarm: stale > 0,
                desc: "overdue or never seen".to_string(),
                info: "Agents that have not checked in for more than 26 hours, or that have \
                       never checked in at all. This is the number this app exists for: GLPI \
                       has no notion of an agent being overdue, so it goes on showing the \
                       machine as though nothing were wrong. An agent that registered and was \
                       never seen counts here too - silent from the start is a worse sign \
                       than late, not an unknown.".to_string(),
            }
            StatTile {
                title: "Oldest contact".to_string(),
                value: oldest,
                desc: "worst case of any agent".to_string(),
                info: "How long ago the least recently seen agent last checked in. If this \
                       figure is small, nothing has gone quiet; it is the one number that \
                       summarises every agent at once.".to_string(),
            }
        }
    }
}
