//! Configured hooks reacting to gameplay events once per tick.

mod events;

pub(super) use events::HookEvents;

use super::players::Departure;
use super::world::World;
use common::models::{CameraCue, ChatEvent, GameState, HookConfig, Piece, SupportedHook};
use common::protocol::{ChatLine, ServerMessage};
use common::types::{ColorHex, PlayerId, TimestampMs};
use events::{CaptureEvent, KillEvent};

/// Sender name of system chat lines.
const SYSTEM_CHAT_NAME: &str = "System";
/// Sender color of system chat lines.
const SYSTEM_CHAT_COLOR: &str = "#facc15";

/// Returns the chat line announcing `event` when its hook configures no message.
fn default_chat_template(event: ChatEvent) -> &'static str {
    match event {
        ChatEvent::PlayerJoin => "{player} joined the game",
        ChatEvent::PlayerDisconnect => "{player} disconnected",
        ChatEvent::PlayerKilled => "{player} was killed by {killer}",
        ChatEvent::QueueCountdown => "Match starts in {seconds}",
        ChatEvent::GameStart => "Game started!",
        ChatEvent::SpectatorJoin => "{player} is spectating",
        ChatEvent::SpectatorLeave => "{player} stopped spectating",
    }
}

/// A user-facing victory message selected by hook evaluation.
#[derive(Debug)]
struct VictoryMessage {
    player_id: PlayerId,
    title: String,
    message: String,
    /// Camera move made for the winner along with the victory.
    camera_cue: Option<CameraCue>,
}

impl VictoryMessage {
    /// Builds the victory of the player who made a winning capture.
    fn capturer(hook: &HookConfig, event: &CaptureEvent) -> Option<Self> {
        Some(Self {
            player_id: event.capturer_id?,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("You won by capturing the enemy king."),
            camera_cue: hook.camera_cue(event.captured_position),
        })
    }

    /// Builds the victory of the last player left in the match.
    fn remaining_player(hook: &HookConfig, player_id: PlayerId) -> Self {
        Self {
            player_id,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("Opponent disconnected. You win."),
            camera_cue: None,
        }
    }
}

/// Template variables for system chat hook messages.
#[derive(Debug, Default)]
struct MessageContext<'a> {
    player: Option<&'a str>,
    killer: Option<&'a str>,
    seconds: Option<u32>,
}

impl<'a> MessageContext<'a> {
    /// Returns one context naming each of `names` as `{player}`.
    fn players(names: &'a [String]) -> impl Iterator<Item = Self> {
        names.iter().map(|name| Self {
            player: Some(name),
            ..Self::default()
        })
    }

    /// Returns one context per occurrence of `event` among `events`.
    fn of_event(event: ChatEvent, events: &'a HookEvents) -> Vec<Self> {
        match event {
            ChatEvent::PlayerJoin => Self::players(&events.joins).collect(),
            ChatEvent::PlayerDisconnect => Self::players(&events.disconnects).collect(),
            ChatEvent::PlayerKilled => events
                .kills
                .iter()
                .map(|kill| Self {
                    player: Some(&kill.victim_name),
                    killer: Some(kill.killer_name.as_deref().unwrap_or("NPC")),
                    ..Self::default()
                })
                .collect(),
            ChatEvent::QueueCountdown => events
                .countdowns
                .iter()
                .map(|&seconds| Self {
                    seconds: Some(seconds),
                    ..Self::default()
                })
                .collect(),
            ChatEvent::GameStart => events
                .game_started
                .then(Self::default)
                .into_iter()
                .collect(),
            ChatEvent::SpectatorJoin => Self::players(&events.spectator_joins).collect(),
            ChatEvent::SpectatorLeave => Self::players(&events.spectator_leaves).collect(),
        }
    }

    /// Substitutes `{player}`, `{killer}` and `{seconds}` in `template`.
    fn render(&self, template: &str) -> String {
        let mut rendered = template.to_string();
        if let Some(player) = self.player {
            rendered = rendered.replace("{player}", player);
        }
        if let Some(killer) = self.killer {
            rendered = rendered.replace("{killer}", killer);
        }
        if let Some(seconds) = self.seconds {
            rendered = rendered.replace("{seconds}", &seconds.to_string());
        }
        rendered
    }
}

/// Outcome of applying one hook entry.
#[derive(Debug, Default)]
struct HookOutcome {
    matched: bool,
    victory: Option<VictoryMessage>,
}

impl HookOutcome {
    /// An outcome that matched without producing a victory.
    fn matched(matched: bool) -> Self {
        Self {
            matched,
            victory: None,
        }
    }

    /// An outcome that matched exactly when it produced a victory.
    fn victory(victory: Option<VictoryMessage>) -> Self {
        Self {
            matched: victory.is_some(),
            victory,
        }
    }
}

impl World {
    /// Records a capture, plus a kill when a player's king fell.
    pub(super) fn record_capture(&mut self, capturer_id: Option<PlayerId>, captured: &Piece) {
        self.hooks.captures.push(CaptureEvent {
            capturer_id,
            captured_piece_type: captured.piece_type.clone(),
            captured_owner_id: captured.owner_id,
            captured_position: captured.position,
        });
        if !captured.piece_type.is_king() {
            return;
        }
        let name_of =
            |id: PlayerId, state: &GameState| state.players.get(&id).map(|p| p.name.clone());
        if let Some(victim_name) = captured.owner_id.and_then(|id| name_of(id, &self.state)) {
            self.hooks.kills.push(KillEvent {
                victim_name,
                killer_name: capturer_id.and_then(|id| name_of(id, &self.state)),
            });
        }
    }

