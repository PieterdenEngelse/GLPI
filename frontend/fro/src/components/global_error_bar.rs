use crate::api;
use dioxus::prelude::*;

/// A strip across the top when the backend cannot be reached at all - or
/// when it answers and this bundle cannot read the reply.
///
/// Separate from any page's own error state on purpose: a page shows what
/// *it* failed to load, this says the whole app is talking to nothing. It
/// occupies no space while things are fine, so it costs nothing to leave on
/// every page.
///
/// The two cases cannot share a bar. "Backend unreachable" across every page
/// is false when the backend just answered 200, and it hides the only thing
/// that would fix it: the browser is running an older bundle than the binary
/// serving it, so reload.
#[component]
pub fn GlobalErrorBar() -> Element {
    let mut message = use_signal(|| None::<api::ApiError>);

    use_future(move || async move {
        loop {
            message.set(api::fetch_health().await.err());
            gloo_timers::future::TimeoutFuture::new(30_000).await;
        }
    });

    // A skew anywhere outranks this bar's own health check: the check can
    // succeed while another endpoint is the one this bundle cannot read.
    let skew = api::skew_seen() || matches!(message(), Some(ref m) if m.is_skew());
    let refused = match message() {
        Some(ref m) if m.is_upstream() => Some(m.to_string()),
        _ => None,
    };
    let unreachable = match message() {
        Some(m) if !m.is_skew() && !m.is_upstream() => Some(m.to_string()),
        _ => None,
    };

    rsx! {
        if skew {
            div { class: "bg-warning px-4 py-1.5 text-center text-sm text-warning-content",
                "This page and the backend disagree — it answered and this bundle could "
                "not read the reply. Reload; if that does not clear it, the running "
                "backend is older than this page and needs rebuilding."
            }
        } else if let Some(m) = refused {
            div { class: "bg-error px-4 py-1.5 text-center text-sm text-error-content",
                "The backend refused the health check — {m}"
            }
        } else if let Some(m) = unreachable {
            div { class: "bg-error px-4 py-1.5 text-center text-sm text-error-content",
                "Backend unreachable — {m}"
            }
        }
    }
}
