//! Root application module wiring config, websocket, and top-level UI.

mod browser;
mod component;
mod config;
mod focus;
mod ws;

pub use component::App;
pub use config::GlobalClientConfig;
