use crate::api;
use crate::components::InfoButton;
use dioxus::prelude::*;

/// The backend's own health, as a dot beside the title - and, on a click,
/// which of the four things this app depends on is actually answering.
///
/// WHAT THIS TOOK FROM RERAG, AND WHAT IT DID NOT. RERAG's header light is
/// the model: a coloured dot that aggregates several independent upstreams,
/// and a panel behind it naming each one with live figures rather than a
/// word. Its seven states are not copied, though - busy, unknown and an
/// orange fallback exist because RERAG has an indexing queue that can be
/// mid-work, and this app has nothing of the kind. A "busy" colour here
/// would be a colour with nothing behind it.
///
/// THE FOUR CHECKS, and why they are not equally serious:
///
/// - **The backend.** `/api/health`, every 30 seconds. If this fails nothing
///   else can be known, so the dot is red and the panel says the rest is
///   unknown rather than showing four stale lines.
/// - **GLPI's API.** `/api/session`. Every page but Refused reads through it,
///   so a failure here is why an agent list is empty - which is the one
///   confusion this whole app exists to prevent. Degraded, not fatal: the
///   reference pages still work.
/// - **The database.** `/api/mariadb`. Optional by design, so *not
///   configured* is not a fault and is not amber - the app says so and moves
///   on. Configured and failing IS amber.
/// - **GLPI's log directory.** `/api/logs`. Same distinction: absent is a
///   choice, and a directory that will not list is a finding - it is 0750
///   glpi:glpi, so the backend can be configured correctly and still be
///   refused by the kernel. What is NOT a finding is a file marked
///   unreadable: that flag is the backend's two-name allowlist
///   (`logs::ALLOWED`), so 12 of this install's 14 files are unservable on
///   purpose and amber-ing on them would cry wolf for ever.
///
/// CLICK, NOT HOVER. RERAG opens its panel on hover with a refcount to keep
/// it alive while the pointer travels. This machine is driven by a touchpad,
/// where hover is something you hold rather than something that happens, so
/// the dot is a button and the panel stays until dismissed. The target is
/// padded well past the dot for the same reason.
///
/// AND AN INFO BUTTON BESIDE IT, as RERAG has: its status light carries a
/// separate circle-and-i that opens a legend of what each colour means. A
/// colour is the one thing on a page that cannot explain itself - there is
/// nowhere on a 12px dot to write "amber means configured and not
/// answering" - which is also why this repo's rule is that a figure a reader
/// could misread gets a modal rather than a tooltip. The panel says what is
/// wrong; the modal says what the dot is.
#[component]
pub fn HealthDot() -> Element {
    let mut health = use_signal(|| None::<Result<api::Health, api::ApiError>>);
    let mut session = use_signal(|| None::<Result<api::SessionInfo, api::ApiError>>);
    let mut db = use_signal(|| None::<Result<api::MariadbReport, api::ApiError>>);
    let mut logs = use_signal(|| None::<Result<api::LogListResponse, api::ApiError>>);
    let mut open = use_signal(|| false);
    let mut checking = use_signal(|| false);

    // The deep checks, which cost an upstream call each. Run at mount, when
    // the panel is opened, and on demand - not on the 30-second loop, which
    // would mean a GLPI API call every half minute for a dot most people are
    // not looking at.
    let deep = move || async move {
        checking.set(true);
        session.set(Some(api::fetch_session().await));
        db.set(Some(api::fetch_mariadb().await));
        logs.set(Some(api::fetch_logs().await));
        checking.set(false);
    };

    use_future(move || async move {
        loop {
            health.set(Some(api::fetch_health().await));
            gloo_timers::future::TimeoutFuture::new(30_000).await;
        }
    });
    use_future(move || async move { deep().await });

    // Worst wins, and the order is the order of consequence: nothing to talk
    // to, then the API every page reads, then an optional source that was
    // configured and did not answer.
    // A reply that arrived and could not be read is THIS APP's fault, so it
    // is kept out of every "broken" below. Counting it as a failing upstream
    // is what made a stale bundle present as a degraded database until
    // 2026-11-18, which points whoever sees it at the wrong system.
    // api::SKEW covers every OTHER fetch in the app. Without it the dot sat
    // green while the page under it said the bundle and the backend
    // disagree, because none of the four below is what failed.
    let skew = api::skew_seen()
        || matches!(health(), Some(Err(ref e)) if e.is_skew())
        || matches!(session(), Some(Err(ref e)) if e.is_skew())
        || matches!(db(), Some(Err(ref e)) if e.is_skew())
        || matches!(logs(), Some(Err(ref e)) if e.is_skew());

    // A decode failure still proves something answered, so the backend counts
    // as up and the other three are asked rather than reported unknown.
    let backend_up =
        matches!(health(), Some(Ok(_))) || matches!(health(), Some(Err(ref e)) if e.is_skew());
    let glpi_up = matches!(session(), Some(Ok(_)));
    let db_broken = matches!(db(), Some(Err(ref e)) if !e.is_skew())
        || matches!(db(), Some(Ok(ref r)) if r.configured && r.facts.is_none());
    let logs_broken = matches!(logs(), Some(Err(ref e)) if !e.is_skew())
        || matches!(logs(), Some(Ok(ref r)) if r.configured && r.error.is_some());

    let (dot, ring, label) = match (health(), backend_up, glpi_up) {
        // Before Checking: a skew is something already known, while Checking
        // is the absence of an answer. Hiding a fact behind a pending request
        // means the dot goes green-ish grey on the one page that just failed.
        _ if skew => (
            "bg-warning",
            "ring-warning/40",
            "Bundle and backend disagree",
        ),
        (None, _, _) => (
            "bg-base-content/30 animate-pulse",
            "ring-base-300",
            "Checking",
        ),
        (Some(Err(ref e)), _, _) if !e.is_skew() => {
            ("bg-error", "ring-error/40", "Backend unreachable")
        }
        (_, true, false) if session().is_some() => {
            ("bg-warning", "ring-warning/40", "GLPI's API not answering")
        }
        _ if db_broken || logs_broken => ("bg-warning", "ring-warning/40", "Degraded"),
        _ => ("bg-success", "ring-success/40", "All four answering"),
    };

    rsx! {
        div { class: "relative flex shrink-0 items-center gap-1",
            button {
                // Sized to the header rather than to a dot's convention.
                // The bar is py-1 around a 2rem row - btn-sm and the logo
                // badge both set that - so 1.5rem of dot plus a 2px ring
                // plus p-0.5 comes to exactly 2rem: the largest circle that
                // fits without making the header taller. It is also the
                // easiest target in the bar, which on a touchpad is the
                // point.
                class: "flex items-center rounded-full p-0.5 hover:bg-base-200",
                aria_label: "{label} - open status detail",
                title: "{label}",
                onclick: move |_| {
                    let was = open();
                    open.set(!was);
                    if !was {
                        spawn(deep());
                    }
                },
                span { class: "inline-block h-6 w-6 rounded-full ring-2 {dot} {ring}" }
            }

            InfoButton {
                title: "What the status light means".to_string(),
                // One body, no sections: the house rule, and at this length a
                // tab strip would hide half a paragraph behind a click.
                body: "Four things have to answer for this app to tell you anything, and the \
                       dot is the worst of the four. Green: all four answered. Amber: the \
                       backend is up but something it reads is not - GLPI's API refused, or \
                       a source that was configured did not reply. Amber also covers one \
                       fault that is no upstream's: a reply that arrives and cannot be read \
                       means the bundle in your browser and the backend serving it are \
                       built from different code, so that line asks for a reload instead of \
                       naming the source as broken - every upstream is healthy, and reading a \
                       version skew as a broken database is the exact mistake this panel \
                       exists to prevent. Which half is older is not knowable from here: if a \
                       reload does not clear it, the deployed backend is the stale one. Red: the backend itself is unreachable, so the other three \
                       were never asked and the panel \
                       says so rather than showing three stale lines. Grey and pulsing: the \
                       first check has not come back yet, which is deliberately not red - a \
                       slow start is not a failure. Click the dot for the four lines, each \
                       with what that source actually said. The distinction those lines are \
                       for: NOT CONFIGURED IS NOT A FAULT. The database and GLPI's log \
                       directory are both optional - without them only the Refused and Logs \
                       pages go dark - so absent is grey with the reason, while configured \
                       and silent is amber. One flag that looks like a fault and is not: a \
                       log file marked unreadable is simply not one of the two the backend \
                       will ever serve, so '14 files, 2 served' is this install working as \
                       designed. What it costs: the backend is checked every 30 seconds \
                       because that is local and free, the other three only when you open \
                       the panel, so a dot nobody is looking at does not spend a GLPI API \
                       call every half minute."
                    .to_string(),
            }

            if open() {
                div { class: "absolute left-0 top-full z-50 mt-1 w-96 max-w-[90vw] rounded-box border border-base-300 bg-base-100 p-3 shadow-lg",
                    div { class: "mb-2 flex items-center justify-between gap-2",
                        span { class: "text-sm font-semibold", "{label}" }
                        div { class: "flex items-center gap-2",
                            if checking() {
                                span { class: "loading loading-spinner loading-xs" }
                            }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| {
                                    health.set(None);
                                    spawn(async move {
                                        health.set(Some(api::fetch_health().await));
                                    });
                                    spawn(deep());
                                },
                                "Check now"
                            }
                            button {
                                class: "btn btn-ghost btn-xs",
                                onclick: move |_| open.set(false),
                                "Close"
                            }
                        }
                    }

                    ul { class: "space-y-2 text-xs",
                        Line {
                            name: "Backend".to_string(),
                            state: match health() {
                                None => Check::Checking,
                                Some(Ok(h)) => Check::Ok(format!("v{} - answering", h.version)),
                                Some(Err(e)) if e.is_skew() => Check::Unreadable(e.to_string()),
                                Some(Err(e)) => Check::Failed(e.to_string()),
                            },
                        }
                        Line {
                            name: "GLPI's API".to_string(),
                            state: match (backend_up, session()) {
                                (false, _) => Check::Unknown,
                                (_, None) => Check::Checking,
                                (_, Some(Ok(s))) => {
                                    Check::Ok(
                                        format!(
                                            "{} on {}",
                                            s.profile_name.unwrap_or_else(|| "no profile".into()),
                                            s.entity_name.unwrap_or_else(|| "no entity".into()),
                                        ),
                                    )
                                }
                                (_, Some(Err(e))) if e.is_skew() => {
                                    Check::Unreadable(e.to_string())
                                }
                                (_, Some(Err(e))) => Check::Failed(e.to_string()),
                            },
                        }
                        Line {
                            name: "Database".to_string(),
                            state: match (backend_up, db()) {
                                (false, _) => Check::Unknown,
                                (_, None) => Check::Checking,
                                (_, Some(Ok(r))) if !r.configured => {
                                    Check::Absent("not configured - only Refused needs it".into())
                                }
                                (_, Some(Ok(r))) => {
                                    match r.facts {
                                        Some(f) => Check::Ok(format!("{} - {} tables", f.version, f.glpi_tables)),
                                        None => Check::Failed("configured, but it reported nothing".into()),
                                    }
                                }
                                (_, Some(Err(e))) if e.is_skew() => {
                                    Check::Unreadable(e.to_string())
                                }
                                (_, Some(Err(e))) => Check::Failed(e.to_string()),
                            },
                        }
                        Line {
                            name: "GLPI's logs".to_string(),
                            state: match (backend_up, logs()) {
                                (false, _) => Check::Unknown,
                                (_, None) => Check::Checking,
                                (_, Some(Ok(r))) if !r.configured => {
                                    Check::Absent("no directory configured".into())
                                }
                                (_, Some(Ok(r))) => {
                                    match r.error {
                                        Some(e) => Check::Failed(e),
                                        None => {
                                            let servable = r.entries.iter().filter(|e| e.readable).count();
                                            Check::Ok(
                                                format!(
                                                    "{} files, {servable} served",
                                                    r.entries.len(),
                                                ),
                                            )
                                        }
                                    }
                                }
                                (_, Some(Err(e))) if e.is_skew() => {
                                    Check::Unreadable(e.to_string())
                                }
                                (_, Some(Err(e))) => Check::Failed(e.to_string()),
                            },
                        }
                    }

                    if api::skew_seen() {
                        p { class: "mt-3 border-t border-base-300 pt-2 text-xs text-warning",
                            "Something on this page could not read a reply from the backend, so "
                            "the bundle in your browser and the binary serving it were built "
                            "from different code. The four lines above can all be healthy and "
                            "this still be true - they are four endpoints, and the one that "
                            "failed may be any other. Reload; if that does not clear it, the "
                            "running backend is the older of the two."
                        }
                    }

                    p { class: "mt-3 border-t border-base-300 pt-2 text-xs text-base-content/60",
                        "The backend is checked every 30 seconds; the other three when this "
                        "panel is opened, so looking at a dot does not cost GLPI an API call "
                        "every half minute."
                    }
                }
            }
        }
    }
}

