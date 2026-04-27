//! Shared per-mode state container for live game instances.

use super::hooks::HookEventBuffer;
use crate::colors::ColorManager;
use crate::time::now_ms;
use crate::types::ConnectionId;
use common::models::{GameModeConfig, GameState, PieceConfig, QueuePresetLayoutConfig, ShopConfig};
use common::protocol::{GameError, ServerMessage, VictoryFocusTarget};
use common::types::{
    BoardCoord, ModeId, PieceId, PieceTypeId, PlayerCount, PlayerId, SessionSecret, ShopId,
    TimestampMs,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// Queued move request stored server-side for cooldown chaining.
#[derive(Clone, Copy)]
pub(super) struct QueuedMoveRequest {
    pub player_id: PlayerId,
    pub target: BoardCoord,
}

/// Live game instance state for a single mode.
pub struct GameInstance {
    pub(super) mode_config: GameModeConfig,
    pub(super) public_mode_id: ModeId,
    pub(super) queue_layout: Option<Arc<QueuePresetLayoutConfig>>,
    pub(super) piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
    pub(super) shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    pub game: RwLock<GameState>,
    pub connection_channels: RwLock<HashMap<ConnectionId, mpsc::Sender<ServerMessage>>>,
    pub player_channels: RwLock<HashMap<PlayerId, mpsc::Sender<ServerMessage>>>,
    pub session_secrets: RwLock<HashMap<PlayerId, (SessionSecret, TimestampMs)>>,
    pub victory_players: RwLock<HashSet<PlayerId>>,
    pub removed_pieces: RwLock<Vec<PieceId>>,
    pub removed_players: RwLock<Vec<PlayerId>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<TimestampMs>,
    pub last_started_at: RwLock<TimestampMs>,
    pub move_unlock_at: RwLock<Option<TimestampMs>>,
    pub death_timestamps: RwLock<HashMap<PlayerId, TimestampMs>>,
    pub(super) hook_events: RwLock<HookEventBuffer>,
    pub(super) queued_moves: RwLock<HashMap<PieceId, VecDeque<QueuedMoveRequest>>>,
}

impl GameInstance {
    /// Returns the public mode identifier for this instance.
    pub fn public_mode_id(&self) -> &ModeId {
        &self.public_mode_id
    }

    /// Returns the internal mode identifier for this instance.
    pub fn mode_id(&self) -> &ModeId {
        &self.mode_config.id
    }

    /// Returns the display name of this mode instance.
    pub fn mode_display_name(&self) -> &str {
        &self.mode_config.display_name
    }

    /// Returns the max player count configured for this mode.
    pub fn max_players(&self) -> PlayerCount {
        self.mode_config.max_players
    }

    /// Returns the respawn cooldown configured for this mode.
    pub fn respawn_cooldown_ms(&self) -> common::types::DurationMs {
        self.mode_config.respawn_cooldown_ms
    }

    /// Returns the current number of in-game players in this instance.
    pub async fn player_count(&self) -> PlayerCount {
        let game = self.game.read().await;
        let victory_players = self.victory_players.read().await;
        let count = game
            .players
            .keys()
            .filter(|player_id| !victory_players.contains(player_id))
            .count();
        PlayerCount::new(count as u32)
    }

    /// Returns the client-safe mode configuration for this instance.
    pub fn client_mode_config(&self) -> common::models::GameModeClientConfig {
        self.mode_config.to_client_config()
    }

    /// Returns the last time this instance transitioned from empty to active.
    pub async fn last_started_at(&self) -> TimestampMs {
        *self.last_started_at.read().await
    }

    /// Returns the timestamp when player move execution is unlocked.
    pub async fn move_unlock_at(&self) -> Option<TimestampMs> {
        *self.move_unlock_at.read().await
    }

    /// Starts a queue countdown if configured for this mode instance.
    pub async fn start_queue_countdown(&self) {
        let countdown = self.mode_config.queue_countdown_ms;
        if countdown <= common::types::DurationMs::zero() {
            *self.move_unlock_at.write().await = None;
            return;
        }
        *self.move_unlock_at.write().await = Some(now_ms() + countdown);
    }

    /// Registers a passive connection channel for lobby/observer updates.
    pub async fn add_connection_channel(
        &self,
        conn_id: ConnectionId,
        tx: mpsc::Sender<ServerMessage>,
    ) {
        self.connection_channels.write().await.insert(conn_id, tx);
    }

    /// Removes a passive connection channel.
    pub async fn remove_connection_channel(&self, conn_id: ConnectionId) {
        self.connection_channels.write().await.remove(&conn_id);
    }

    /// Returns true when the player is already active in this instance.
    pub async fn has_active_player_session(&self, player_id: PlayerId) -> bool {
        let channels = self.player_channels.read().await;
        if !channels.contains_key(&player_id) {
            return false;
        }
        let game = self.game.read().await;
        game.players.contains_key(&player_id)
    }

    /// Returns true when no players or viewers remain attached to this instance.
    pub async fn is_empty(&self) -> bool {
        let has_players = !self.game.read().await.players.is_empty();
        let has_player_channels = !self.player_channels.read().await.is_empty();
        let has_viewers = !self.connection_channels.read().await.is_empty();
        !has_players && !has_player_channels && !has_viewers
    }

    /// Returns a snapshot of piece configurations for client initialization.
    pub fn piece_config_snapshot(&self) -> HashMap<PieceTypeId, PieceConfig> {
        (*self.piece_configs).clone()
    }

    /// Returns a snapshot of shop configurations for client initialization.
    pub fn shop_config_snapshot(&self) -> HashMap<ShopId, ShopConfig> {
        (*self.shop_configs).clone()
    }

    /// Sends a fatal custom message to a specific player.
    ///
    /// `player_id` is the recipient and `title`/`message` are shown on the client.
    pub async fn send_custom_to_player(&self, player_id: PlayerId, title: String, message: String) {
        let channels = self.player_channels.read().await;
        if let Some(tx) = channels.get(&player_id) {
            let _ = tx.try_send(ServerMessage::Error(GameError::Custom { title, message }));
        }
    }

    /// Sends a non-fatal victory payload to a specific player.
    pub async fn send_victory_to_player(
        &self,
        player_id: PlayerId,
        title: String,
        message: String,
        focus_target: VictoryFocusTarget,
    ) {
        self.victory_players.write().await.insert(player_id);
        let channels = self.player_channels.read().await;
        if let Some(tx) = channels.get(&player_id) {
            let _ = tx.try_send(ServerMessage::Victory {
                title,
                message,
                focus_target,
            });
        }
    }

    /// Creates a new game instance for a given mode.
    ///
    /// `mode_config` defines the rules, `public_mode_id` is the shared mode identifier, and
    /// `piece_configs`/`shop_configs` provide assets.
    /// Returns a fully initialized `GameInstance`.
    pub fn new(
        mode_config: GameModeConfig,
        public_mode_id: ModeId,
        queue_layout: Option<Arc<QueuePresetLayoutConfig>>,
        piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
        shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    ) -> Self {
        let board_size = common::logic::calculate_board_size(&mode_config, 0);
        let now = now_ms();
        Self {
            mode_config: mode_config.clone(),
            public_mode_id,
            queue_layout,
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
            victory_players: RwLock::new(HashSet::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            color_manager: RwLock::new(ColorManager::new()),
            last_viewed_at: RwLock::new(now),
            last_started_at: RwLock::new(now),
            move_unlock_at: RwLock::new(None),
            death_timestamps: RwLock::new(HashMap::new()),
            hook_events: RwLock::new(HookEventBuffer::default()),
            queued_moves: RwLock::new(HashMap::new()),
        }
    }

    /// Broadcasts a server message to all active players and connections.
    ///
    /// `msg` is cloned per recipient. Returns nothing.
    pub async fn broadcast(&self, msg: ServerMessage) {
        let mut to_remove_players = Vec::new();
        {
            let mut player_channels = self.player_channels.write().await;
            player_channels.retain(|id, tx| {
                if tx.try_send(msg.clone()).is_err() {
                    to_remove_players.push(*id);
                    false
                } else {
                    true
                }
            });
        }

        for player_id in to_remove_players {
            self.remove_player_only_state(player_id).await;
        }

        {
            let mut connection_channels = self.connection_channels.write().await;
            connection_channels.retain(|_, tx| tx.try_send(msg.clone()).is_ok());
        }
    }

    /// Records a piece identifier for removal in the next update.
    ///
    /// `piece_id` is the piece to remove. Returns nothing.
    pub async fn record_piece_removal(&self, piece_id: PieceId) {
        self.removed_pieces.write().await.push(piece_id);
    }

    /// Clears any queued premoves referencing pieces that were removed from the instance.
    pub(super) async fn clear_queued_moves_for_pieces<I>(&self, piece_ids: I)
    where
        I: IntoIterator<Item = PieceId>,
    {
        let mut queued_moves = self.queued_moves.write().await;
        for piece_id in piece_ids {
            queued_moves.remove(&piece_id);
        }
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
