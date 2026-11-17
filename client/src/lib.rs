//! Yew-based browser client, and with the `ssr` feature the main menu prerendered by the
//! server.

mod app;
mod camera;
mod canvas;
mod components;
mod math;
mod premoves;
mod reducer;
#[cfg(feature = "ssr")]
mod snapshot;
mod ui_state;
mod utils;

#[cfg(feature = "ssr")]
pub use snapshot::render_menu_snapshot;

/// Id of the element the root app component mounts into.
const APP_ROOT_ID: &str = "app";

/// Mounts the root app component beside the prerendered menu, which the app removes once it
/// shows a screen of its own.
pub fn run() {
    let root = gloo_utils::document()
        .get_element_by_id(APP_ROOT_ID)
        .expect("page has an app root");
    yew::Renderer::<app::App>::with_root(root).render();
}
