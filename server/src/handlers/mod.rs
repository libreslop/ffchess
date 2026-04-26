//! Axum handlers for HTTP endpoints and WebSocket sessions.

mod http;
mod name;
mod queue;
mod ws;

pub use http::{index_html, list_modes};
pub use ws::ws_handler;
