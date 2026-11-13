//! Router, layout, and the chrome every page shares.
//!
//! Shaped after RERAG's frontend/fro/src/app.rs: one Route enum, a single
//! #[layout] wrapping the real pages, and a catch-all after #[end_layout] so
//! an unknown path renders a 404 instead of a blank screen.
//!
//! The backend serves this bundle with a ServeDir that falls back to
//! index.html, so a client-side route survives a page reload or a pasted
//! link - without that fallback every path but / would 404 from the server
//! before the router ever ran.
use crate::components::{GlobalErrorBar, Header};
use crate::pages::{
    About, Agents, Computers, Db, Inv, InvHardware, InvNetwork, InvRefused, InvSoftware, Logs,
    Netw, PageNotFound, Rights, Screens, Sec, Status, TemplateForm, TemplateMessage, TemplateRow,
    TemplateValues,
};
use dioxus::prelude::*;
use dioxus_router::{use_route, Outlet, Routable, Router};

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
        #[route("/")]
        Agents {},
        #[route("/computers")]
        Computers {},
        #[route("/status")]
        Status {},
        // Tab in the URL, same reasoning as /logs below.
        #[route("/screens?:tab")]
        Screens { tab: String },
        // The four things GLPI calls a template, one page each. Flat routes
        // and no top-nav entry: they are reached from the Screens page, which
        // is where the question arises. Same shape as the /inv children.
        #[route("/templates/row")]
        TemplateRow {},
        #[route("/templates/values")]
        TemplateValues {},
        #[route("/templates/form")]
        TemplateForm {},
        #[route("/templates/message")]
        TemplateMessage {},
        // The inventory section: a landing page and four views under it. Flat
        // routes rather than nested - the sub-header is one component used by
        // all five, so there is nothing a nested layout would factor out.
        #[route("/inv")]
        Inv {},
        #[route("/inv/hardware")]
        InvHardware {},
        #[route("/inv/software")]
        InvSoftware {},
        #[route("/inv/network")]
        InvNetwork {},
        #[route("/inv/refused")]
        InvRefused {},
        #[route("/rights?:tab")]
        Rights { tab: String },
        // Two arguments: the page strip and the one strip inside it. The
        // sub-strips on Maria and api-read are in different tabs, so they
        // never both apply and can share `sub`. The third level - Rsql's own
        // strip inside api-read - stays page state; it is a detail of one
        // sub-tab rather than a place you would link someone to.
        #[route("/db?:tab&:sub")]
        Db { tab: String, sub: String },
        #[route("/netw")]
        Netw {},
        #[route("/sec")]
        Sec {},
        // The tab is in the URL as a query argument, so a particular log is
        // linkable and survives a reload. Query rather than a path segment:
        // the strip is built from whatever files exist, so a path would turn
        // a missing file into a 404 instead of a fallback to the first tab.
        #[route("/logs?:log")]
        Logs { log: String },
        #[route("/about")]
        About {},
    #[end_layout]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}

impl Route {
    /// The browser tab's text for this page.
    ///
    /// An exhaustive match on purpose: a new route does not compile until it
    /// is given a title here, which is the only way "every page has one" stays
    /// true without anyone remembering to check. The names follow whatever the
    /// nav calls the page rather than its heading, so the tab and the thing
    /// clicked to reach it agree.
    pub fn title(&self) -> &'static str {
        match self {
            Route::Agents {} => "Agents",
            Route::Computers {} => "Computers",
            Route::Status {} => "Status",
            Route::Screens { .. } => "Screens",
            Route::TemplateRow {} => "Flagged row",
            Route::TemplateValues {} => "Template values",
            Route::TemplateForm {} => "Template form",
            Route::TemplateMessage {} => "Template message",
            Route::Inv {} => "Inv",
            Route::InvHardware {} => "Hardware",
            Route::InvSoftware {} => "Software",
            Route::InvNetwork {} => "Network",
            Route::InvRefused {} => "Refused",
            Route::Rights { .. } => "Rights",
            Route::Db { .. } => "DB",
            Route::Netw {} => "Netw",
            Route::Sec {} => "Security",
            Route::Logs { .. } => "Logs",
            Route::About {} => "About",
            Route::PageNotFound { .. } => "Not found",
        }
    }
}

/// Sets the browser tab's title from the current route.
///
/// Rendered in `Layout` and again in the 404, for the same reason the
/// stylesheet is declared in `App`: the catch-all sits outside the layout by
/// design, and a page with no title would silently keep the previous one -
/// the tab would name wherever you had just been.
///
/// Page name first, app name after: a tab strip truncates from the right, so
/// the distinguishing half has to lead.
#[component]
pub fn PageTitle() -> Element {
    let route = use_route::<Route>();

    // web-sys rather than `document::Title { .. }` or Dioxus's own
    // `document().set_title()`. Both of those were tried here and neither
    // changed the tab: the component accepts only two exact rsx template
    // shapes and logs to the console when its children match neither, and
    // set_title defers through queue_effect. This is the same
    // window().document() route theme.rs already takes, set synchronously
    // during render, with no abstraction left to fail silently.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        doc.set_title(&format!("{} \u{b7} GLPI inventory", route.title()));
    }

    rsx! {}
}

#[component]
pub fn App() -> Element {
    // Re-apply a stored theme choice once, before the first paint. Nothing
    // runs when none is stored: dark is the stylesheet's default, so the
    // absence of a data-theme attribute is already correct.
    use_hook(crate::theme::restore);

    // Fetched once here so every info panel can link into GLPI.
    crate::glpi_link::provide();

    rsx! {
        // Declared here rather than in Layout so the 404 - which sits outside
        // the layout by design - is styled like everything else.
        //
        // output.css is GENERATED from index.css plus the class names found in
        // these .rs files; `just css-build` regenerates it and the frontend
        // recipes depend on that. Editing it directly is overwritten silently.
        document::Stylesheet { href: asset!("/assets/styling/output.css") }
        Router::<Route> {}
    }
}

#[component]
fn Layout() -> Element {
    rsx! {
        PageTitle {}
        // A full-bleed application surface, not a centred document. The
        // frame owns the background so every page - including the 404 - sits
        // on the same colour, and min-h-screen keeps it covering the viewport
        // when a page is short.
        div { class: "flex min-h-screen flex-col bg-base-200 text-base-content",
            GlobalErrorBar {}
            Header {}
            main { class: "w-full flex-1 px-4 py-4", Outlet::<Route> {} }
        }
    }
}
