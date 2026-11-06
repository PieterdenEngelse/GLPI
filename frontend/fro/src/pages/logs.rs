//! GLPI's own log files - one tab per file that actually exists.
//!
//! The tab strip is built from the directory, not from a list in this file.
//! That is the whole design note. An earlier version hardcoded eight names
//! read out of the GLPI source; the install had **fifteen** files, and the
//! page was confidently wrong about its own subject. GLPI does not hardcode
//! them either - `LogParser` takes `GLPI_LOG_DIR` and `scandir`s it against
//! `/^(.+)\.log$/` - because `Toolbox::logInFile()` is called with computed
//! channel names in several places and plugins add their own. A set derived
//! from source is a lower bound, never the answer.
//!
//! Listing and reading are separate privileges here, deliberately. Every
//! file's name and size is shown; **two** have their contents served -
//! `access-errors.log`, which `AccessLogHandler` fills with nothing but
//! refusals, and `php-errors.log`, which carries every error GLPI raised.
//! The backend owns that allowlist and sends a `readable` flag per entry, so
//! this file keeps no copy of it.
//!
//! Why not through GLPI at all, checked in the 12.0.1 source:
//!
//!   - No API. `system_logs`, `LogViewer` and `LogParser` have ZERO
//!     occurrences under src/Glpi/Api/.
//!   - `ajax/logviewer.php` returns HTML, not JSON, and calls
//!     Session::checkRight('system_logs', READ) - a right the `inv` service
//!     account does not hold.
//!   - `front/logs.php` additionally calls
//!     LogViewer::checkReAuthenticationOrRedirect(). GLPI demands a fresh
//!     password for log access on purpose.
//!
//! See backend/src/logs.rs and gl-adapter.md section 4.
use crate::api;
use crate::components::Panel;
use dioxus::prelude::*;

/// What a given log holds, for the ones whose writer was identified in the
/// GLPI source. Returns None for anything else - a file this app has nothing
/// true to say about gets its name and size and no invented explanation.
fn describe(file: &str) -> Option<&'static str> {
    Some(match file {
        "access-errors.log" => {
            "Refusals, and nothing else: AccessLogHandler::isHandling returns \
             false for any record that is not an AccessDeniedHttpException. This is what an \
             agent gets when the inventory endpoint is disabled or a right is missing, which \
             makes it the most on-subject log GLPI keeps."
        }
        "php-errors.log" => {
            "GLPI's errors and uncaught exceptions, written by ErrorLogHandler. \
             Where a 500 from either API lands, and in GLPI 12 where SQL failures surface too - \
             there is no separate sql-errors.log any more."
        }
        "api.log" => {
            "Not what the name suggests. Written from exactly one place in the LEGACY \
             v1 API (logEndpointUsage), only when the calling API client has dolog_method = \
             DOLOG_LOGS - not the default - and it records SUCCESSFUL endpoint usage rather \
             than refusals."
        }
        "event.log" => {
            "GLPI's event stream: logins, item changes, the rest of what the Events \
             screen lists. Note the overlap with the glpi_events TABLE, a different store read \
             by a different screen."
        }
        "cron.log" => {
            "Automatic actions - what glpi-cron.timer drives every minute. Where a \
             scheduled task that silently stopped running leaves its last word."
        }
        "mail.log" => {
            "Outbound mail. Failures go to mail-error.log, which is why a quiet \
             mail.log is not on its own evidence that notifications are being delivered."
        }
        "mail-error.log" => "Outbound mail that failed. The companion to mail.log.",
        "mailgate.log" => {
            "The mail collector - mail coming IN to open or follow up tickets, as \
             opposed to notifications going out."
        }
        "notification.log" => {
            "What GLPI decided to notify about, which is upstream of whether \
             the mail was sent. A notification never raised and one that failed to send look \
             the same from the mail side."
        }
        "webhook.log" => "Outgoing webhook deliveries.",
        "bench_inventory.log" => {
            "Inventory benchmarking. Written through a computed channel \
             name, which is one of the reasons a source-derived list of logs is only ever a \
             lower bound."
        }
        "project.log" | "projecttask.log" => {
            "Project and project-task activity. A feature not \
             in use on this install."
        }
        _ => return None,
    })
}

