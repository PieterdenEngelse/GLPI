//! When a GLPI session ends, and the settings that decide it.
//!
//! WHY THIS IS A TILE AND NOT A PAGE. Every other settings surface in this
//! app is a page because the subject is large - 484 variables, sixteen nginx
//! directives. This is five rows, three of which are read-only, and the
//! question it answers ("why am I logged out after twenty minutes") is the
//! kind someone arrives at the front door with.
//!
//! WHY THE ROWS LOOK DIFFERENT FROM EACH OTHER. They are owned by three
//! different things, and the row says which:
//!
//!   * `pool` - php.ini through this pool's `phpOptions`, which is the one
//!     file this app writes. A field and a save.
//!   * `package` - php.ini through `php.buildEnv { extraConfig }` in
//!     glpi.nix. PHP loads that file AFTER the pool's and it wins, so a
//!     control here would write a file, rebuild clean, and change nothing.
//!     Read-only with the file named.
//!   * `glpi` - a row in GLPI's own config. This app reads GLPI and does not
//!     write to it, so the row carries a link into GLPI's screen instead of
//!     a field.
//!
//! WHY THE NUMBERS ARE LABELLED "default" RATHER THAN SHOWN BARE. The
//! backend does not run `ini_get` - there is no PHP on this host's PATH and
//! the effective ini is assembled from a store path plus a scan directory -
//! so an unset value is PHP's documented default and is said to be. A
//! dashboard that printed 1440 as though it had measured it would be lying
//! in the one place a reader would most trust it.
use crate::api;
use crate::components::{InfoLink, Panel};
use dioxus::prelude::*;

#[component]
pub fn LogoutTile() -> Element {
    let report = use_resource(|| async move { api::fetch_logout().await });
    let notice = use_signal(|| None::<String>);

    rsx! {
        Panel {
            title: "Logout".to_string(),
            subtitle: "When a GLPI session ends, and where each setting lives".to_string(),
            info: "GLPI has no session-duration setting. What logs people out is PHP's \
                   session.gc_maxlifetime, and GLPI's own session cron task then reads the \
                   same value to delete session files and revoke tracked sessions. The only \
                   timeout-shaped figure GLPI does own is login_remember_time, which is the \
                   \"Remember Me\" cookie and not a timeout at all. That is why these five \
                   settings sit in three places. Two of them can be changed here: this app \
                   writes one Nix file and a nixos-rebuild switch applies it, because php.ini \
                   is a read-only store path and PHP has no runtime settings interface - there \
                   is no apply-it-now at any price. Two more are set by glpi.nix's own php \
                   package, which PHP loads after this file and which therefore wins, so they \
                   are read-only here rather than offering a control that would quietly do \
                   nothing. The last belongs to GLPI and is changed in GLPI."
                .to_string(),
            info_links: vec![InfoLink::new("GLPI general setup", "front/config.form.php")],

            match &*report.read_unchecked() {
                Some(Ok(r)) => rsx! { Rows { report: r.clone(), notice, res: report } },
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Could not read the session settings." }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },
                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Loading…"
                    }
                },
            }
        }
    }
}

#[component]
fn Rows(
    report: api::LogoutReport,
    notice: Signal<Option<String>>,
    res: Resource<Result<api::LogoutReport, api::ApiError>>,
) -> Element {
    rsx! {
        // The state of the file, before the rows: whether a change can be
        // kept at all is the first thing that decides what the rows mean.
        if !report.configured {
            div { class: "mb-3 alert alert-warning text-sm",
                div {
                    div { class: "font-semibold", "Nothing can be changed yet." }
                    p { class: "mt-1",
                        "GLPI_INV_PHP_TUNING names no file, so there is nowhere to keep a "
                        "value. Point it at a path, import that path from the system config, "
                        "and git add it - /etc/nixos is a git repo and a flake cannot see an "
                        "untracked file."
                    }
                }
            }
        } else if !report.writable {
            div { class: "mb-3 alert alert-info text-sm",
                div {
                    div { class: "font-semibold", "Read-only." }
                    p { class: "mt-1",
                        "Start the backend with GLPI_INV_PHP_WRITES=1 to change these. The "
                        "endpoint checks the flag too, not just this control."
                    }
                }
            }
        }

        if let Some(e) = notice() {
            div { class: "mb-3 alert alert-error text-sm",
                pre { class: "break-all whitespace-pre-wrap", "{e}" }
            }
        }

        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "Setting" }
                        th { "Value" }
                        th { "Owned by" }
                    }
                }
                tbody {
                    for s in report.settings.iter().cloned() {
                        tr {
                            key: "{s.name}",
                            td {
                                div { class: "font-mono text-xs", "{s.name}" }
                                div { class: "text-xs text-base-content/60", "{s.what}" }
                            }
                            td {
                                if s.editable {
                                    LogoutField {
                                        name: s.name.clone(),
                                        wanted: s.wanted.clone().unwrap_or_default(),
                                        default: s.default.clone(),
                                        min: s.min,
                                        max: s.max,
                                        enabled: report.writable,
                                        notice,
                                        res,
                                    }
                                } else {
                                    Reading { setting: s.clone(), remember_error: report.remember_error.clone() }
                                }
                            }
                            td { class: "align-top text-xs",
                                div { class: "font-mono break-all", "{s.via}" }
                                if let Some(p) = s.glpi_path.clone() {
                                    GlpiLink { path: p }
                                }
                            }
                        }
                    }
                }
            }
        }

        if report.configured {
            p { class: "mt-3 text-xs text-base-content/60",
                "Written to "
                span { class: "font-mono", "{report.path.clone().unwrap_or_default()}" }
                " as services.phpfpm.pools."
                span { class: "font-mono", "{report.pool}" }
                ".phpOptions. Nothing here applies on its own: "
                span { class: "font-mono", "sudo nixos-rebuild switch" }
                " is what makes it real, and it restarts the pool - which ends every session "
                "that is open at the time."
            }
        }
    }
}

