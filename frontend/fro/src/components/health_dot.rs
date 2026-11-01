use crate::api;
use dioxus::prelude::*;

/// The backend's own health, as a dot beside the title.
///
/// RERAG puts one here and it earns its place: the status of the thing you
/// are looking at should be visible without reading anything. Grey while
/// unknown, so a slow first check never flashes a false red.
#[component]
pub fn HealthDot() -> Element {
    let mut healthy = use_signal(|| None::<bool>);

    use_future(move || async move {
        loop {
            healthy.set(Some(api::fetch_health().await.is_ok()));
            gloo_timers::future::TimeoutFuture::new(30_000).await;
        }
    });

    let (colour, title) = match healthy() {
        None => ("bg-base-content/30", "Checking the backend…"),
        Some(true) => ("bg-success", "Backend reachable"),
        Some(false) => ("bg-error", "Backend unreachable"),
    };

    rsx! {
        span {
            class: "inline-block h-2.5 w-2.5 shrink-0 rounded-full {colour}",
            title: "{title}",
        }
    }
}
