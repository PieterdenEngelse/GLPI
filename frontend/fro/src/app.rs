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
use crate::pages::{About, Agents, Computers, PageNotFound, Rights, Status};
use dioxus::prelude::*;
use dioxus_router::{Outlet, Routable, Router};

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
        #[route("/rights")]
        Rights {},
        #[route("/about")]
        About {},
    #[end_layout]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
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
