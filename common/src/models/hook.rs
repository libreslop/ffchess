use crate::types::PieceTypeId;
use serde::{Deserialize, Serialize};

/// Trigger-action hook for game events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookTrigger {
    #[serde(rename = "OnCapture")]
    OnCapture,
    #[serde(rename = "OnCapturePieceActive")]
    OnCapturePieceActive,
    #[serde(rename = "OnPlayerLeave")]
    OnPlayerLeave,
    #[serde(rename = "OnPlayerJoin")]
    OnPlayerJoin,
    #[serde(rename = "OnPlayerDisconnect")]
    OnPlayerDisconnect,
    #[serde(rename = "OnPlayerKilled")]
    OnPlayerKilled,
    #[serde(rename = "OnQueueCountdown")]
    OnQueueCountdown,
    #[serde(rename = "OnGameStart")]
    OnGameStart,
}

/// Effect applied when a hook trigger matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookAction {
    #[serde(rename = "EliminateOwner")]
    EliminateOwner,
    #[serde(rename = "WinCapturer")]
    WinCapturer,
    #[serde(rename = "WinRemaining")]
    WinRemaining,
    #[serde(rename = "SystemChatMessage")]
    SystemChatMessage,
}

/// Camera focus policy for victory overlays produced by hooks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookVictoryFocus {
    #[serde(rename = "CaptureSquare")]
    CaptureSquare,
    #[serde(rename = "KeepCurrent")]
    KeepCurrent,
}

/// Supported hook behaviors understood by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedHook {
    EliminateOwnerOnCapture,
    WinCapturerOnActiveCapture,
    WinRemainingOnPlayerLeave,
    SystemChatOnPlayerJoin,
    SystemChatOnPlayerDisconnect,
    SystemChatOnPlayerKilled,
    SystemChatOnQueueCountdown,
    SystemChatOnGameStart,
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
    #[serde(default)]
    pub victory_focus: Option<HookVictoryFocus>,
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
            (HookTrigger::OnPlayerJoin, HookAction::SystemChatMessage) => {
                Some(SupportedHook::SystemChatOnPlayerJoin)
            }
            (HookTrigger::OnPlayerDisconnect, HookAction::SystemChatMessage) => {
                Some(SupportedHook::SystemChatOnPlayerDisconnect)
            }
            (HookTrigger::OnPlayerKilled, HookAction::SystemChatMessage) => {
                Some(SupportedHook::SystemChatOnPlayerKilled)
            }
            (HookTrigger::OnQueueCountdown, HookAction::SystemChatMessage) => {
                Some(SupportedHook::SystemChatOnQueueCountdown)
            }
            (HookTrigger::OnGameStart, HookAction::SystemChatMessage) => {
                Some(SupportedHook::SystemChatOnGameStart)
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

    /// Returns the configured victory focus policy, or the default for `supported_hook`.
    pub fn victory_focus_or_default(&self, supported_hook: SupportedHook) -> HookVictoryFocus {
        self.victory_focus.unwrap_or(match supported_hook {
            SupportedHook::WinCapturerOnActiveCapture => HookVictoryFocus::CaptureSquare,
            SupportedHook::WinRemainingOnPlayerLeave => HookVictoryFocus::KeepCurrent,
            SupportedHook::EliminateOwnerOnCapture => HookVictoryFocus::KeepCurrent,
            SupportedHook::SystemChatOnPlayerJoin
            | SupportedHook::SystemChatOnPlayerDisconnect
            | SupportedHook::SystemChatOnPlayerKilled
            | SupportedHook::SystemChatOnQueueCountdown
            | SupportedHook::SystemChatOnGameStart => HookVictoryFocus::KeepCurrent,
        })
    }
}
