//! Tick-batched hook event buffering and resolution.

use super::GameInstance;
use crate::time::now_ms;
use common::models::{GameState, HookConfig, HookVictoryFocus, SupportedHook};
use common::protocol::{ChatLine, VictoryFocusTarget};
use common::types::{BoardCoord, ColorHex, PieceTypeId, PlayerId};

const SYSTEM_CHAT_NAME: &str = "System";
const SYSTEM_CHAT_COLOR: &str = "#facc15";

/// A captured piece event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct CaptureEvent {
    capturer_id: Option<PlayerId>,
    captured_piece_type: PieceTypeId,
    captured_owner_id: Option<PlayerId>,
    captured_position: BoardCoord,
}

/// A player-join event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct PlayerJoinEvent {
    player_name: String,
}

/// A player-disconnect event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct PlayerDisconnectEvent {
    player_name: String,
}

/// A player-killed event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct PlayerKilledEvent {
    player_name: String,
    killer_name: Option<String>,
}

/// A queue-countdown event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct QueueCountdownEvent {
    seconds: u32,
}

/// Gameplay events observed across one tick.
#[derive(Debug, Default)]
struct TickHookEvents {
    captures: Vec<CaptureEvent>,
    player_left: bool,
    player_joins: Vec<PlayerJoinEvent>,
    player_disconnects: Vec<PlayerDisconnectEvent>,
    player_kills: Vec<PlayerKilledEvent>,
    queue_countdowns: Vec<QueueCountdownEvent>,
    game_started: bool,
}

impl TickHookEvents {
    /// Returns whether the tick observed no hook-relevant events.
    fn is_empty(&self) -> bool {
        self.captures.is_empty()
            && !self.player_left
            && self.player_joins.is_empty()
            && self.player_disconnects.is_empty()
            && self.player_kills.is_empty()
            && self.queue_countdowns.is_empty()
            && !self.game_started
    }

    /// Records a capture event.
    fn record_capture(
        &mut self,
        capturer_id: Option<PlayerId>,
        captured_piece_type: PieceTypeId,
        captured_owner_id: Option<PlayerId>,
        captured_position: BoardCoord,
    ) {
        self.captures.push(CaptureEvent {
            capturer_id,
            captured_piece_type,
            captured_owner_id,
            captured_position,
        });
    }

    /// Records that at least one player left during the tick.
    fn record_player_leave(&mut self) {
        self.player_left = true;
    }

    /// Records one player join event.
    fn record_player_join(&mut self, player_name: String) {
        self.player_joins.push(PlayerJoinEvent { player_name });
    }

    /// Records one player disconnect event.
    fn record_player_disconnect(&mut self, player_name: String) {
        self.player_disconnects
            .push(PlayerDisconnectEvent { player_name });
    }

    /// Records one player killed event.
    fn record_player_killed(&mut self, player_name: String, killer_name: Option<String>) {
        self.player_kills.push(PlayerKilledEvent {
            player_name,
            killer_name,
        });
    }

    /// Records one queue countdown tick event.
    fn record_queue_countdown(&mut self, seconds: u32) {
        self.queue_countdowns.push(QueueCountdownEvent { seconds });
    }

    /// Records a game start event.
    fn record_game_start(&mut self) {
        self.game_started = true;
    }

    /// Returns player ids whose owned pieces were captured by hooks matching `hook`.
    fn captured_players<'a>(&'a self, hook: &'a HookConfig) -> impl Iterator<Item = PlayerId> + 'a {
        self.captures.iter().filter_map(move |event| {
            hook.targets_piece(&event.captured_piece_type)
                .then_some(event.captured_owner_id)
                .flatten()
        })
    }

    /// Returns the first capture event matching `hook`.
    fn winning_capture_event(&self, hook: &HookConfig) -> Option<&CaptureEvent> {
        self.captures
            .iter()
            .find(|event| hook.targets_piece(&event.captured_piece_type))
    }
}

/// Double-buffered hook queue spanning tick boundaries.
#[derive(Debug, Default)]
pub(super) struct HookEventBuffer {
    current_tick: TickHookEvents,
    queued: TickHookEvents,
    tick_in_progress: bool,
}

impl HookEventBuffer {
    /// Starts a new tick by promoting any queued events into the active buffer.
    fn begin_tick(&mut self) {
        self.current_tick = std::mem::take(&mut self.queued);
        self.tick_in_progress = true;
    }

    /// Finishes the active tick and returns its drained events.
    fn finish_tick(&mut self) -> TickHookEvents {
        self.tick_in_progress = false;
        std::mem::take(&mut self.current_tick)
    }