/// A value this app will not write, said plainly.
#[component]
fn Reading(setting: api::LogoutSetting, remember_error: Option<String>) -> Element {
    rsx! {
        if setting.owner == "glpi" {
            match setting.live {
                Some(n) => rsx! {
                    div { class: "font-mono text-sm", "{n}" }
                    div { class: "text-xs text-base-content/60", "{human(n)}, read from GLPI" }
                },
                None => rsx! {
                    div { class: "text-sm text-base-content/60", "unknown" }
                    div { class: "text-xs text-base-content/60",
                        "GLPI's default is {setting.default}. "
                        if let Some(e) = remember_error.clone() { "{e}" }
                    }
                },
            }
        } else {
            div { class: "font-mono text-sm", "{setting.default}" }
            div { class: "text-xs text-base-content/60", "set by this deployment" }
        }
    }
}

/// A link into GLPI's own screen, through the base URL the backend reports.
///
/// Nothing is rendered while that address is still unknown: a link with no
/// host would navigate the reader out of the app and nowhere useful, which
/// is the rule `glpi_link` already states.
#[component]
fn GlpiLink(path: String) -> Element {
    rsx! {
        if let Some(href) = crate::glpi_link::link(&path) {
            a {
                class: "link link-primary",
                href: "{href}",
                target: "_blank",
                rel: "noopener",
                "change it in GLPI"
            }
        }
    }
}

/// One editable session setting: a box, and a save that reports its refusal.
///
/// NOT `TuningField`, and the difference is not cosmetic. That component is
/// bound to `set_nginx_tuning` and to `Resource<NginxTuningReport>` - the
/// endpoint and the resource type are both in its body - and it describes a
/// value by its SHAPE ("a number with an optional k, m or g") because an
/// nginx directive has one. Here the endpoint is different, the resource is
/// different, and a value is described by its RANGE, which is per setting and
/// arrives from the backend rather than being a kind. Clearing also means
/// something else: an nginx directive disappears from the config, while
/// clearing here falls back to PHP's compiled default, and the placeholder
/// has to say which number that is or an empty box reads as "no timeout".
#[component]
fn LogoutField(
    name: String,
    wanted: String,
    default: String,
    min: u64,
    max: u64,
    enabled: bool,
    notice: Signal<Option<String>>,
    res: Resource<Result<api::LogoutReport, api::ApiError>>,
) -> Element {
    let mut draft = use_signal(|| wanted.clone());
    let busy = use_signal(|| false);
    // Against the file, not the last keystroke: the control only offers to
    // act when there is something to do.
    let changed = draft() != wanted;
    let help = format!("{min} to {max} seconds; empty falls back to PHP's default of {default}");

    rsx! {
        div { class: "flex items-center gap-1",
            input {
                class: "input input-xs input-bordered w-28 font-mono",
                r#type: "text",
                value: "{draft}",
                disabled: !enabled || busy(),
                placeholder: "{default}",
                title: "{help}",
                oninput: move |e| draft.set(e.value()),
            }
            if changed && enabled {
                button {
                    class: "btn btn-xs btn-primary",
                    disabled: busy(),
                    onclick: move |_| {
                        let name = name.clone();
                        let v = draft();
                        let mut busy = busy;
                        let mut notice = notice;
                        let mut res = res;
                        busy.set(true);
                        notice.set(None);
                        spawn(async move {
                            // Empty clears the setting, which is how a value
                            // is removed rather than written as nothing.
                            let v = if v.trim().is_empty() { None } else { Some(v) };
                            match api::set_logout_setting(&name, v.as_deref()).await {
                                Ok(()) => res.restart(),
                                Err(e) => notice.set(Some(format!("{name}: {e}"))),
                            }
                            busy.set(false);
                        });
                    },
                    if busy() { "..." } else { "save" }
                }
            }
        }
        div { class: "mt-0.5 text-xs text-base-content/60",
            match wanted.parse::<i64>() {
                Ok(n) => rsx! { "{human(n)}, pending a rebuild" },
                Err(_) => rsx! { "unset, so PHP's default of {default} applies" },
            }
        }
    }
}

/// Seconds as something a reader can judge at a glance.
///
/// The figures here span 1440 to 5184000, and "604800" is not a duration
/// anyone reads as a week. Deliberately coarse - one unit, no remainder:
/// this sits under a field that already shows the exact number, so a second
/// precise rendering of it would be noise.
fn human(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 3600;
    const DAY: i64 = 86_400;
    match seconds {
        0 => "until the browser closes".to_string(),
        s if s < MINUTE => format!("{s} seconds"),
        s if s < HOUR => plural(s / MINUTE, "minute"),
        s if s < DAY => plural(s / HOUR, "hour"),
        s => plural(s / DAY, "day"),
    }
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}
