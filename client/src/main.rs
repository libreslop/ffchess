mod app;
mod camera;
mod canvas;
mod components;
mod reducer;
mod utils;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
