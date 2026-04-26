use crate::state::ServerState;
use axum::{extract::State, response::IntoResponse};
use common::models::ModeSummary;
use common::types::{ModeId, PlayerCount};
use jsonc_parser::parse_to_serde_value;
use std::{fs, sync::Arc};

/// Builds a snapshot of current mode status for list endpoints.
async fn mode_list_snapshot(state: &Arc<ServerState>) -> Vec<ModeSummary> {
    let public_games = state.public_game_instances().await;
    let private_games = state.private_game_instances().await;

    let mut list = Vec::new();
    for (mode_id, instance) in public_games {
        let players = mode_player_count(state, &mode_id, &private_games, &instance).await;
        let queue_target = state.queue_target_players(&mode_id);
        list.push(ModeSummary {
            id: mode_id,
            display_name: instance.mode_display_name().to_string(),
            players,
            max_players: queue_target.unwrap_or_else(|| instance.max_players()),
            queue_players: queue_target.unwrap_or_else(PlayerCount::zero),
            respawn_cooldown_ms: instance.respawn_cooldown_ms(),
        });
    }
    list
}

async fn mode_player_count(
    state: &Arc<ServerState>,
    mode_id: &ModeId,
    private_games: &[(ModeId, Arc<crate::instance::GameInstance>)],
    instance: &Arc<crate::instance::GameInstance>,
) -> PlayerCount {
    let queue_target = state.queue_target_players(mode_id);
    if queue_target.is_some() {
        let private_mode_prefix = format!("{}__", mode_id.as_ref());
        let mut active_match_players = PlayerCount::zero();
        for (private_id, private_instance) in private_games {
            if private_id.as_ref().starts_with(&private_mode_prefix) {
                active_match_players += private_instance.player_count().await;
            }
        }
        state.queue_len(mode_id).await + active_match_players
    } else {
        instance.player_count().await
    }
}

/// Serves the index HTML with injected mode/global JSON.
pub async fn index_html(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let html_path = crate::paths::client_dist_dir().join("index.html");
    let html = fs::read_to_string(html_path)
        .unwrap_or_else(|_| "<!doctype html><body>missing index</body>".to_string());
    let modes_json = serde_json::to_string(&mode_list_snapshot(&state).await)
        .unwrap_or_else(|_| "[]".to_string());
    let global_json = fs::read_to_string("config/global/client.jsonc")
        .ok()
        .and_then(|raw| {
            parse_to_serde_value(&raw, &Default::default())
                .ok()
                .flatten()
                .and_then(|value| serde_json::to_string(&value).ok())
        })
        .unwrap_or_else(|| "{}".to_string());
    let replaced = html
        .replace("__MODES_JSON__", &modes_json)
        .replace("__GLOBAL_JSON__", &global_json);
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        replaced,
    )
}

/// Lists current game modes and player counts.
pub async fn list_modes(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    axum::Json(mode_list_snapshot(&state).await)
}
