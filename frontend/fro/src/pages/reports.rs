//! Compose a GLPI report from dropdowns.
//!
//! GLPI has no UI for authoring a report: `Report::getReports()` builds that
//! menu from PHP plus a plugin hook, with no table behind it. The `swcat`
//! plugin adds one — definitions are rows, one parameterised renderer serves
//! them all, and the plugin rebuilds its Tools > Reports menu from the table on
//! every request. So a definition saved here is a GLPI report immediately: no
//! plugin file, and on this host no `nixos-rebuild`, because nothing new has to
//! land in /nix/store.
//!
//! What is NOT here, deliberately: a preview of the figures. Previewing means
//! reimplementing the plugin's aggregation in Rust, and two engines that
//! disagree are worse than one engine and a link. The saved report links out to
//! GLPI instead, where the numbers come from the renderer that produces them.
use crate::api::{self, ReportDef, ReportList};
use crate::components::{FetchError, Panel};
use dioxus::prelude::*;

/// Fields offered in the group-by list.
///
/// A short curated list rather than every foreign key GLPI has: these are the
/// ones that carry across most item types and mean something when counted. The
/// field is a free-text input as well, so nothing here is a ceiling - the
/// backend accepts any `*_id` and the plugin reports an unknown one clearly.
const GROUP_FIELDS: &[(&str, &str)] = &[
    ("locations_id", "Location (tree)"),
    ("states_id", "Status (tree)"),
    ("manufacturers_id", "Manufacturer"),
    ("entities_id", "Entity (tree)"),
    ("groups_id", "Group (tree)"),
    ("users_id", "User"),
    ("softwarecategories_id", "Software category (tree)"),
    ("computertypes_id", "Computer type"),
];

/// Item types offered for the "limit to" list.
///
/// Not every CI type GLPI has — the ones with enough of a presence in an
/// inventory to be worth counting separately. Leaving the list empty means
/// every type that carries the chosen field, which is the useful default.
/// Owner types that can be counted through a link table.
///
/// These carry no grouped field of their own — a contract is not "in" a
/// location — so each is counted under the value of the items it links to,
/// in a second table. The list is exactly what the plugin knows how to join;
/// unlike the group-by field there is no free-text escape, because each one
/// needs its link table and foreign key named in the plugin.
const RELATION_TYPES: &[&str] = &["Contract", "Certificate", "SoftwareLicense"];

const ITEM_TYPES: &[&str] = &[
    "Computer",
    "Software",
    "SoftwareLicense",
    "Monitor",
    "Printer",
    "NetworkEquipment",
    "Peripheral",
    "Phone",
    "Rack",
    "Certificate",
    "Contract",
];

#[component]
pub fn Reports() -> Element {
    let mut list = use_resource(|| async move { api::fetch_reports().await });

    rsx! {
        match &*list.read_unchecked() {
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) => rsx! {
                Saved { list: r.clone(), on_change: move |_| list.restart() }
                if r.writes_enabled {
                    Builder { on_saved: move |_| list.restart() }
                } else {
                    div { class: "mt-4 rounded-box border border-base-300 bg-base-200 p-4",
                        p { class: "text-sm font-medium", "Read-only backend." }
                        p { class: "mt-2 text-sm text-base-content/70",
                            "Definitions can be listed but not saved or deleted. Start the "
                            "backend with "
                            code { class: "text-xs", "GLPI_INV_REPORT_WRITES=1" }
                            " to enable the form. It is a separate flag from the other write "
                            "permissions on purpose - it admits rows in the plugin's own "
                            "table and nothing else."
                        }
                    }
                }
            },
            None => rsx! { div { class: "text-base-content/60", "Loading…" } },
        }
    }
}

