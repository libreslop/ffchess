//! Client entrypoint for the Yew-based UI.

mod app;
mod camera;
mod canvas;
mod components;
mod math;
mod reducer;
mod utils;

/// Boots the Yew renderer and mounts the root app component.
fn main() {
    yew::Renderer::<app::App>::new().render();
}
