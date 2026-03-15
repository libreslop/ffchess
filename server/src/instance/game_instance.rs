//! Shared per-mode state container for live game instances.

use crate::colors::ColorManager;
use crate::time::now_ms;
use crate::types::ConnectionId;
use common::models::{GameModeConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::ServerMessage;
use common::types::{PieceId, PieceTypeId, PlayerId, SessionSecret, ShopId, TimestampMs};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// Live game instance state for a single mode.
pub struct GameInstance {
    pub mode_config: GameModeConfig,
    pub piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
    pub shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    pub game: RwLock<GameState>,
    pub connection_channels: RwLock<HashMap<ConnectionId, mpsc::UnboundedSender<ServerMessage>>>,
    pub player_channels: RwLock<HashMap<PlayerId, mpsc::UnboundedSender<ServerMessage>>>,
    pub session_secrets: RwLock<HashMap<PlayerId, SessionSecret>>,
    pub removed_pieces: RwLock<Vec<PieceId>>,
    pub removed_players: RwLock<Vec<PlayerId>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<TimestampMs>,
    pub death_timestamps: RwLock<HashMap<PlayerId, TimestampMs>>,
}

impl GameInstance {
    /// Creates a new game instance for a given mode.
    ///
    /// `mode_config` defines the rules, `piece_configs` and `shop_configs` provide assets.
    /// Returns a fully initialized `GameInstance`.
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
            connection_channels: RwLock::new(HashMap::new()),
            player_channels: RwLock::new(HashMap::new()),
            session_secrets: RwLock::new(HashMap::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            color_manager: RwLock::new(ColorManager::new()),
            last_viewed_at: RwLock::new(now_ms()),
            death_timestamps: RwLock::new(HashMap::new()),
        }
    }

    /// Broadcasts a server message to all active players and connections.
    ///
    /// `msg` is cloned per recipient. Returns nothing.
    pub async fn broadcast(&self, msg: ServerMessage) {
        let player_channels = self.player_channels.read().await;
        let connection_channels = self.connection_channels.read().await;
        for tx in player_channels.values().chain(connection_channels.values()) {
            let _ = tx.send(msg.clone());
        }
    }

    /// Records a piece identifier for removal in the next update.
    ///
    /// `piece_id` is the piece to remove. Returns nothing.
    pub async fn record_piece_removal(&self, piece_id: PieceId) {
        self.removed_pieces.write().await.push(piece_id);
    }

    /// Removes pieces and shops that drift outside the current board bounds.
    ///
    /// `game` is the mutable game state to prune. Returns nothing.
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