    /// Records that a player left the match.
    pub(super) fn record_leave(&mut self) {
        self.hooks.player_left = true;
    }

    /// Records that a player joined the match.
    pub(super) fn record_join(&mut self, player_name: String) {
        self.hooks.joins.push(player_name);
    }

    /// Records that a player left the board by leaving or by losing the connection.
    pub(super) fn record_disconnect(&mut self, player_name: String) {
        self.hooks.disconnects.push(player_name);
    }

    /// Records one second of the match countdown.
    pub(super) fn record_countdown(&mut self, seconds: u32) {
        self.hooks.countdowns.push(seconds);
    }

    /// Records that the match started.
    pub(super) fn record_game_start(&mut self) {
        self.hooks.game_started = true;
    }

    /// Applies every configured hook to the events collected since the last tick.
    ///
    /// Hooks run in config order; a matched hook with `capture` stops later hooks. Only the
    /// first victory produced is delivered.
    pub(super) fn resolve_hooks(&mut self, now: TimestampMs) {
        let events = std::mem::take(&mut self.hooks);
        if events.is_empty() {
            return;
        }

        let setup = self.setup.clone();
        let mut victory = None;
        for hook in &setup.mode.hooks {
            let outcome = self.apply_hook(hook, &events, now);
            victory = victory.or(outcome.victory);
            if outcome.matched && hook.capture {
                break;
            }
        }

        if let Some(victory) = victory {
            self.victors.insert(victory.player_id);
            self.outbox.send_to_player(
                victory.player_id,
                ServerMessage::Victory {
                    title: victory.title,
                    message: victory.message,
                },
            );
            if let Some(cue) = victory.camera_cue {
                self.outbox
                    .send_to_player(victory.player_id, ServerMessage::SetCamera(cue));
            }
        }
    }

    /// Applies one hook to the collected events.
    fn apply_hook(
        &mut self,
        hook: &HookConfig,
        events: &HookEvents,
        now: TimestampMs,
    ) -> HookOutcome {
        let targets = |piece_type: &_| hook.targets_piece(piece_type);
        let Some(supported) = hook.supported_hook() else {
            return HookOutcome::default();
        };
        match supported {
            SupportedHook::EliminateOwnerOnCapture => HookOutcome::matched(self.eliminate_owners(
                hook,
                events.owned_captures(targets),
                now,
            )),
            SupportedHook::WinCapturerOnActiveCapture => {
                let capture = events.first_capture(targets);
                HookOutcome {
                    matched: capture.is_some(),
                    victory: capture.and_then(|event| VictoryMessage::capturer(hook, event)),
                }
            }
            SupportedHook::WinRemainingOnPlayerLeave => {
                HookOutcome::victory(self.remaining_player_victory(hook, events))
            }
            SupportedHook::SystemChat(event) => HookOutcome::matched(self.emit_system_chats(
                hook,
                default_chat_template(event),
                MessageContext::of_event(event, events),
                now,
            )),
        }
    }

    /// Eliminates the owners of the captured pieces that are still in the match, making the
    /// hook's camera move for each of them first, while they can still be messaged.
    ///
    /// Elimination is not a leave, so it never triggers leave-based victories.
    fn eliminate_owners<'a>(
        &mut self,
        hook: &HookConfig,
        captures: impl Iterator<Item = &'a CaptureEvent>,
        now: TimestampMs,
    ) -> bool {
        let mut matched = false;
        for capture in captures {
            let Some(player_id) = capture.captured_owner_id else {
                continue;
            };
            if !self.state.players.contains_key(&player_id) {
                continue;
            }
            if let Some(cue) = hook.camera_cue(capture.captured_position) {
                self.outbox
                    .send_to_player(player_id, ServerMessage::SetCamera(cue));
            }
            matched |= self.remove_player(player_id, Departure::Eliminated, now);
        }
        matched
    }

    /// Returns the victory of the only remaining player after a leave, if `hook` applies.
    fn remaining_player_victory(
        &self,
        hook: &HookConfig,
        events: &HookEvents,
    ) -> Option<VictoryMessage> {
        if !events.player_left {
            return None;
        }
        let players_left = self.state.players.len() as u32;
        if hook
            .players_left
            .is_some_and(|required| required != players_left)
            || players_left != 1
        {
            return None;
        }
        let player_id = *self.state.players.keys().next()?;
        Some(VictoryMessage::remaining_player(hook, player_id))
    }

    /// Posts one system chat line per context, using the hook message or `default_template`.
    ///
    /// Returns true when at least one line was posted.
    fn emit_system_chats<'a>(
        &mut self,
        hook: &HookConfig,
        default_template: &str,
        contexts: impl IntoIterator<Item = MessageContext<'a>>,
        now: TimestampMs,
    ) -> bool {
        let template = hook.message.as_deref().unwrap_or(default_template);
        let mut sent = false;
        for context in contexts {
            let line = ChatLine {
                sender_name: SYSTEM_CHAT_NAME.to_string(),
                sender_color: ColorHex::from(SYSTEM_CHAT_COLOR),
                message: context.render(template),
                is_system: true,
                sent_at: now,
            };
            self.post_chat(line, now);
            sent = true;
        }
        sent
    }
}
