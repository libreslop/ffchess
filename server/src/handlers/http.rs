//! HTTP endpoints serving the client page and the mode list.

use crate::state::ServerState;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use std::sync::Arc;

/// Serves the client page with the mode list and client globals injected.
pub async fn index_html(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let html_path = crate::paths::client_dist_dir().join("index.html");
    let html = std::fs::read_to_string(html_path)
        .unwrap_or_else(|_| "<!doctype html><body>missing index</body>".to_string());
    let modes_json =
        serde_json::to_string(&state.mode_summaries()).unwrap_or_else(|_| "[]".to_string());
    let page = html
        .replace("__MODES_JSON__", &modes_json)
        .replace("__GLOBAL_JSON__", &state.config().client_global_json);
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html")], page)
}

/// Lists current game modes and player counts.
pub async fn list_modes(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    axum::Json(state.mode_summaries())
}
