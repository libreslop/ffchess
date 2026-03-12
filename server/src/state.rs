use crate::colors::ColorManager;
use common::*;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct ServerState {
    pub game: RwLock<GameState>,
    pub player_channels: RwLock<HashMap<Uuid, tokio::sync::mpsc::UnboundedSender<ServerMessage>>>,
    pub session_secrets: RwLock<HashMap<Uuid, Uuid>>,
    pub removed_pieces: RwLock<Vec<Uuid>>,
    pub removed_players: RwLock<Vec<Uuid>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<i64>,
    pub death_timestamps: RwLock<HashMap<Uuid, i64>>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            game: RwLock::new(GameState {
                board_size: 40,
                ..Default::default()
            }),
            player_channels: RwLock::new(HashMap::new()),
            session_secrets: RwLock::new(HashMap::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            color_manager: RwLock::new(ColorManager::new()),
            last_viewed_at: RwLock::new(chrono::Utc::now().timestamp_millis()),
            death_timestamps: RwLock::new(HashMap::new()),
        }
    }

    pub async fn record_piece_removal(&self, piece_id: Uuid) {
        self.removed_pieces.write().await.push(piece_id);
    }

    pub async fn broadcast(&self, msg: ServerMessage) {
        let channels = self.player_channels.read().await;
        for tx in channels.values() {
            let _ = tx.send(msg.clone());
        }
    }

    pub async fn cleanup_death_timestamps(&self, now_ms: i64, max_age_ms: i64) {
        let mut dt = self.death_timestamps.write().await;
        dt.retain(|_, timestamp_ms| now_ms - *timestamp_ms <= max_age_ms);
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