#[component]
pub fn Logs() -> Element {
    let listing = use_resource(|| async move { api::fetch_logs().await });
    let mut tab = use_signal(|| 0usize);

    match &*listing.read_unchecked() {
        None => rsx! {
            div { class: "text-sm text-base-content/60", "Reading\u{2026}" }
        },
        Some(Err(e)) => rsx! {
            div { class: "rounded-box border border-error/40 bg-error/10 p-4 text-sm",
                "Could not reach this dashboard's own backend: {e}"
            }
        },
        Some(Ok(list)) => {
            if !list.configured {
                return rsx! { NotConfigured {} };
            }
            if let Some(err) = &list.error {
                return rsx! { CannotList { why: err.clone() } };
            }
            if list.entries.is_empty() {
                return rsx! {
                    div { class: "rounded-box border border-base-300 bg-base-200 p-4 text-sm",
                        "GLPI's log directory is readable and holds no .log files at all. On a \
                         working install that is unusual rather than impossible - GLPI creates \
                         each log on its first write."
                    }
                };
            }

            // Clamp rather than trust: the list is refetched, and a file that
            // disappeared between renders must not index past the end.
            let i = tab().min(list.entries.len() - 1);
            let e = &list.entries[i];

            rsx! {
                // flex-wrap, not a scrolling strip. Fifteen tabs do not fit a
                // laptop width, and this machine has no mouse - a horizontal
                // drag to reach a tab is the expensive gesture here.
                div { role: "tablist", class: "tabs tabs-border mb-5 flex-wrap",
                    for (n , entry) in list.entries.iter().enumerate() {
                        button {
                            key: "{entry.file}",
                            role: "tab",
                            class: if i == n { "tab text-base tab-active" } else { "tab text-base" },
                            onclick: move |_| tab.set(n),
                            "{entry.file.trim_end_matches(\".log\")}"
                            // Marks the two whose contents this app serves, so
                            // the other thirteen are not mistaken for tabs
                            // that failed to load.
                            if entry.readable {
                                span { class: "ml-1 text-primary", "\u{2022}" }
                            }
                        }
                    }
                }

                Panel {
                    title: e.file.clone(),
                    subtitle: format!("/var/lib/glpi/files/_log/ \u{00b7} {} bytes", e.bytes),

                    if let Some(d) = describe(&e.file) {
                        p { class: "mb-4 text-sm", "{d}" }
                    } else {
                        p { class: "mb-4 text-sm text-base-content/60",
                            "Nothing in the GLPI 12.0.1 source identified what writes this file, \
                             so this page has nothing true to say about it beyond its name and \
                             size."
                        }
                    }

                    if e.readable {
                        LogTail { file: e.file.clone() }
                    } else {
                        NotRead { file: e.file.clone() }
                    }
                }
            }
        }
    }
}

/// Fetches and renders one log's tail.
#[component]
fn LogTail(file: String) -> Element {
    let f = file.clone();
    let tail = use_resource(move || {
        let f = f.clone();
        async move { api::fetch_log(&f).await }
    });

    match &*tail.read_unchecked() {
        None => rsx! {
            div { class: "text-sm text-base-content/60", "Reading\u{2026}" }
        },
        Some(Err(e)) => rsx! {
            div { class: "rounded-box border border-error/40 bg-error/10 p-4 text-sm",
                "Could not reach this dashboard's own backend: {e}"
            }
        },
        Some(Ok(r)) => {
            let Some(t) = r.tail.clone() else {
                return rsx! {
                    div { class: "text-sm text-base-content/60", "The backend returned no log." }
                };
            };
            match &t.state {
                api::LogState::Ok => rsx! {
                    div { class: "mb-2 flex flex-wrap items-center gap-2 text-xs text-base-content/60",
                        span { "{t.lines.len()} lines shown" }
                        span { "\u{00b7}" }
                        span { "{t.bytes} bytes on disk" }
                        if t.truncated {
                            span { "\u{00b7}" }
                            span { class: "text-warning", "tail only - earlier lines not read" }
                        }
                    }
                    pre {
                        class: "max-h-[32rem] overflow-auto rounded-box border border-base-300 bg-base-200 p-3 text-xs leading-relaxed",
                        code { "{t.lines.join(\"\n\")}" }
                    }
                },
                // Three ways to have no lines, three different fixes.
                // Collapsing them into one empty pane is the mistake the
                // backend's state enum exists to prevent.
                api::LogState::Empty => rsx! {
                    div { class: "rounded-box border border-base-300 bg-base-200 p-4 text-sm",
                        "The file exists and is empty. Nothing has been written to it - which \
                         for an error log is the good outcome."
                    }
                },
                api::LogState::Absent => rsx! {
                    div { class: "rounded-box border border-base-300 bg-base-200 p-4 text-sm",
                        "Gone since the directory was listed. GLPI creates each log on first \
                         write, so this is a file that was there a moment ago and is not now."
                    }
                },
                api::LogState::Unreadable(why) => rsx! {
                    div { class: "rounded-box border border-warning/40 bg-warning/10 p-4",
                        p { class: "text-sm font-medium", "Listed, but the contents cannot be read." }
                        p { class: "mt-2 text-sm text-base-content/80", "{why}" }
                    }
                },
            }
        }
    }
}

