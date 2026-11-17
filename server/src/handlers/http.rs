//! HTTP endpoints serving the client page, the mode list, and replays.

use crate::state::ServerState;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use common::types::ReplayId;
use std::sync::Arc;

/// Main menu HTML, prerendered once to show while the client loads.
#[derive(Clone)]
pub struct MenuSnapshot(Arc<str>);

impl MenuSnapshot {
    /// Prerenders the main menu of the configured modes, as the client would render it before
    /// it knows how many players are online.
    pub async fn render(state: &ServerState) -> Self {
        let html = client::render_menu_snapshot(
            state.mode_summaries(),
            &state.config().client_global_json,
        )
        .await;
        Self(html.into())
    }
}

/// Serves the client page with the mode list, client globals, and prerendered menu injected.
pub async fn index_html(
    State(state): State<Arc<ServerState>>,
    Extension(menu): Extension<MenuSnapshot>,
) -> impl IntoResponse {
    let html_path = crate::paths::client_dist_dir().join("index.html");
    let html = std::fs::read_to_string(html_path)
        .unwrap_or_else(|_| "<!doctype html><body>missing index</body>".to_string());
    let modes_json =
        serde_json::to_string(&state.mode_summaries()).unwrap_or_else(|_| "[]".to_string());
    let page = html
        .replace("__MODES_JSON__", &modes_json)
        .replace("__GLOBAL_JSON__", &state.config().client_global_json)
        .replace("__MENU_HTML__", &menu.0);
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html")], page)
}

/// Lists current game modes and player counts.
pub async fn list_modes(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    axum::Json(state.mode_summaries())
}

/// Serves a replay as JSON, or 404 while its match runs, after it expired, or when unknown.
pub async fn get_replay(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<ReplayId>,
) -> impl IntoResponse {
    let replay = tokio::task::spawn_blocking(move || state.finished_replay(id))
        .await
        .ok()
        .flatten();
    match replay {
        Some(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
