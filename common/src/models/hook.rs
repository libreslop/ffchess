use crate::models::camera::{CameraCue, SetCameraConfig};
use crate::types::{BoardCoord, PieceTypeId};
use serde::{Deserialize, Serialize};

/// Gameplay event a hook reacts to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookTrigger {
    OnCapture,
    OnCapturePieceActive,
    OnPlayerLeave,
    OnPlayerJoin,
    OnPlayerDisconnect,
    OnPlayerKilled,
    OnQueueCountdown,
    OnGameStart,
    OnSpectatorJoin,
    OnSpectatorLeave,
}

impl HookTrigger {
    /// Returns the event a system chat hook on this trigger announces, if it announces any.
    fn chat_event(self) -> Option<ChatEvent> {
        Some(match self {
            Self::OnPlayerJoin => ChatEvent::PlayerJoin,
            Self::OnPlayerDisconnect => ChatEvent::PlayerDisconnect,
            Self::OnPlayerKilled => ChatEvent::PlayerKilled,
            Self::OnQueueCountdown => ChatEvent::QueueCountdown,
            Self::OnGameStart => ChatEvent::GameStart,
            Self::OnSpectatorJoin => ChatEvent::SpectatorJoin,
            Self::OnSpectatorLeave => ChatEvent::SpectatorLeave,
            Self::OnCapture | Self::OnCapturePieceActive | Self::OnPlayerLeave => return None,
        })
    }
}

/// Effect applied when a hook trigger matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookAction {
    EliminateOwner,
    WinCapturer,
    WinRemaining,
    SystemChatMessage,
}

/// Event announced in chat by a system chat hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatEvent {
    PlayerJoin,
    PlayerDisconnect,
    PlayerKilled,
    QueueCountdown,
    GameStart,
    SpectatorJoin,
    SpectatorLeave,
}

/// Supported hook behaviors understood by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedHook {
    EliminateOwnerOnCapture,
    WinCapturerOnActiveCapture,
    WinRemainingOnPlayerLeave,
    /// Posts a system chat line for every occurrence of the event.
    SystemChat(ChatEvent),
}

/// Trigger-action hook for game events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    pub trigger: HookTrigger,
    pub target_piece_id: Option<PieceTypeId>,
    #[serde(default)]
    pub players_left: Option<u32>,
    pub action: HookAction,
    #[serde(default)]
    pub capture: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    /// Camera move made for the player the hook affects when a piece dies; read by
    /// `EliminateOwner` for the eliminated player and `WinCapturer` for the winner.
    #[serde(default)]
    pub set_camera: Option<SetCameraConfig>,
}

impl HookConfig {
    /// Returns the typed supported hook behavior for this config entry.
    pub fn supported_hook(&self) -> Option<SupportedHook> {
        match (self.trigger, self.action) {
            (HookTrigger::OnCapture, HookAction::EliminateOwner) => {
                Some(SupportedHook::EliminateOwnerOnCapture)
            }
            (HookTrigger::OnCapturePieceActive, HookAction::WinCapturer) => {
                Some(SupportedHook::WinCapturerOnActiveCapture)
            }
            (HookTrigger::OnPlayerLeave, HookAction::WinRemaining) => {
                Some(SupportedHook::WinRemainingOnPlayerLeave)
            }
            (trigger, HookAction::SystemChatMessage) => {
                trigger.chat_event().map(SupportedHook::SystemChat)
            }
            _ => None,
        }
    }

    /// Returns whether this hook targets the provided piece type, or all piece types.
    pub fn targets_piece(&self, piece_type_id: &PieceTypeId) -> bool {
        self.target_piece_id
            .as_ref()
            .is_none_or(|target_piece_id| target_piece_id == piece_type_id)
    }

    /// Returns the custom hook title or `default_title` when none is configured.
    pub fn victory_title_or(&self, default_title: &str) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| default_title.to_string())
    }

    /// Returns the custom hook message or `default_message` when none is configured.
    pub fn victory_message_or(&self, default_message: &str) -> String {
        self.message
            .clone()
            .unwrap_or_else(|| default_message.to_string())
    }

    /// Returns the camera move this hook makes when a piece died on `death_square`, if any.
    pub fn camera_cue(&self, death_square: BoardCoord) -> Option<CameraCue> {
        self.set_camera.clone().map(|camera| CameraCue {
            camera,
            death_square,
        })
    }

    /// Checks the camera expressions of this hook.
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.set_camera
            .as_ref()
            .map_or(Ok(()), SetCameraConfig::validate)
            .map_err(|error| format!("{:?} hook: {error}", self.trigger))
    }
}
