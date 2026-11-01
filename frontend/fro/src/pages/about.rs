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
        section { class: "max-w-prose",
            h2 { class: "mb-1 text-base font-semibold", "What this answers" }
            p { class: "mb-3 text-base-content/80",
                "GLPI shows the computer an agent created and goes on showing it, "
                "looking healthy, long after the agent stopped reporting. Nothing "
                "in GLPI marks an agent as overdue. This page does."
            }

            h2 { class: "mt-7 mb-1 text-base font-semibold", "When an agent counts as stale" }
            p { class: "mb-3 text-base-content/80",
                "26 hours without contact, not 24. The agent timer runs daily with "
                "a RandomizedDelaySec, so a strict 24 would flag healthy machines "
                "whose run drifted by minutes. An agent registered but never seen "
                "counts as stale too - silent from the start is worse than late, "
                "not a neutral unknown."
            }

            h2 { class: "mt-7 mb-1 text-base font-semibold", "What it cannot see" }
            dl { class: "mt-2",
                dt { class: "mt-4 font-medium", "Rejected submissions" }
                dd { class: "mt-1 text-base-content/70",
                    "GLPI's API does not expose RefusedEquipment. That is the "
                    "failure worth surfacing most: the agent exits 0, GLPI rejects "
                    "the submission, the asset silently never appears, and the "
                    "agent's own log says nothing."
                }
                dt { class: "mt-4 font-medium", "What changed between runs" }
                dd { class: "mt-1 text-base-content/70", "Field-level history is not reachable through the API either." }
                dt { class: "mt-4 font-medium", "Agents outside this account's entities" }
                dd { class: "mt-1 text-base-content/70",
                    "GLPI filters every query by the entities the service account "
                    "can reach. An agent in an entity it cannot see is absent from "
                    "this table, and absent looks identical to gone."
                }
            }
        }
    }
}
