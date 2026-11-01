//! Where GLPI lives, shared with every component that wants to link into it.
//!
//! Fetched once and held in context rather than passed down: an info panel
//! three levels deep should not force every component above it to carry a
//! URL it does not use.
//!
//! Empty until the first health response arrives, and components render no
//! link rather than a broken one - a link to `/front/profile.php` with no
//! host would navigate the user out of the app and nowhere useful.
use crate::api;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct GlpiUrl(pub Signal<String>);

/// Call once, at the top of the app.
pub fn provide() {
    let mut url = use_signal(String::new);
    use_future(move || async move {
        if let Ok(h) = api::fetch_health().await {
            url.set(h.glpi_url);
        }
    });
    use_context_provider(|| GlpiUrl(url));
}

/// An absolute URL into GLPI, or None while the address is still unknown.
pub fn link(path: &str) -> Option<String> {
    let base = try_consume_context::<GlpiUrl>()?.0;
    let base = base();
    (!base.is_empty()).then(|| {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    })
}
