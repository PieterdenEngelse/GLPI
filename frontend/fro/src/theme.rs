//! Theme selection: which of daisyUI's two themes is active, and where that
//! choice is remembered.
//!
//! Dark is the default, declared in index.css as `dark --default`. That means
//! "no `data-theme` attribute" already renders dark, so nothing has to run for
//! the default to be correct - the stylesheet alone is enough, and a user who
//! never touches the toggle gets dark even if the wasm fails to start.
//!
//! The system preference deliberately does not participate. `--prefersdark`
//! was removed when dark became the default: an explicit default and an
//! automatic one cannot both win, and a toggle that silently loses to the OS
//! on the next reload is worse than no toggle.
use dioxus::prelude::*;

const STORAGE_KEY: &str = "glpi-inv-theme";
pub const DARK: &str = "dark";
pub const LIGHT: &str = "light";

/// The stored choice, if the viewer has made one.
///
/// Every failure here - no window, storage blocked by the browser, a value
/// written by some other version - collapses to None, which means dark. A
/// theme preference is not worth propagating an error for.
pub fn stored() -> Option<String> {
    let v = web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(STORAGE_KEY)
        .ok()??;
    (v == DARK || v == LIGHT).then_some(v)
}

/// Write `data-theme` on the root element, which is what daisyUI's selectors
/// key off, and remember the choice.
pub fn apply(theme: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Some(root) = window.document().and_then(|d| d.document_element()) {
        let _ = root.set_attribute("data-theme", theme);
    }
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(STORAGE_KEY, theme);
    }
}

/// Set the theme on load: the stored choice, or dark.
///
/// Writes `data-theme` unconditionally rather than leaning on "no attribute
/// means dark". That default is real - index.css declares `dark --default` -
/// but it depends on a top-level `:root:not([data-theme])` rule outranking a
/// later `:where(:root)` rule that daisyUI also emits for the light theme.
/// Correct, and far too subtle to rest a visible behaviour on: an explicit
/// `[data-theme=dark]` attribute wins on its own terms and cannot be
/// reordered out of effect by a future daisyUI release.
pub fn restore() {
    apply(stored().as_deref().unwrap_or(DARK));
}

/// A signal seeded from storage, for components that need to show which theme
/// is active rather than only set it.
pub fn use_theme() -> Signal<String> {
    use_signal(|| stored().unwrap_or_else(|| DARK.to_string()))
}
