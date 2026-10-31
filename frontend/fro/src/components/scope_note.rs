use crate::api::{self, Entity};
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

    let (class, text) = match &*entities.read_unchecked() {
        None => ("scope", "Scope: checking…".to_string()),

        // Never hide the line on failure. A page with no scope note reads as
        // "no limits apply", which is the opposite of what is known here.
        Some(Err(e)) => (
            "scope warn",
            format!("Scope: could not be read ({e}). The list below may be filtered."),
        ),

        // Readable but empty is a real state, and a bad one: an account with
        // no entities can read nothing, so an empty table below means nothing.
        Some(Ok(list)) if list.is_empty() => (
            "scope warn",
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
                "scope",
                format!(
                    "Scope: {} {noun} — {names}. Agents outside these are not listed.",
                    list.len()
                ),
            )
        }
    };

    rsx! { p { class: "{class}", "{text}" } }
}
