//! One editable nginx setting: a box, and a save that reports its own
//! refusal.
//!
//! A component rather than a private helper on the perf page, because two
//! pages now write the same tuning file - the settings table on /perf/nginx
//! and the resolver board on /netw/proxy. The rules a field has to get right
//! are the same in both places and are not obvious: empty CLEARS the setting
//! rather than writing an empty value, the button only appears when the
//! draft differs from what the file already asks for, and a refusal carries
//! the backend's own text because that text is where the useful part is -
//! which flag owns a directive, or what shape the value should have taken. A
//! second copy of all that is a second thing to keep true.
use crate::api;
use dioxus::prelude::*;

/// One editable setting: a box, and a save that reports its own refusal.
#[component]
pub fn TuningField(
    name: String,
    wanted: String,
    accepts: String,
    enabled: bool,
    /// Room for the value. The settings table on /perf/nginx is sixteen rows
    /// of short numbers and wants the narrow box; a resolver is several
    /// addresses and two flags, and in the narrow box a reader cannot see
    /// what they are about to save.
    #[props(default = false)]
    wide: bool,
    notice: Signal<Option<String>>,
    report: Resource<Result<api::NginxTuningReport, api::ApiError>>,
) -> Element {
    let mut draft = use_signal(|| wanted.clone());
    let busy = use_signal(|| false);
    // Compared against what the file says rather than against the last
    // keystroke, so the control only offers to act when there is something
    // to do.
    let changed = draft() != wanted;

    rsx! {
        div { class: "flex items-center gap-1",
            input {
                class: if wide {
                    "input input-sm input-bordered w-full max-w-md font-mono"
                } else {
                    "input input-xs input-bordered w-36 font-mono"
                },
                r#type: "text",
                value: "{draft}",
                disabled: !enabled || busy(),
                placeholder: "{accepts}",
                title: "{accepts}",
                oninput: move |e| draft.set(e.value()),
            }
            if changed && enabled {
                button {
                    class: if wide { "btn btn-sm btn-primary" } else { "btn btn-xs btn-primary" },
                    disabled: busy(),
                    onclick: move |_| {
                        let name = name.clone();
                        let v = draft();
                        let mut busy = busy;
                        let mut notice = notice;
                        let mut report = report;
                        busy.set(true);
                        notice.set(None);
                        spawn(async move {
                            // Empty clears the setting, which is how a value
                            // is removed rather than set to nothing.
                            let v = if v.trim().is_empty() { None } else { Some(v) };
                            match api::set_nginx_tuning(&name, v.as_deref()).await {
                                Ok(()) => report.restart(),
                                Err(e) => notice.set(Some(format!("{name}: {e}"))),
                            }
                            busy.set(false);
                        });
                    },
                    if busy() { "..." } else { "save" }
                }
            }
        }
    }
}