#[component]
fn Saved(list: ReportList, on_change: EventHandler<()>) -> Element {
    let mut error = use_signal(|| Option::<String>::None);

    rsx! {
        Panel {
            title: "Saved reports".to_string(),
            subtitle: "Each one is an entry in GLPI under Tools > Reports".to_string(),
            info: "A GLPI report is normally a PHP file. These are rows instead: the swcat \
                   plugin reads this table on every request and rebuilds its part of the \
                   Tools > Reports menu from it, so saving one here publishes it to GLPI \
                   with no deployment at all. Deleting one removes the menu entry the same \
                   way."
                .to_string(),

            if let Some(e) = error() {
                div { class: "mb-3 rounded border border-error/40 bg-error/10 p-3 text-sm text-error",
                    "{e}"
                }
            }

            if list.definitions.is_empty() {
                div { class: "text-base-content/60",
                    "No definitions yet. Nothing is wrong - the table is empty, or the swcat "
                    "plugin has not been installed in GLPI. The two look the same from here."
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                th { "Name" }
                                th { "Grouped by" }
                                th { "Rollup" }
                                th { "Limited to" }
                                th { "Linked" }
                                th { "" }
                            }
                        }
                        tbody {
                            for d in list.definitions.iter().cloned() {
                                tr { key: "{d.id}",
                                    td { class: "font-medium",
                                        // The name opens the report in GLPI. None
                                        // until the health response carries GLPI's
                                        // address, and the rule everywhere here is
                                        // to render no link rather than a broken one.
                                        if let Some(href) = crate::glpi_link::link(
                                            &format!("plugins/swcat/report/builder.php?id={}", d.id)
                                        ) {
                                            a {
                                                class: "link link-primary",
                                                href: "{href}",
                                                target: "_blank",
                                                rel: "noopener",
                                                title: "Open this report in GLPI",
                                                "{d.name}"
                                            }
                                        } else {
                                            "{d.name}"
                                        }
                                    }
                                    td { class: "font-mono text-xs", "{d.group_by}" }
                                    td {
                                        if d.rollup {
                                            span { class: "badge badge-sm", "to root" }
                                        } else {
                                            span { class: "text-base-content/50", "—" }
                                        }
                                    }
                                    td { class: "text-xs text-base-content/70",
                                        match &d.itemtypes {
                                            Some(v) if !v.is_empty() => v.join(", "),
                                            _ => "every type with the field".to_string(),
                                        }
                                    }
                                    td { class: "text-xs text-base-content/70",
                                        match &d.relations {
                                            Some(v) if !v.is_empty() => v.join(", "),
                                            _ => "—".to_string(),
                                        }
                                    }
                                    td { class: "text-right",
                                        if list.writes_enabled {
                                            button {
                                                class: "btn btn-ghost btn-xs text-error",
                                                onclick: move |_| {
                                                    let id = d.id;
                                                    async move {
                                                        match api::delete_report(id).await {
                                                            Ok(()) => { error.set(None); on_change.call(()); }
                                                            Err(e) => error.set(Some(e)),
                                                        }
                                                    }
                                                },
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Builder(on_saved: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut group_by = use_signal(|| "locations_id".to_string());
    let mut rollup = use_signal(|| true);
    let mut chosen = use_signal(Vec::<String>::new);
    let mut relations = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut saved = use_signal(|| false);

    rsx! {
        Panel {
            title: "New report".to_string(),
            subtitle: "Three choices and a name".to_string(),
            info: "Grouping by a tree field with rollup on is the case worth understanding: \
                   an item placed several levels down counts under the top of its tree, which \
                   is something GLPI's own dashboard cards cannot do - they match an exact \
                   node, so filtering a card to a root shows nothing while the items sit on \
                   its descendants. Rollup on a flat field is ignored, and the report says so \
                   rather than pretending."
                .to_string(),

            if let Some(e) = error() {
                div { class: "mb-3 rounded border border-error/40 bg-error/10 p-3 text-sm text-error",
                    "{e}"
                }
            }
            if saved() {
                div { class: "mb-3 rounded border border-success/40 bg-success/10 p-3 text-sm",
                    "Saved. It is in GLPI's Tools > Reports menu now, and in the list above - "
                    "its name opens it."
                }
            }

            div { class: "grid gap-4 sm:grid-cols-2",
                label { class: "form-control",
                    div { class: "label", span { class: "label-text", "Name" } }
                    input {
                        class: "input input-sm input-bordered",
                        placeholder: "Assets by location",
                        value: "{name}",
                        oninput: move |e| { name.set(e.value()); saved.set(false); },
                    }
                    div { class: "label",
                        span { class: "label-text-alt text-base-content/60",
                            "What appears in the GLPI menu."
                        }
                    }
                }

                label { class: "form-control",
                    div { class: "label", span { class: "label-text", "Group by" } }
                    select {
                        class: "select select-sm select-bordered",
                        value: "{group_by}",
                        onchange: move |e| { group_by.set(e.value()); saved.set(false); },
                        for (field, label) in GROUP_FIELDS.iter() {
                            option { value: "{field}", "{label}" }
                        }
                    }
                    div { class: "label",
                        span { class: "label-text-alt text-base-content/60",
                            "Item types without this field are left out of the report, not "
                            "counted as zero."
                        }
                    }
                }
            }

            label { class: "label mt-2 cursor-pointer justify-start gap-3",
                input {
                    r#type: "checkbox",
                    class: "checkbox checkbox-sm",
                    checked: rollup(),
                    onchange: move |e| { rollup.set(e.checked()); saved.set(false); },
                }
                span { class: "label-text",
                    "Aggregate to the top of the tree (ignored when the field is not a tree)"
                }
            }

            div { class: "mt-4",
                div { class: "label",
                    span { class: "label-text", "Limit to item types" }
                    span { class: "label-text-alt text-base-content/60",
                        "none selected = every type that has the field"
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    for it in ITEM_TYPES.iter() {
                        {
                            let it = it.to_string();
                            let on = chosen().contains(&it);
                            rsx! {
                                button {
                                    key: "{it}",
                                    class: if on { "btn btn-xs btn-primary" } else { "btn btn-xs btn-outline" },
                                    onclick: move |_| {
                                        let mut v = chosen();
                                        if let Some(i) = v.iter().position(|x| *x == it) {
                                            v.remove(i);
                                        } else {
                                            v.push(it.clone());
                                        }
                                        chosen.set(v);
                                        saved.set(false);
                                    },
                                    "{it}"
                                }
                            }
                        }
                    }
                }
            }

            div { class: "mt-4",
                div { class: "label",
                    span { class: "label-text", "Also count, through what they are linked to" }
                    span { class: "label-text-alt text-base-content/60", "optional" }
                }
                p { class: "mb-2 text-xs text-base-content/60",
                    "These carry no such field of their own - a contract is not \"in\" a "
                    "location - so each is counted under the value of the items it links to, "
                    "in a second table. GLPI's own search cannot make this join in one hop, "
                    "which is the reason the option exists."
                }
                div { class: "flex flex-wrap gap-2",
                    for rt in RELATION_TYPES.iter() {
                        {
                            let rt = rt.to_string();
                            let on = relations().contains(&rt);
                            rsx! {
                                button {
                                    key: "{rt}",
                                    class: if on { "btn btn-xs btn-secondary" } else { "btn btn-xs btn-outline" },
                                    onclick: move |_| {
                                        let mut v = relations();
                                        if let Some(i) = v.iter().position(|x| *x == rt) {
                                            v.remove(i);
                                        } else {
                                            v.push(rt.clone());
                                        }
                                        relations.set(v);
                                        saved.set(false);
                                    },
                                    "{rt}"
                                }
                            }
                        }
                    }
                }
            }

            div { class: "mt-5",
                button {
                    class: "btn btn-sm btn-primary",
                    disabled: name().trim().is_empty(),
                    onclick: move |_| async move {
                        let picked = chosen();
                        let rels = relations();
                        let def = ReportDef {
                            id: 0,
                            name: name().trim().to_string(),
                            itemtypes: (!picked.is_empty()).then_some(picked),
                            group_by: group_by(),
                            rollup: rollup(),
                            relations: (!rels.is_empty()).then_some(rels),
                            is_active: true,
                        };
                        match api::create_report(&def).await {
                            Ok(()) => {
                                error.set(None);
                                saved.set(true);
                                name.set(String::new());
                                chosen.set(Vec::new());
                                relations.set(Vec::new());
                                on_saved.call(());
                            }
                            Err(e) => { error.set(Some(e)); saved.set(false); }
                        }
                    },
                    "Save report"
                }
            }
        }
    }
}
