//! The app's destinations, declared once.
//!
//! Two things render this and they do different jobs with it: the strip in the
//! header (nav_tabs.rs) gives every page one-click reach, and the sidebar
//! additionally nests what is under the active page. What they must never
//! disagree about is which pages exist, which is why the list lives here and
//! neither of them owns a copy.
//!
//! `Route` carries `String` fields, so the destinations cannot be a `const` -
//! they are built per call. The name tables below can be, because they are the
//! strings that appear in a URL.
use crate::app::Route;

/// One entry in the navigation.
pub struct Dest {
    pub route: Route,
    pub label: &'static str,
    /// Whether the header strip shows it.
    ///
    /// Templates is the only false: it is reached from the Screens page, where
    /// the question it answers comes up, and the strip is already at ten. The
    /// sidebar shows it regardless - a menu can afford a row where a strip
    /// cannot afford a column.
    pub in_header: bool,
}

/// One entry below a destination, and possibly entries below that.
///
/// Clone and PartialEq because it crosses a component boundary: Dioxus props
/// must be both, so it can tell whether a re-render changed anything.
///
/// A tree rather than a list because the DB page is three deep: DB, its tabs,
/// and the sub-tabs two of those tabs carry. Nothing builds a fourth level;
/// past three it stops being navigation and becomes a document.
#[derive(Clone, PartialEq)]
pub struct Node {
    pub route: Route,
    /// Owned, not &'static: the Logs section's entries are file names that
    /// arrive at runtime, and the alternative was leaking one string per file
    /// per render to satisfy a lifetime.
    pub label: String,
    /// Whether the reader is on this exact entry.
    pub here: bool,
    pub subs: Vec<Node>,
}

/// Second-level names, as they appear in each page's URL. These mirror the
/// `TABS` constants the pages route on.
pub const SCREENS_TABS: [(&str, &str); 4] = [
    ("cf", "Custom fields"),
    ("def", "Definitions"),
    ("rights", "Rights"),
    ("instruct", "How to"),
];
/// Third level under Screens: the How-to tab's two halves.
pub const INSTRUCT_SUBS: [(&str, &str); 2] = [("general", "General"), ("template", "Template")];

pub const RIGHTS_TABS: [(&str, &str); 3] =
    [("profiles", "Profiles"), ("api", "API"), ("db", "Database")];
pub const DB_TABS: [(&str, &str); 4] = [
    ("overview", "Overview"),
    ("db-read", "db-read"),
    ("api-read", "api-read"),
    ("maria", "Maria"),
];

/// Third-level names: the sub-tabs two of the DB tabs carry, as they appear in
/// the URL's `sub`.
pub const MARIA_SUBS: [(&str, &str); 3] = [
    ("general", "general"),
    ("storage", "storage"),
    ("databases", "db\u{2019}s"),
];
pub const API_SUBS: [(&str, &str); 3] =
    [("fields", "fields"), ("schema", "schema"), ("rsql", "rsql")];

/// Every page, in the order both renderers show them.
///
/// About is deliberately absent: it describes the app rather than showing
/// data, so it sits apart on the right of the header and does not compete with
/// the pages a reader switches between.
pub fn destinations() -> Vec<Dest> {
    vec![
        Dest {
            route: Route::Agents {},
            label: "Agents",
            in_header: true,
        },
        Dest {
            route: Route::Computers {},
            label: "Computers",
            in_header: true,
        },
        Dest {
            route: Route::Status {},
            label: "Status",
            in_header: true,
        },
        Dest {
            route: Route::Screens {
                tab: String::new(),
                sub: String::new(),
            },
            label: "Screens",
            in_header: true,
        },
        Dest {
            route: Route::TemplateRow {},
            label: "Templates",
            in_header: false,
        },
        Dest {
            route: Route::Inv {},
            label: "Inv",
            in_header: true,
        },
        Dest {
            route: Route::Rights { tab: String::new() },
            label: "Rights",
            in_header: true,
        },
        Dest {
            route: Route::Db {
                tab: String::new(),
                sub: String::new(),
            },
            label: "DB",
            in_header: true,
        },
        // Empty so far - see pages/netw.rs. Distinct from the fleet network
        // view at /inv/network, which stays where it is under Inv.
        Dest {
            route: Route::Netw {},
            label: "Netw",
            in_header: true,
        },
        Dest {
            route: Route::Sec {},
            label: "Security",
            in_header: true,
        },
        // Empty log: the bare page, which falls back to the first file.
        Dest {
            route: Route::Logs { log: String::new() },
            label: "Logs",
            in_header: true,
        },
    ]
}

/// Whether `dest` is the section the reader is in.
///
/// Discriminant comparison rather than equality, because a variant carrying
/// fields never matches a freshly constructed one - the 404 carries segments,
/// Db carries its tab. Inv and Templates are sections rather than single
/// pages, so each matches any of its children; without that, /inv/hardware
/// would light nothing at all.
pub fn is_active(dest: &Route, current: &Route) -> bool {
    match dest {
        Route::Inv {} => matches!(
            current,
            Route::Inv {}
                | Route::InvHardware {}
                | Route::InvSoftware {}
                | Route::InvNetwork {}
                | Route::InvRefused {}
        ),
        Route::TemplateRow {} => matches!(
            current,
            Route::TemplateRow {}
                | Route::TemplateValues {}
                | Route::TemplateForm {}
                | Route::TemplateMessage {}
        ),
        _ => std::mem::discriminant(dest) == std::mem::discriminant(current),
    }
}

