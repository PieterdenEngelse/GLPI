//! What this app judges, and what it cannot see.
//!
//! RERAG treats its UI as a place to make the invisible visible. The
//! equivalent here is small but matters more: a dashboard about missing
//! machines has to be explicit about its own blind spots, because a machine
//! this app cannot see looks exactly like a machine that does not exist.
use dioxus::prelude::*;

#[component]
pub fn About() -> Element {
    rsx! {
        section { class: "prose",
            h2 { "What this answers" }
            p {
                "GLPI shows the computer an agent created and goes on showing it, "
                "looking healthy, long after the agent stopped reporting. Nothing "
                "in GLPI marks an agent as overdue. This page does."
            }

            h2 { "When an agent counts as stale" }
            p {
                "26 hours without contact, not 24. The agent timer runs daily with "
                "a RandomizedDelaySec, so a strict 24 would flag healthy machines "
                "whose run drifted by minutes. An agent registered but never seen "
                "counts as stale too - silent from the start is worse than late, "
                "not a neutral unknown."
            }

            h2 { "What it cannot see" }
            dl {
                dt { "Rejected submissions" }
                dd {
                    "GLPI's API does not expose RefusedEquipment. That is the "
                    "failure worth surfacing most: the agent exits 0, GLPI rejects "
                    "the submission, the asset silently never appears, and the "
                    "agent's own log says nothing."
                }
                dt { "What changed between runs" }
                dd { "Field-level history is not reachable through the API either." }
                dt { "Agents outside this account's entities" }
                dd {
                    "GLPI filters every query by the entities the service account "
                    "can reach. An agent in an entity it cannot see is absent from "
                    "this table, and absent looks identical to gone."
                }
            }
        }
    }
}
