//! GLPI inventory telemetry - the view.
//!
//! A launcher and nothing else. The router, layout and global signals live in
//! app.rs and the fetch helpers in api.rs, which is the split RERAG uses: a
//! page that wants data calls api.rs, and a page that wants to navigate names
//! a Route. Neither reaches for reqwest or a URL directly.
mod api;
mod app;
mod components;
mod glpi_link;
mod pages;
mod theme;

fn main() {
    dioxus::launch(app::App);
}
