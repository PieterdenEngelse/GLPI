use crate::api::ApiError;
use dioxus::prelude::*;

/// What to show when a fetch failed, which depends entirely on WHY.
///
/// The two cases are opposite news and were one message until 2026-11-18.
/// `Unreachable` is the backend: nothing answered, the page has nothing to
/// show, and red is right. `Unreadable` means it answered and this bundle
/// could not read the reply - every upstream is healthy and the fault is
/// version skew between the browser and the binary serving it, so the only
/// useful instruction is to reload. Rendering that as "Could not reach the
/// backend" sends the reader off to check a database that is working
/// perfectly, which is the most expensive thing a wrong error message can do.
///
/// One component rather than the seven copies it replaced: a skew is the
/// case a reader meets rarely and has to understand immediately, so the
/// wording cannot be allowed to drift between pages.
#[component]
pub fn FetchError(error: ApiError) -> Element {
    let skew = error.is_skew();
    // Three outcomes, three sentences. An upstream refusal is neither a
    // skew nor a dead backend: it answered, on purpose, and said why - so
    // the only useful thing to show is its own words.
    let (cls, head) = if skew {
        ("alert alert-warning", "This page and the backend disagree.")
    } else if error.is_upstream() {
        ("alert alert-error", "The backend refused this request.")
    } else {
        ("alert alert-error", "Could not reach the backend.")
    };

    rsx! {
        div { class: "{cls}",
            div {
                div { class: "font-semibold", "{head}" }
                if skew {
                    p { class: "mt-1 text-sm",
                        "The backend answered and this page could not read the reply, so the \
                         bundle in your browser and the binary serving it were built from \
                         different code. Reload first. If that does not clear it, the running \
                         backend is the older of the two and has to be rebuilt and restarted: \
                         a backend missing a path this page calls answers with the page \
                         itself, which is what will not parse. Nothing upstream is wrong."
                    }
                }
                pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{error}" }
            }
        }
    }
}