    /// Records a capture into the active tick or the next queued tick.
    fn record_capture(
        &mut self,
        capturer_id: Option<PlayerId>,
        captured_piece_type: PieceTypeId,
        captured_owner_id: Option<PlayerId>,
        captured_position: BoardCoord,
    ) {
        self.target_buffer().record_capture(
            capturer_id,
            captured_piece_type,
            captured_owner_id,
            captured_position,
        );
    }

    /// Records that a player left into the active tick or the next queued tick.
    fn record_player_leave(&mut self) {
        self.target_buffer().record_player_leave();
    }

    /// Records a player join into the active tick or the next queued tick.
    fn record_player_join(&mut self, player_name: String) {
        self.target_buffer().record_player_join(player_name);
    }

    /// Records a player disconnect into the active tick or the next queued tick.
    fn record_player_disconnect(&mut self, player_name: String) {
        self.target_buffer().record_player_disconnect(player_name);
    }

    /// Records a player killed event into the active tick or the next queued tick.
    fn record_player_killed(&mut self, player_name: String, killer_name: Option<String>) {
        self.target_buffer()
            .record_player_killed(player_name, killer_name);
    }

    /// Records a queue countdown tick into the active tick or the next queued tick.
    fn record_queue_countdown(&mut self, seconds: u32) {
        self.target_buffer().record_queue_countdown(seconds);
    }

    /// Records a game start event into the active tick or the next queued tick.
    fn record_game_start(&mut self) {
        self.target_buffer().record_game_start();
    }

    /// Returns the buffer currently receiving events.
    fn target_buffer(&mut self) -> &mut TickHookEvents {
        if self.tick_in_progress {
            &mut self.current_tick
        } else {
            &mut self.queued
        }
    }
}

/// A user-facing victory message selected by hook evaluation.
#[derive(Debug)]
struct VictoryMessage {
    player_id: PlayerId,
    title: String,
    message: String,
    focus_target: VictoryFocusTarget,
}

impl VictoryMessage {
    /// Builds the default capture victory message for `player_id`.
    fn capturer(hook: &HookConfig, event: &CaptureEvent) -> Option<Self> {
        let player_id = event.capturer_id?;
        let focus_target =
            match hook.victory_focus_or_default(SupportedHook::WinCapturerOnActiveCapture) {
                HookVictoryFocus::CaptureSquare => {
                    VictoryFocusTarget::BoardPosition(event.captured_position)
                }
                HookVictoryFocus::KeepCurrent => VictoryFocusTarget::KeepCurrent,
            };
        Some(Self {
            player_id,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("You won by capturing the enemy king."),
            focus_target,
        })
    }

    /// Builds the default remaining-player victory message for `player_id`.
    fn remaining_player(hook: &HookConfig, player_id: PlayerId) -> Self {
        let focus_target =
            match hook.victory_focus_or_default(SupportedHook::WinRemainingOnPlayerLeave) {
                HookVictoryFocus::KeepCurrent | HookVictoryFocus::CaptureSquare => {
                    VictoryFocusTarget::KeepCurrent
                }
            };
        Self {
            player_id,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("Opponent disconnected. You win."),
            focus_target,
        }
    }
}

/// Hook message template variables for chat-system events.
#[derive(Debug, Clone, Default)]
struct HookMessageContext {
    player_name: Option<String>,
    killer_name: Option<String>,
    seconds: Option<u32>,
}

impl HookMessageContext {
    fn render_message(&self, template: &str) -> String {
        let mut rendered = template.to_string();
        if let Some(player_name) = &self.player_name {
            rendered = rendered.replace("{player}", player_name);
        }
        if let Some(killer_name) = &self.killer_name {
            rendered = rendered.replace("{killer}", killer_name);
        }
        if let Some(seconds) = self.seconds {
            rendered = rendered.replace("{seconds}", &seconds.to_string());
        }
        rendered
    }
}

/// Result of applying one hook entry.
#[derive(Debug, Default)]
struct HookApplyResult {
    matched: bool,
    victory: Option<VictoryMessage>,
}

impl GameInstance {
    /// Records a capture event for hook processing at the end of the current tick.
    pub(super) async fn record_capture_event(
        &self,
        capturer_id: Option<PlayerId>,
        captured_piece_type: PieceTypeId,
        captured_owner_id: Option<PlayerId>,
        captured_position: BoardCoord,
    ) {
        self.hook_events.write().await.record_capture(
            capturer_id,
            captured_piece_type,
            captured_owner_id,
            captured_position,
        );
    }

