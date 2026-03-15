use crate::colors::ColorManager;
use common::models::{GameModeConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::ServerMessage;
use common::types::{PieceId, PieceTypeId, PlayerId, SessionSecret, ShopId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

pub struct GameInstance {
    pub mode_config: GameModeConfig,
    pub piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
    pub shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    pub game: RwLock<GameState>,
    pub player_channels: RwLock<HashMap<PlayerId, mpsc::UnboundedSender<ServerMessage>>>,
    pub session_secrets: RwLock<HashMap<PlayerId, SessionSecret>>,
    pub removed_pieces: RwLock<Vec<PieceId>>,
    pub removed_players: RwLock<Vec<PlayerId>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<i64>,
    pub death_timestamps: RwLock<HashMap<PlayerId, i64>>,
}

impl GameInstance {
    pub fn new(
        mode_config: GameModeConfig,
        piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
        shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    ) -> Self {
        let board_size = common::logic::calculate_board_size(&mode_config, 0);
        Self {
            mode_config: mode_config.clone(),
            piece_configs,
            shop_configs,
            game: RwLock::new(GameState {
                board_size,
                mode_id: mode_config.id.clone(),
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

    pub async fn broadcast(&self, msg: ServerMessage) {
        let channels = self.player_channels.read().await;
        for tx in channels.values() {
            let _ = tx.send(msg.clone());
        }
    }

    pub async fn record_piece_removal(&self, piece_id: PieceId) {
        self.removed_pieces.write().await.push(piece_id);
    }

    pub async fn prune_out_of_bounds(&self, game: &mut GameState) {
        let board_size = game.board_size;
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if !common::logic::is_within_board(p.position, board_size) {
                rp.push(*id);
                false
            } else {
                true
            }
        });
        game.shops
            .retain(|s| common::logic::is_within_board(s.position, board_size));
    }
}