#[component]
fn NotConfigured() -> Element {
    rsx! {
        div { class: "rounded-box border border-base-300 bg-base-200 p-4",
            p { class: "text-sm font-medium", "No log directory configured." }
            p { class: "mt-2 text-sm text-base-content/70",
                "Set "
                code { class: "text-xs", "GLPI_INV_LOG_DIR=/var/lib/glpi/files/_log" }
                " in this service's environment and restart it."
            }
        }
    }
}

/// The directory is configured and cannot be listed - which is the state this
/// page is in until the ACL is granted, so it carries the command.
#[component]
fn CannotList(why: String) -> Element {
    rsx! {
        div { class: "rounded-box border border-warning/40 bg-warning/10 p-4",
            p { class: "text-sm font-medium", "GLPI's log directory cannot be listed." }
            p { class: "mt-2 text-sm text-base-content/80", "{why}" }
            p { class: "mt-2 text-sm text-base-content/70",
                "Note what this is NOT: an install with no logs. Nothing can be concluded here \
                 about what GLPI has or has not logged. "
                code { class: "text-xs", "/var/lib/glpi/files" }
                " is "
                code { class: "text-xs", "0750 glpi:glpi" }
                " and this backend runs as an ordinary user, so it needs read access granted:"
            }
            pre {
                class: "mt-3 overflow-x-auto rounded-box border border-base-300 bg-base-100 p-3 text-xs",
                code {
                    "sudo install -d -o glpi -g glpi -m 0750 /var/lib/glpi/files/_log\n"
                    "sudo setfacl -m u:$USER:x /var/lib/glpi /var/lib/glpi/files\n"
                    "sudo setfacl -m u:$USER:rX -m d:u:$USER:r /var/lib/glpi/files/_log"
                }
            }
            p { class: "mt-2 text-sm text-base-content/60",
                "Or declaratively: "
                code { class: "text-xs", "services.glpi.logReaders = [ \"pde\" ];" }
                " in glpi.nix. Traversal only on the parents - adding the user to the glpi "
                "GROUP would also expose config_db.php, which holds the database credentials "
                "and the encryption key."
            }
        }
    }
}

/// One of the files this app lists but does not read.
#[component]
fn NotRead(file: String) -> Element {
    let viewer = crate::glpi_link::link("front/logviewer.php");

    rsx! {
        div { class: "rounded-box border border-base-300 bg-base-200 p-4",
            p { class: "text-sm font-medium", "Listed here, read in GLPI." }
            p { class: "mt-2 text-sm text-base-content/70",
                "This app serves the contents of "
                code { class: "text-xs", "access-errors.log" }
                " and "
                code { class: "text-xs", "php-errors.log" }
                " only - the one log that is purely refusals, and the one that carries every "
                "error GLPI raised. "
                code { class: "text-xs", "{file}" }
                " is about something else, and GLPI's own viewer shows it behind the fresh "
                "password confirmation GLPI asks for on purpose."
            }
            if let Some(href) = viewer {
                a {
                    class: "btn btn-sm btn-outline mt-3",
                    href: "{href}",
                    target: "_blank",
                    rel: "noreferrer",
                    "Open GLPI's log viewer"
                }
            }
        }
    }
}
