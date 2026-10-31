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
use crate::components::Header;
use crate::pages::{About, Fleet, PageNotFound};
use dioxus::prelude::*;
use dioxus_router::{Outlet, Routable, Router};

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
        #[route("/")]
        Fleet {},
        #[route("/about")]
        About {},
    #[end_layout]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}

#[component]
pub fn App() -> Element {
    rsx! {
        // Declared here rather than in Layout so the 404 - which sits outside
        // the layout by design - is styled like everything else.
        document::Stylesheet { href: asset!("/assets/styling/main.css") }
        Router::<Route> {}
    }
}

#[component]
fn Layout() -> Element {
    rsx! {
        Header {}
        main { Outlet::<Route> {} }
    }
}
