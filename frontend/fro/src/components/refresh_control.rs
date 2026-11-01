use dioxus::prelude::*;

/// Shows the poll interval and offers a manual refresh.
///
/// RERAG has a component of this name whose button only calls
/// console::log_1 - scaffolding that was never wired up. This one takes the
/// handler as a prop, because a refresh button that does not refresh is worse
/// than no button: it teaches the reader the page is current when it is not.
#[component]
pub fn RefreshControl(
    #[props(default = "5m".to_string())] interval_label: String,
    on_refresh: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "mb-4 flex items-center gap-3",
            span { class: "text-xs text-base-content/60", "⟳ every {interval_label}" }
            button {
                class: "btn btn-ghost btn-sm text-primary",
                onclick: move |_| on_refresh.call(()),
                "Refresh now"
            }
        }
    }
}
