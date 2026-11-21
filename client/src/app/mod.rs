//! Root application module wiring config, websocket, and top-level UI.

mod browser;
mod component;
mod config;
mod focus;
mod replay_link;
mod ws;

pub use component::App;
pub use config::GlobalClientConfig;
#[cfg(feature = "ssr")]
pub use config::order_modes;
pub use replay_link::ReplayLink;