    /// Records that a player left during the current or next tick window.
    pub(super) async fn record_player_leave_event(&self) {
        self.hook_events.write().await.record_player_leave();
    }

    /// Records one player join event for hook processing.
    pub(super) async fn record_player_join_event(&self, player_name: String) {
        self.hook_events
            .write()
            .await
            .record_player_join(player_name);
    }

    /// Records one player disconnect event for hook processing.
    pub(crate) async fn record_player_disconnect_event(&self, player_name: String) {
        self.hook_events
            .write()
            .await
            .record_player_disconnect(player_name);
    }

    /// Records one player killed event for hook processing.
    pub(super) async fn record_player_killed_event(
        &self,
        player_name: String,
        killer_name: Option<String>,
    ) {
        self.hook_events
            .write()
            .await
            .record_player_killed(player_name, killer_name);
    }

    /// Records one queue countdown event for hook processing.
    pub(super) async fn record_queue_countdown_event(&self, seconds: u32) {
        self.hook_events
            .write()
            .await
            .record_queue_countdown(seconds);
    }

    /// Records a game start event for hook processing.
    pub(super) async fn record_game_start_event(&self) {
        self.hook_events.write().await.record_game_start();
    }

    /// Starts collecting hook events for the current tick.
    pub(super) async fn start_tick_hooks(&self) {
        self.hook_events.write().await.begin_tick();
    }

    /// Resolves all queued hook effects for the completed tick.
    pub(super) async fn resolve_tick_hooks(&self) {
        let hook_events = self.hook_events.write().await.finish_tick();
        if hook_events.is_empty() {
            return;
        }

        let mut victory_message = None;

        {
            let mut game = self.game.write().await;
            for hook in &self.mode_config.hooks {
                let apply_result = self.apply_tick_hook(hook, &hook_events, &mut game).await;
                if victory_message.is_none() {
                    victory_message = apply_result.victory;
                }
                if apply_result.matched && hook.capture {
                    break;
                }
            }
        }

        if let Some(message) = victory_message {
            self.send_victory_to_player(
                message.player_id,
                message.title,
                message.message,
                message.focus_target,
            )
            .await;
        }
    }

    /// Applies one configured hook to the completed tick's events.
    async fn apply_tick_hook(
        &self,
        hook: &HookConfig,
        hook_events: &TickHookEvents,
        game: &mut GameState,
    ) -> HookApplyResult {
        match hook.supported_hook() {
            Some(SupportedHook::EliminateOwnerOnCapture) => HookApplyResult {
                matched: self
                    .apply_eliminate_owner_hook(hook, hook_events, game)
                    .await,
                victory: None,
            },
            Some(SupportedHook::WinCapturerOnActiveCapture) => HookApplyResult {
                matched: hook_events.winning_capture_event(hook).is_some(),
                victory: hook_events
                    .winning_capture_event(hook)
                    .and_then(|event| VictoryMessage::capturer(hook, event)),
            },
            Some(SupportedHook::WinRemainingOnPlayerLeave) => {
                let victory = self.find_remaining_player_victory(hook, hook_events, game);
                HookApplyResult {
                    matched: victory.is_some(),
                    victory,
                }
            }
            Some(SupportedHook::SystemChatOnPlayerJoin) => HookApplyResult {
                matched: self
                    .emit_system_chat_for_joins(hook, &hook_events.player_joins)
                    .await,
                victory: None,
            },
            Some(SupportedHook::SystemChatOnPlayerDisconnect) => HookApplyResult {
                matched: self
                    .emit_system_chat_for_disconnects(hook, &hook_events.player_disconnects)
                    .await,
                victory: None,
            },
            Some(SupportedHook::SystemChatOnPlayerKilled) => HookApplyResult {
                matched: self
                    .emit_system_chat_for_kills(hook, &hook_events.player_kills)
                    .await,
                victory: None,
            },
            Some(SupportedHook::SystemChatOnQueueCountdown) => HookApplyResult {
                matched: self
                    .emit_system_chat_for_countdown(hook, &hook_events.queue_countdowns)
                    .await,
                victory: None,
            },
            Some(SupportedHook::SystemChatOnGameStart) => HookApplyResult {
                matched: self
                    .emit_system_chat_for_game_start(hook, hook_events.game_started)
                    .await,
                victory: None,
            },
            _ => HookApplyResult::default(),
        }
    }

