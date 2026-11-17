//! Axum handlers for HTTP endpoints and WebSocket sessions.

mod http;
mod ws;

pub use http::{MenuSnapshot, get_replay, index_html, list_modes};
pub use ws::ws_handler;
