use crate::api::AgentStatus;
use crate::components::{InfoLink, InfoSection, StatTile};
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
    let not_landing = agents.iter().filter(|a| a.not_landing).count();
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
        .unwrap_or_else(|| "\u{2014}".into());

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",

            StatTile {
                title: "Agents".to_string(),
                value: total.to_string(),
                desc: "registered in GLPI".to_string(),
                body: "Every inventory agent GLPI knows about. An agent is a small program \
                       running on a machine that collects what hardware and software it has \
                       and sends that to GLPI, normally once a day.".to_string(),
                sections: vec![
                    InfoSection::new(
                        "What it counts",
                        "One row per registered agent, not per machine. A machine that was \
                         rebuilt and enrolled again can appear twice, under two identifiers, \
                         until somebody removes the old record - so this number can exceed \
                         the number of physical machines you own.",
                    ),
                    InfoSection::new(
                        "Where it comes from",
                        "GLPI's own list of agents, read through its API. This app never \
                         creates, changes or deletes anything in GLPI; everything you see \
                         here exists there already.",
                    ),
                ],
                links: vec![InfoLink::new("Agents", "front/agent.php")],
            }

            StatTile {
                title: "Reporting".to_string(),
                value: reporting.to_string(),
                desc: "seen within 26h".to_string(),
                body: "Agents that have checked in during the last 26 hours.".to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why 26 hours and not 24",
                        "The agent runs once a day on a timer that is deliberately given a \
                         few minutes of random delay, so that a hundred machines do not all \
                         report in the same second. A strict 24 hours would flag perfectly \
                         healthy machines whose run happened to drift past the hour.",
                    ),
                    InfoSection::new(
                        "What it does not prove",
                        "That contact was made - not that anything useful arrived. GLPI \
                         records the check-in separately from the inventory it carries, so \
                         an agent can be counted here while the machine's details sit \
                         unchanged. The Computers page shows when each record was last \
                         actually updated.",
                    ),
                ],
                links: vec![InfoLink::new("Agents", "front/agent.php")],
            }

            StatTile {
                title: "Stale".to_string(),
                value: stale.to_string(),
                alarm: stale > 0,
                desc: "overdue or never seen".to_string(),
                body: "Agents that have not checked in for more than 26 hours, or that have \
                       never checked in at all.".to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why this app exists",
                        "GLPI has no notion of an agent being overdue. It shows the machine \
                         the agent created and goes on showing it, looking perfectly healthy, \
                         however long ago that agent went quiet. Nothing in GLPI turns \
                         amber. This number is the whole reason for this dashboard.",
                    ),
                    InfoSection::new(
                        "Never seen counts here",
                        "An agent that registered and was then never heard from is counted \
                         as stale, not as unknown. Silent from the very start is a worse \
                         sign than late: it usually means the agent was installed but could \
                         never reach GLPI, and nobody noticed because an asset did appear.",
                    ),
                    InfoSection::new(
                        "What to check",
                        "In rough order: the machine is off or off the network; the agent \
                         service stopped or was removed; the machine was retired but its \
                         agent record was not; or GLPI is refusing the submissions. That \
                         last one is invisible from here - GLPI does not expose rejected \
                         inventories through its API at all.",
                    ),
                ],
                links: vec![InfoLink::new("Agents", "front/agent.php")],
            }

            StatTile {
                title: "Not landing".to_string(),
                value: not_landing.to_string(),
                alarm: not_landing > 0,
                desc: "reporting, nothing stored".to_string(),
                body: "Agents that are checking in normally while nothing they send is \
                       being kept."
                    .to_string(),
                sections: vec![
                    InfoSection::new(
                        "Why this looks healthy everywhere else",
                        "GLPI records the moment of contact from the submission's own \
                         header, before it inspects what was sent. If an import rule then \
                         throws the submission away, the contact time has already been \
                         written - so the agent appears to be reporting, because it is. \
                         Nothing in GLPI's asset list says the data was discarded.",
                    ),
                    InfoSection::new(
                        "How it is detected",
                        "By comparing two dates GLPI does keep honestly: when the agent \
                         last made contact, and when its machine's record was last \
                         actually written. A machine whose details have not changed still \
                         gets its record stamped, so a gap between the two means the \
                         submissions are arriving and not being stored.",
                    ),
                    InfoSection::new(
                        "What to do",
                        "Look at the import rules in GLPI - something is matching this \
                         machine and refusing it. The refusal itself is recorded in GLPI's \
                         database but is not available through its API, so this page can \
                         say that it is happening but not yet which rule is responsible.",
                    ),
                ],
                links: vec![InfoLink::new("Import rules", "front/ruleimportasset.php")],
            }
            StatTile {
                title: "Oldest contact".to_string(),
                value: oldest,
                desc: "worst case of any agent".to_string(),
                body: "How long ago the least recently seen agent last checked in - the \
                       single worst case among every agent on this page.".to_string(),
                sections: vec![
                    InfoSection::new(
                        "How to read it",
                        "If this figure is small, nothing has gone quiet and the rest of the \
                         page needs no attention. It is the one number that summarises every \
                         agent at once, which is why it sits beside the totals rather than \
                         inside the table.",
                    ),
                    InfoSection::new(
                        "What it cannot tell you",
                        "Only agents this app is allowed to see are counted. If the account \
                         it signs in with cannot reach part of GLPI, machines there are \
                         missing from this figure with nothing to indicate it. The line under \
                         the table states that boundary, and the Status page explains it.",
                    ),
                ],
            }
        }
    }
}
