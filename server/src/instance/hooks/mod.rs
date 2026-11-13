//! Configured hooks reacting to gameplay events once per tick.

mod events;

pub(super) use events::HookEvents;

use super::players::Departure;
use super::world::World;
use common::models::{GameState, HookConfig, HookVictoryFocus, Piece, SupportedHook};
use common::protocol::{ChatLine, ServerMessage, VictoryFocusTarget};
use common::types::{ColorHex, PlayerId, TimestampMs};
use events::{CaptureEvent, KillEvent};

const SYSTEM_CHAT_NAME: &str = "System";
const SYSTEM_CHAT_COLOR: &str = "#facc15";

/// A user-facing victory message selected by hook evaluation.
#[derive(Debug)]
struct VictoryMessage {
    player_id: PlayerId,
    title: String,
    message: String,
    focus_target: VictoryFocusTarget,
}

impl VictoryMessage {
    /// Builds the victory of the player who made a winning capture.
    fn capturer(hook: &HookConfig, event: &CaptureEvent) -> Option<Self> {
        let focus_target =
            match hook.victory_focus_or_default(SupportedHook::WinCapturerOnActiveCapture) {
                HookVictoryFocus::CaptureSquare => {
                    VictoryFocusTarget::BoardPosition(event.captured_position)
                }
                HookVictoryFocus::KeepCurrent => VictoryFocusTarget::KeepCurrent,
            };
        Some(Self {
            player_id: event.capturer_id?,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("You won by capturing the enemy king."),
            focus_target,
        })
    }

    /// Builds the victory of the last player left in the match.
    fn remaining_player(hook: &HookConfig, player_id: PlayerId) -> Self {
        Self {
            player_id,
            title: hook.victory_title_or("VICTORY"),
            message: hook.victory_message_or("Opponent disconnected. You win."),
            focus_target: VictoryFocusTarget::KeepCurrent,
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

impl MessageContext<'_> {
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

    /// Records that a player's connection dropped.
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
                    focus_target: victory.focus_target,
                },
            );
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
            SupportedHook::EliminateOwnerOnCapture => {
                HookOutcome::matched(self.eliminate_owners(events.captured_owners(targets), now))
            }
            SupportedHook::WinCapturerOnActiveCapture => HookOutcome {
                matched: events.first_capture(targets).is_some(),
                victory: events
                    .first_capture(targets)
                    .and_then(|event| VictoryMessage::capturer(hook, event)),
            },
            SupportedHook::WinRemainingOnPlayerLeave => {
                HookOutcome::victory(self.remaining_player_victory(hook, events))
            }
            SupportedHook::SystemChatOnPlayerJoin => HookOutcome::matched(self.emit_system_chats(
                hook,
                "{player} joined the game",
                events.joins.iter().map(|name| MessageContext {
                    player: Some(name),
                    ..MessageContext::default()
                }),
                now,
            )),
            SupportedHook::SystemChatOnPlayerDisconnect => {
                HookOutcome::matched(self.emit_system_chats(
                    hook,
                    "{player} disconnected",
                    events.disconnects.iter().map(|name| MessageContext {
                        player: Some(name),
                        ..MessageContext::default()
                    }),
                    now,
                ))
            }
            SupportedHook::SystemChatOnPlayerKilled => {
                HookOutcome::matched(self.emit_system_chats(
                    hook,
                    "{player} was killed by {killer}",
                    events.kills.iter().map(|kill| MessageContext {
                        player: Some(&kill.victim_name),
                        killer: Some(kill.killer_name.as_deref().unwrap_or("NPC")),
                        ..MessageContext::default()
                    }),
                    now,
                ))
            }
            SupportedHook::SystemChatOnQueueCountdown => {
                HookOutcome::matched(self.emit_system_chats(
                    hook,
                    "Match starts in {seconds}",
                    events.countdowns.iter().map(|seconds| MessageContext {
                        seconds: Some(*seconds),
                        ..MessageContext::default()
                    }),
                    now,
                ))
            }
            SupportedHook::SystemChatOnGameStart => HookOutcome::matched(self.emit_system_chats(
                hook,
                "Game started!",
                events.game_started.then(MessageContext::default),
                now,
            )),
        }
    }

    /// Eliminates the listed players that are still in the match.
    ///
    /// Elimination is not a leave, so it never triggers leave-based victories.
    fn eliminate_owners(&mut self, owners: Vec<PlayerId>, now: TimestampMs) -> bool {
        let mut matched = false;
        for player_id in owners {
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
