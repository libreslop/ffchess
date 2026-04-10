use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Total points or currency awarded to a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Score(pub u64);

impl Score {
    /// Returns a zero-valued score.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Returns the underlying numeric score value.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for Score {
    /// Provides the default score (zero).
    fn default() -> Self {
        Self::zero()
    }
}

impl From<u64> for Score {
    /// Wraps a raw `u64` as a score.
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Score> for u64 {
    /// Unwraps a score into its raw numeric value.
    fn from(value: Score) -> Self {
        value.0
    }
}

impl fmt::Display for Score {
    /// Formats the score as a decimal string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for Score {
    type Output = Self;

    /// Adds two scores, saturating on overflow.
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Score {
    type Output = Self;

    /// Subtracts two scores, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for Score {
    /// Adds another score in-place, saturating on overflow.
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Score {
    /// Subtracts another score in-place, saturating on underflow.
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// Number of players represented as a domain value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct PlayerCount(pub u32);

impl PlayerCount {
    /// Returns a zero-valued player count.
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Wraps a raw player count.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying player count.
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns true when exactly one player is represented.
    pub const fn is_one(self) -> bool {
        self.0 == 1
    }

    /// Saturating subtraction between counts.
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl Default for PlayerCount {
    /// Provides a default count of zero players.
    fn default() -> Self {
        Self::zero()
    }
}

impl From<u32> for PlayerCount {
    /// Wraps a raw `u32` as a player count.
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PlayerCount> for u32 {
    /// Unwraps a player count into its raw numeric value.
    fn from(value: PlayerCount) -> Self {
        value.0
    }
}

impl fmt::Display for PlayerCount {
    /// Formats the player count as a decimal string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for PlayerCount {
    type Output = Self;

    /// Adds two counts, saturating on overflow.
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for PlayerCount {
    type Output = Self;

    /// Subtracts two counts, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for PlayerCount {
    /// Adds another count in-place, saturating on overflow.
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for PlayerCount {
    /// Subtracts another count in-place, saturating on underflow.
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// One-based position within a matchmaking queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct QueuePosition(pub u32);

impl QueuePosition {
    /// Wraps a queue position value.
    pub const fn new(value: u32) -> Self {
        if value == 0 {
            return Self(1);
        }
        Self(value)
    }

    /// Returns the underlying queue position value.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for QueuePosition {
    /// Formats the queue position as a decimal string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
