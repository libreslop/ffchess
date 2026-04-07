//! Strongly typed UI flow state for overlays and cooldowns.

/// Join overlay step in the pre-game flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum JoinStep {
    #[default]
    EnterName,
    SelectKit,
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

    /// Creates a cooldown from whole seconds.
    pub const fn from_seconds(seconds: u32) -> Self {
        Self(seconds)
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
