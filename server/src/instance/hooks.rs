//! Tick-batched hook event buffering and resolution.

use super::GameInstance;
use common::models::{GameState, HookConfig, HookVictoryFocus, SupportedHook};
use common::protocol::VictoryFocusTarget;
use common::types::{BoardCoord, PieceTypeId, PlayerId};

/// A captured piece event deferred until the end of a tick.
#[derive(Debug, Clone)]
struct CaptureEvent {
    capturer_id: Option<PlayerId>,
    captured_piece_type: PieceTypeId,
    captured_owner_id: Option<PlayerId>,
    captured_position: BoardCoord,
}

/// Gameplay events observed across one tick.
#[derive(Debug, Default)]
struct TickHookEvents {
    captures: Vec<CaptureEvent>,
    player_left: bool,
}

impl TickHookEvents {
    /// Returns whether the tick observed no hook-relevant events.
    fn is_empty(&self) -> bool {
        self.captures.is_empty() && !self.player_left
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
                if let Some(new_message) = self.apply_hook(hook, &hook_events, &mut game).await
                    && victory_message.is_none()
                {
                    victory_message = Some(new_message);
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
    async fn apply_hook(
        &self,
        hook: &HookConfig,
        hook_events: &TickHookEvents,
        game: &mut GameState,
    ) -> Option<VictoryMessage> {
        match hook.supported_hook() {
            Some(SupportedHook::EliminateOwnerOnCapture) => {
                self.apply_eliminate_owner_hook(hook, hook_events, game)
                    .await;
                None
            }
            Some(SupportedHook::WinCapturerOnActiveCapture) => hook_events
                .winning_capture_event(hook)
                .and_then(|event| VictoryMessage::capturer(hook, event)),
            Some(SupportedHook::WinRemainingOnPlayerLeave) => {
                self.find_remaining_player_victory(hook, hook_events, game)
            }
            _ => None,
        }
    }

    /// Eliminates players whose matching pieces were captured this tick.
    async fn apply_eliminate_owner_hook(
        &self,
        hook: &HookConfig,
        hook_events: &TickHookEvents,
        game: &mut GameState,
    ) {
        let mut all_removed_piece_ids = Vec::new();
        for player_id in hook_events.captured_players(hook) {
            if game.players.contains_key(&player_id) {
                let (removed, piece_ids) = self.remove_player_state(player_id, game).await;
                if removed {
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
}