/// One dependency's state. Four cases rather than two, because the whole
/// point of this panel is that "not configured" and "did not answer" are
/// different news, and so is "could not look".
#[derive(Clone, PartialEq)]
enum Check {
    Checking,
    Ok(String),
    /// Configured and failing.
    Failed(String),
    /// It answered, and this bundle could not read the reply. Not a fault in
    /// the thing named on the line - the only honest remedy is a reload.
    Unreadable(String),
    /// Deliberately absent. Not a fault, and not amber.
    Absent(String),
    /// The backend is down, so this was never asked.
    Unknown,
}

#[component]
fn Line(name: String, state: Check) -> Element {
    let (dot, detail, tone) = match &state {
        Check::Checking => (
            "bg-base-content/30 animate-pulse",
            "checking\u{2026}".to_string(),
            "",
        ),
        Check::Ok(d) => ("bg-success", d.clone(), ""),
        Check::Failed(d) => ("bg-error", d.clone(), "text-error"),
        Check::Unreadable(d) => (
            "bg-warning",
            format!("answered, but this page could not read the reply - reload, and if that does not clear it the backend is the older of the two ({d})"),
            "text-warning",
        ),
        Check::Absent(d) => ("bg-base-content/30", d.clone(), "text-base-content/60"),
        Check::Unknown => (
            "bg-base-content/30",
            "not asked - the backend is not answering".to_string(),
            "text-base-content/60",
        ),
    };

    rsx! {
        li { class: "flex items-start gap-2",
            span { class: "mt-1 inline-block h-2 w-2 shrink-0 rounded-full {dot}" }
            div { class: "min-w-0",
                span { class: "font-medium", "{name}. " }
                span { class: "break-all {tone}", "{detail}" }
            }
        }
    }
}
