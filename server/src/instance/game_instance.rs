//! Shared per-mode state container for live game instances.

use crate::colors::ColorManager;
use crate::time::now_ms;
use crate::types::ConnectionId;
use common::models::{GameModeConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::{GameError, ServerMessage};
use common::types::{PieceId, PieceTypeId, PlayerId, SessionSecret, ShopId, TimestampMs};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

#[derive(Debug, Clone)]
pub struct CaptureHookEvent {
    pub capturer_id: Option<PlayerId>,
    pub captured_piece_type: PieceTypeId,
    pub captured_owner_id: Option<PlayerId>,
}

#[derive(Debug, Default)]
pub struct HookEventBuffer {
    pub captures: Vec<CaptureHookEvent>,
    pub player_leaves: Vec<PlayerId>,
}

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
    pub hook_events: RwLock<HookEventBuffer>,
}

impl GameInstance {
    /// Sends a fatal custom message to a specific player.
    ///
    /// `player_id` is the recipient and `title`/`message` are shown on the client.
    pub async fn send_custom_to_player(&self, player_id: PlayerId, title: String, message: String) {
        let channels = self.player_channels.read().await;
        if let Some(tx) = channels.get(&player_id) {
            let _ = tx.send(ServerMessage::Error(GameError::Custom { title, message }));
        }
    }

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
            hook_events: RwLock::new(HookEventBuffer::default()),
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

    /// Records a capture event for hook processing at the end of the next tick.
    pub async fn record_capture_event(
        &self,
        capturer_id: Option<PlayerId>,
        captured_piece_type: PieceTypeId,
        captured_owner_id: Option<PlayerId>,
    ) {
        self.hook_events
            .write()
            .await
            .captures
            .push(CaptureHookEvent {
                capturer_id,
                captured_piece_type,
                captured_owner_id,
            });
    }

    /// Records that a player left during the current tick window.
    pub async fn record_player_leave_event(&self, player_id: PlayerId) {
        self.hook_events.write().await.player_leaves.push(player_id);
    }

    /// Drains queued hook events for processing in the current tick.
    pub async fn take_hook_events(&self) -> HookEventBuffer {
        let mut hook_events = self.hook_events.write().await;
        std::mem::take(&mut *hook_events)
    }

    /// Applies queued hook events in config order at the end of a tick.
    pub async fn process_hook_events(&self, hook_events: HookEventBuffer) {
        if hook_events.captures.is_empty() && hook_events.player_leaves.is_empty() {
            return;
        }

        let mut game = self.game.write().await;
        let mut had_player_leave = !hook_events.player_leaves.is_empty();
        let mut winner_message: Option<(PlayerId, String, String)> = None;

        for hook in &self.mode_config.hooks {
            if hook.trigger == "OnCapture" && hook.action == "EliminateOwner" {
                let eliminated_players: Vec<PlayerId> = hook_events
                    .captures
                    .iter()
                    .filter(|event| {
                        hook.target_piece_id.as_ref() == Some(&event.captured_piece_type)
                    })
                    .filter_map(|event| event.captured_owner_id)
                    .collect();

                for player_id in eliminated_players {
                    if game.players.contains_key(&player_id) {
                        self.record_player_removal(player_id, &mut game).await;
                        game.players.remove(&player_id);
                        had_player_leave = true;
                    }
                }
                continue;
            }

            if hook.trigger == "OnCapturePieceActive"
                && hook.action == "WinCapturer"
                && winner_message.is_none()
            {
                if let Some(event) = hook_events.captures.iter().find(|event| {
                    event.capturer_id.is_some()
                        && hook.target_piece_id.as_ref().is_none_or(|target_piece_id| {
                            target_piece_id == &event.captured_piece_type
                        })
                }) && let Some(capturer_id) = event.capturer_id
                {
                    let title = hook.title.clone().unwrap_or_else(|| "VICTORY".to_string());
                    let message = hook
                        .message
                        .clone()
                        .unwrap_or_else(|| "You won by capturing the enemy king.".to_string());
                    winner_message = Some((capturer_id, title, message));
                }
                continue;
            }

            if hook.trigger == "OnPlayerLeave"
                && hook.action == "WinRemaining"
                && had_player_leave
                && winner_message.is_none()
            {
                let players_left = game.players.len() as u32;
                if let Some(required) = hook.players_left
                    && players_left != required
                {
                    continue;
                }
                if players_left != 1 {
                    continue;
                }
                if let Some((&winner_id, _)) = game.players.iter().next() {
                    let title = hook.title.clone().unwrap_or_else(|| "VICTORY".to_string());
                    let message = hook
                        .message
                        .clone()
                        .unwrap_or_else(|| "Opponent disconnected. You win.".to_string());
                    winner_message = Some((winner_id, title, message));
                }
            }
        }

        drop(game);

        if let Some((winner_id, title, message)) = winner_message {
            self.send_custom_to_player(winner_id, title, message).await;
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