/// What sits under the active page: its tabs, and the active tab's own
/// sub-tabs where it has them.
///
/// Empty for a page with none, which is what keeps the sidebar short - only
/// the section being read is expanded.
pub fn children(current: &Route) -> Vec<Node> {
    match current {
        Route::Screens { tab, sub } => SCREENS_TABS
            .iter()
            .map(|(name, label)| {
                let on_this_tab = selected(tab, name, SCREENS_TABS[0].0);
                let subs = if *name == "instruct" && on_this_tab {
                    INSTRUCT_SUBS
                        .iter()
                        .map(|(sub_name, sub_label)| Node {
                            route: Route::Screens {
                                tab: "instruct".to_string(),
                                sub: sub_name.to_string(),
                            },
                            label: sub_label.to_string(),
                            here: selected(sub, sub_name, INSTRUCT_SUBS[0].0),
                            subs: vec![],
                        })
                        .collect()
                } else {
                    vec![]
                };
                Node {
                    route: Route::Screens {
                        tab: name.to_string(),
                        sub: String::new(),
                    },
                    label: label.to_string(),
                    here: on_this_tab,
                    subs,
                }
            })
            .collect(),
        Route::Rights { tab } => tabs_of(&RIGHTS_TABS, tab, |name| Route::Rights {
            tab: name.to_string(),
        }),
        Route::Db { tab, sub } => DB_TABS
            .iter()
            .map(|(name, label)| {
                let on_this_tab = selected(tab, name, DB_TABS[0].0);
                // Only the tab being read gets its sub-tabs, for the same
                // reason only the active page gets its tabs.
                let subs = match (*name, on_this_tab) {
                    ("maria", true) => db_subs(&MARIA_SUBS, "maria", sub),
                    ("api-read", true) => db_subs(&API_SUBS, "api-read", sub),
                    _ => vec![],
                };
                Node {
                    route: Route::Db {
                        tab: name.to_string(),
                        sub: String::new(),
                    },
                    label: label.to_string(),
                    here: on_this_tab,
                    subs,
                }
            })
            .collect(),
        Route::Inv {}
        | Route::InvHardware {}
        | Route::InvSoftware {}
        | Route::InvNetwork {}
        | Route::InvRefused {} => vec![
            leaf(
                Route::InvHardware {},
                "Hardware",
                matches!(current, Route::InvHardware {}),
            ),
            leaf(
                Route::InvSoftware {},
                "Software",
                matches!(current, Route::InvSoftware {}),
            ),
            leaf(
                Route::InvNetwork {},
                "Network",
                matches!(current, Route::InvNetwork {}),
            ),
            leaf(
                Route::InvRefused {},
                "Refused",
                matches!(current, Route::InvRefused {}),
            ),
        ],
        Route::TemplateRow {}
        | Route::TemplateValues {}
        | Route::TemplateForm {}
        | Route::TemplateMessage {} => vec![
            leaf(
                Route::TemplateRow {},
                "Flagged row",
                matches!(current, Route::TemplateRow {}),
            ),
            leaf(
                Route::TemplateValues {},
                "Values",
                matches!(current, Route::TemplateValues {}),
            ),
            leaf(
                Route::TemplateForm {},
                "Form",
                matches!(current, Route::TemplateForm {}),
            ),
            leaf(
                Route::TemplateMessage {},
                "Message",
                matches!(current, Route::TemplateMessage {}),
            ),
        ],
        _ => vec![],
    }
}

/// A page's tabs, where an empty `tab` in the URL means the first one.
fn tabs_of(
    names: &'static [(&'static str, &'static str)],
    tab: &str,
    to: impl Fn(&str) -> Route,
) -> Vec<Node> {
    names
        .iter()
        .map(|(name, label)| Node {
            route: to(name),
            label: label.to_string(),
            here: selected(tab, name, names[0].0),
            subs: vec![],
        })
        .collect()
}

/// The sub-tabs of one DB tab, carrying that tab in every link so following
/// one does not drop the reader back to the page's default tab.
fn db_subs(
    names: &'static [(&'static str, &'static str)],
    tab: &'static str,
    sub: &str,
) -> Vec<Node> {
    names
        .iter()
        .map(|(name, label)| Node {
            route: Route::Db {
                tab: tab.to_string(),
                sub: name.to_string(),
            },
            label: label.to_string(),
            here: selected(sub, name, names[0].0),
            subs: vec![],
        })
        .collect()
}

fn leaf(route: Route, label: &str, here: bool) -> Node {
    Node {
        route,
        label: label.to_string(),
        here,
        subs: vec![],
    }
}

/// A URL that names no tab lands on the first one, so that is what lights.
fn selected(from_url: &str, name: &str, first: &str) -> bool {
    from_url == name || (from_url.is_empty() && name == first)
}
