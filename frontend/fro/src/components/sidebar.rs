//! The left navigation: every page, and the active page's own tabs under it.
//!
//! This replaced a horizontal strip in the header plus a tab strip inside each
//! page. Two reasons, in order of weight. A horizontal strip cannot grow - ten
//! destinations already filled a laptop width, and this machine has no mouse,
//! so reaching a tab that has scrolled off is the expensive gesture. And the
//! per-page strips cost a line of height each, which is the scarce axis: the
//! window is 1080 tall at 120% scale, where width is not short at all.
//!
//! The destinations come from components/nav.rs, shared with the header
//! strip, so the two cannot disagree about which pages exist. What is local
//! to this component is the nesting: only the sidebar shows the active page's
//! own tabs beneath it.
//!
//! DYNAMIC MEANS ONE THING HERE: only the active section shows its children.
//! Every page's sub-tabs listed at once would be forty-odd entries, which is a
//! menu to read rather than to use. The children are built from the same
//! constants the pages route on, so a tab added to a page appears here without
//! a second edit.
use crate::app::Route;
use crate::components::nav;
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// Whether the sidebar is showing. Held by `Layout`, toggled from the header.
///
/// A newtype rather than a bare `Signal<bool>`: context is keyed by type, and
/// any other boolean signal provided above this would silently bind instead -
/// the same trap `InstructTab` documents on the Screens page.
#[derive(Clone, Copy)]
pub struct SidebarOpen(pub Signal<bool>);

const STORAGE_KEY: &str = "sidebar-open";

/// Whether the sidebar was open last time, defaulting to open.
///
/// Every failure - no window, storage blocked, a value from some other
/// version - collapses to the default, the same way theme::stored does. A
/// menu preference is not worth propagating an error for.
pub fn remembered() -> bool {
    let stored = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten());
    !matches!(stored.as_deref(), Some("closed"))
}

/// Remember the choice, so a reader who closed it does not meet it again on
/// every reload.
pub fn remember(open: bool) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, if open { "open" } else { "closed" });
    }
}

#[component]
pub fn Sidebar() -> Element {
    let current = use_route::<Route>();
    let SidebarOpen(open) = use_context::<SidebarOpen>();

    if !open() {
        return rsx! {};
    }

    rsx! {
        nav {
            // No md: breakpoint on this. Collapsing is the hamburger's job, and
            // hiding it by width as well would make that button appear dead on
            // a narrow window. py-1 on the header plus a 28px mark puts its
            // height at 2.5rem, which is what the offset and the height here
            // subtract - not the 3.5rem a default navbar would take.
            class: "sticky top-10 h-[calc(100vh-2.5rem)] w-52 shrink-0 overflow-y-auto border-r border-base-300 bg-base-100 px-2 py-3",
            ul { class: "menu menu-sm w-full gap-0.5 p-0",
                for dest in nav::destinations() {
                    // Logs is the one section whose children cannot be
                    // declared: they are whatever files the backend finds, so
                    // they are fetched instead - and only while that section
                    // is open, so no other page pays for the request.
                    if matches!(dest.route, Route::Logs { .. })
                        && nav::is_active(&dest.route, &current)
                    {
                        LogsSection { current: current.clone() }
                    } else {
                        SidebarItem {
                            key: "{dest.label}",
                            to: dest.route.clone(),
                            label: dest.label.to_string(),
                            active: nav::is_active(&dest.route, &current),
                            subs: if nav::is_active(&dest.route, &current) {
                                nav::children(&current)
                            } else {
                                vec![]
                            },
                        }
                    }
                }
            }
        }
    }
}

/// One entry, with whatever is under it.
///
/// Recursive, because the DB page is three deep and a second hand-written
/// level would have to be kept in step with the first. The parent link stays
/// clickable at every level: clicking "DB" goes to the page's default tab,
/// which is what a reader expects of a heading they can see is a link.
#[component]
fn SidebarItem(to: Route, label: String, active: bool, subs: Vec<nav::Node>) -> Element {
    rsx! {
        li {
            Link {
                class: if active { "font-medium menu-active" } else { "" },
                to,
                "{label}"
            }
            if active && !subs.is_empty() {
                ul {
                    for node in subs {
                        SidebarItem {
                            key: "{node.label}",
                            to: node.route,
                            label: node.label,
                            active: node.here,
                            subs: node.subs,
                        }
                    }
                }
            }
        }
    }
}

/// Logs, with one entry per file the backend is willing to serve.
///
/// Its own component so the fetch happens only when the section is open: a
/// hook cannot be conditional, but a component that is not rendered has none.
/// While the list is loading, or if it cannot be had, the section is just a
/// link - a menu that shows an error is worse than one that shows a page.
#[component]
fn LogsSection(current: Route) -> Element {
    let listing = use_resource(|| async move { crate::api::fetch_logs().await });

    let subs: Vec<nav::Node> = match &*listing.read_unchecked() {
        Some(Ok(list)) => list
            .entries
            .iter()
            .map(|e| {
                // The URL names a log by its stem, which is what the page
                // parses - linking to the file name would miss and silently
                // fall back to the first log.
                let stem = e.file.trim_end_matches(".log").to_string();
                nav::Node {
                    here: matches!(&current, Route::Logs { log } if log == &stem),
                    // All fifteen, not only the readable ones: a log GLPI
                    // writes but this app will not serve is still worth
                    // knowing exists. The bullet marks the two whose contents
                    // can actually be shown, as the page's strip did.
                    label: if e.readable {
                        format!("{stem} \u{2022}")
                    } else {
                        stem.clone()
                    },
                    route: Route::Logs { log: stem },
                    subs: vec![],
                }
            })
            .collect(),
        _ => vec![],
    };

    rsx! {
        SidebarItem {
            to: Route::Logs { log: String::new() },
            label: "Logs",
            active: true,
            subs,
        }
    }
}
