//! Root application module wiring config, websocket, and top-level UI.

mod component;
mod config;
mod favicon;
mod ws;

pub use component::App;
pub use config::GlobalClientConfig;
