use crate::api;
use dioxus::prelude::*;

/// A strip across the top when the backend cannot be reached at all.
///
/// Separate from any page's own error state on purpose: a page shows what
/// *it* failed to load, this says the whole app is talking to nothing. It
/// occupies no space while things are fine, so it costs nothing to leave on
/// every page.
#[component]
pub fn GlobalErrorBar() -> Element {
    let mut message = use_signal(|| None::<String>);

    use_future(move || async move {
        loop {
            message.set(api::fetch_health().await.err());
            gloo_timers::future::TimeoutFuture::new(30_000).await;
        }
    });

    rsx! {
        if let Some(m) = message() {
            div { class: "bg-error px-4 py-1.5 text-center text-sm text-error-content",
                "Backend unreachable — {m}"
            }
        }
    }
}
