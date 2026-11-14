use crate::api::{self, Entity};
use crate::components::{InfoButton, InfoLink};
use dioxus::prelude::*;

/// States which entities the data below was drawn from.
///
/// This cannot verify that a list is complete, and nothing can: GLPI filters
/// every collection by the entities the account may read, and the filtered
/// rows leave no trace in the response - no count, no header, nothing to
/// compare against. What it can do is put the scope on screen, where someone
/// who knows the estate has four entities will notice that one is listed.
///
/// It fetches its own data rather than taking it as a prop, the way RERAG's
/// Header polls health for itself: the note is about the account, not about
/// whichever table it happens to sit above.
#[component]
pub fn ScopeNote() -> Element {
    let entities = use_resource(|| async move { api::fetch_entities().await });

    // Muted by default - it qualifies the table rather than being content -
    // but error-coloured when the scope is unknown or empty, because a scope
    // that cannot be established undermines everything below it.
    const OK: &str = "text-xs text-base-content/60";
    const WARN: &str = "text-xs text-error";

    let (class, text) = match &*entities.read_unchecked() {
        None => (OK, "Scope: checking…".to_string()),

        // Never hide the line on failure. A page with no scope note reads as
        // "no limits apply", which is the opposite of what is known here.
        Some(Err(e)) => (
            WARN,
            format!("Scope: could not be read ({e}). The list below may be filtered."),
        ),

        // Readable but empty is a real state, and a bad one: an account with
        // no entities can read nothing, so an empty table below means nothing.
        Some(Ok(list)) if list.is_empty() => (
            WARN,
            "Scope: this account can read no entities, so nothing below is complete.".to_string(),
        ),

        Some(Ok(list)) => {
            let names = list
                .iter()
                .map(Entity::label)
                .collect::<Vec<_>>()
                .join(", ");
            let noun = if list.len() == 1 {
                "entity"
            } else {
                "entities"
            };
            (
                OK,
                format!(
                    "Scope: {} {noun} — {names}. Agents outside these are not listed.",
                    list.len()
                ),
            )
        }
    };

    rsx! {
        div { class: "mb-4 flex items-center gap-2",
            p { class: "{class}", "{text}" }
            InfoButton {
                title: "Scope".to_string(),
                body: "Which parts of GLPI this list was drawn from. Why it cannot promise \
                      completeness: GLPI filters every query by entity, and the rows it \
                      removes leave no trace in the answer - no count, no marker, nothing to \
                      compare against. So this line cannot verify that the list is complete, \
                      and nothing else can either. It states the boundary, so that somebody \
                      who knows the estate can notice when it is too narrow. How to widen \
                      it: Two separate settings, easy to confuse. Permission to list \
                      entities decides whether this line can be shown at all. Whether the \
                      account's authorization is recursive decides whether machines in \
                      entities below this one are included. Granting the first does not \
                      grant the second. The Status page shows both."
                    .to_string(),
                links: vec![
                    InfoLink::new("Entities", "front/entity.php"),
                    InfoLink::new("Users", "front/user.php"),
                ],
            }
        }
    }
}
