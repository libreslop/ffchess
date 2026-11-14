//! Strongly typed UI flow state for overlays and cooldowns.

use crate::reducer::{ClientPhase, GameStateReducer};
use common::types::DurationMs;

/// Join overlay step in the pre-game flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum JoinStep {
    #[default]
    EnterName,
    SelectKit,
    /// Picking a running match of a queue mode to spectate.
    SelectMatch,
}

impl JoinStep {
    /// Returns true when the name entry form should be shown.
    pub const fn is_enter_name(self) -> bool {
        matches!(self, Self::EnterName)
    }

    /// Returns true when kit selection should be shown.
    pub const fn is_select_kit(self) -> bool {
        matches!(self, Self::SelectKit)
    }

    /// Returns true when the list of matches to spectate should be shown.
    pub const fn is_select_match(self) -> bool {
        matches!(self, Self::SelectMatch)
    }
}

/// Whether the UI is currently forcing the join overlay during a rejoin cycle.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RejoinFlow {
    #[default]
    Inactive,
    Active,
}

impl RejoinFlow {
    /// Returns true when the join overlay should remain pinned over game content.
    pub const fn forces_join_overlay(self, has_match_result: bool) -> bool {
        self.is_active() && !has_match_result
    }

    /// Returns true when this rejoin flow is active.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// UI cooldown value represented in whole seconds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct CooldownSeconds(u32);

impl CooldownSeconds {
    /// Returns a zero-second cooldown.
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Converts a duration to whole seconds, rounding down and clamping negatives to zero.
    pub fn from_duration(duration: DurationMs) -> Self {
        Self((duration.as_u64() / 1000) as u32)
    }

    /// Returns the cooldown in whole seconds.
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns true when no cooldown remains.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns true when cooldown is currently active.
    pub const fn is_active(self) -> bool {
        self.0 > 0
    }

    /// Returns the cooldown decremented by one second, saturating at zero.
    pub const fn decrement(self) -> Self {
        Self(self.0.saturating_sub(1))
    }
}

/// What sits behind floating UI such as the chat and the leave button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backdrop {
    /// The bright board, seen while playing or spectating.
    LightBoard,
    /// The dimming overlay of menus, queues and end screens.
    DarkOverlay,
}

impl Backdrop {
    /// Returns the backdrop of the screen the client currently shows.
    pub fn of(reducer: &GameStateReducer) -> Self {
        let watching = reducer.phase == ClientPhase::Spectating && !reducer.spectate_ended();
        if (reducer.phase == ClientPhase::Alive && !reducer.has_match_result()) || watching {
            Self::LightBoard
        } else {
            Self::DarkOverlay
        }
    }

    /// Returns the color of text and icons drawn over this backdrop.
    pub const fn foreground(self) -> &'static str {
        match self {
            Self::LightBoard => "#6b7280",
            Self::DarkOverlay => "#d1d5db",
        }
    }

    /// Returns the drop-shadow color of text and icons drawn over this backdrop.
    pub const fn foreground_shadow(self) -> &'static str {
        "#374151"
    }
}