    /// Eliminates players whose matching pieces were captured this tick.
    async fn apply_eliminate_owner_hook(
        &self,
        hook: &HookConfig,
        hook_events: &TickHookEvents,
        game: &mut GameState,
    ) -> bool {
        let mut matched = false;
        let mut all_removed_piece_ids = Vec::new();
        for player_id in hook_events.captured_players(hook) {
            if game.players.contains_key(&player_id) {
                let (removed, piece_ids) = self.remove_player_state(player_id, game).await;
                if removed {
                    matched = true;
                    // Elimination-by-capture is not a disconnect/leave event.
                    // Keeping this out of OnPlayerLeave avoids delayed "opponent disconnected" wins.
                    all_removed_piece_ids.extend(piece_ids);
                }
            }
        }

        if !all_removed_piece_ids.is_empty() {
            self.clear_queued_moves_for_pieces(all_removed_piece_ids)
                .await;
        }

        matched
    }

    /// Returns the remaining-player victory, if this tick's leave events satisfy `hook`.
    fn find_remaining_player_victory(
        &self,
        hook: &HookConfig,
        hook_events: &TickHookEvents,
        game: &GameState,
    ) -> Option<VictoryMessage> {
        if !hook_events.player_left {
            return None;
        }

        let players_left = game.players.len() as u32;
        if let Some(required) = hook.players_left
            && players_left != required
        {
            return None;
        }

        let (&player_id, _) = game.players.iter().next()?;
        (players_left == 1).then(|| VictoryMessage::remaining_player(hook, player_id))
    }

    async fn emit_system_chat_for_joins(
        &self,
        hook: &HookConfig,
        events: &[PlayerJoinEvent],
    ) -> bool {
        let mut sent = false;
        let template = hook
            .message
            .clone()
            .unwrap_or_else(|| "{player} joined the game".to_string());
        for event in events {
            let context = HookMessageContext {
                player_name: Some(event.player_name.clone()),
                ..Default::default()
            };
            self.emit_system_chat_message(context.render_message(&template))
                .await;
            sent = true;
        }
        sent
    }

    async fn emit_system_chat_for_disconnects(
        &self,
        hook: &HookConfig,
        events: &[PlayerDisconnectEvent],
    ) -> bool {
        let mut sent = false;
        let template = hook
            .message
            .clone()
            .unwrap_or_else(|| "{player} disconnected".to_string());
        for event in events {
            let context = HookMessageContext {
                player_name: Some(event.player_name.clone()),
                ..Default::default()
            };
            self.emit_system_chat_message(context.render_message(&template))
                .await;
            sent = true;
        }
        sent
    }

    async fn emit_system_chat_for_kills(
        &self,
        hook: &HookConfig,
        events: &[PlayerKilledEvent],
    ) -> bool {
        let mut sent = false;
        let template = hook
            .message
            .clone()
            .unwrap_or_else(|| "{player} was killed by {killer}".to_string());
        for event in events {
            let context = HookMessageContext {
                player_name: Some(event.player_name.clone()),
                killer_name: Some(
                    event
                        .killer_name
                        .clone()
                        .unwrap_or_else(|| "NPC".to_string()),
                ),
                ..Default::default()
            };
            self.emit_system_chat_message(context.render_message(&template))
                .await;
            sent = true;
        }
        sent
    }

    async fn emit_system_chat_for_countdown(
        &self,
        hook: &HookConfig,
        events: &[QueueCountdownEvent],
    ) -> bool {
        let mut sent = false;
        let template = hook
            .message
            .clone()
            .unwrap_or_else(|| "Match starts in {seconds}".to_string());
        for event in events {
            let context = HookMessageContext {
                seconds: Some(event.seconds),
                ..Default::default()
            };
            self.emit_system_chat_message(context.render_message(&template))
                .await;
            sent = true;
        }
        sent
    }

    async fn emit_system_chat_for_game_start(&self, hook: &HookConfig, started: bool) -> bool {
        if !started {
            return false;
        }
        let template = hook
            .message
            .clone()
            .unwrap_or_else(|| "Game started!".to_string());
        self.emit_system_chat_message(template).await;
        true
    }

    async fn emit_system_chat_message(&self, message: String) {
        let line = ChatLine {
            sender_name: SYSTEM_CHAT_NAME.to_string(),
            sender_color: ColorHex::from(SYSTEM_CHAT_COLOR),
            message,
            is_system: true,
            sent_at: now_ms(),
        };
        self.push_chat_line(line.clone()).await;
        self.broadcast(common::protocol::ServerMessage::Chat { line })
            .await;
    }
}
