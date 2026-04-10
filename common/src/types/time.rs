use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Absolute time in milliseconds since epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
#[serde(transparent)]
pub struct TimestampMs(pub i64);

impl TimestampMs {
    /// Wraps a raw millisecond timestamp.
    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    /// Returns the raw millisecond timestamp.
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for TimestampMs {
    /// Wraps a raw millisecond timestamp.
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<TimestampMs> for i64 {
    /// Unwraps the timestamp into raw milliseconds.
    fn from(value: TimestampMs) -> Self {
        value.0
    }
}

/// Duration measured in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct DurationMs(pub i64);

impl DurationMs {
    /// Returns a zero-length duration.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Wraps a raw millisecond duration.
    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    /// Returns the raw millisecond duration.
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Returns the duration clamped to a non-negative `u64`.
    pub fn as_u64(self) -> u64 {
        self.0.max(0) as u64
    }

    /// Returns the duration in seconds as a floating point value.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }
}

impl Default for DurationMs {
    /// Provides the default duration (zero).
    fn default() -> Self {
        Self::zero()
    }
}

impl From<i64> for DurationMs {
    /// Wraps a raw millisecond duration.
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<DurationMs> for i64 {
    /// Unwraps the duration into raw milliseconds.
    fn from(value: DurationMs) -> Self {
        value.0
    }
}

impl Add for DurationMs {
    type Output = Self;

    /// Adds two durations, saturating on overflow.
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for DurationMs {
    type Output = Self;

    /// Subtracts two durations, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for DurationMs {
    /// Adds another duration in-place, saturating on overflow.
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for DurationMs {
    /// Subtracts another duration in-place, saturating on underflow.
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Sub for TimestampMs {
    type Output = DurationMs;

    /// Returns the duration between two timestamps, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        DurationMs(self.0.saturating_sub(rhs.0))
    }
}

impl Add<DurationMs> for TimestampMs {
    type Output = Self;

    /// Adds a duration to a timestamp, saturating on overflow.
    fn add(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub<DurationMs> for TimestampMs {
    type Output = Self;

    /// Subtracts a duration from a timestamp, saturating on underflow.
    fn sub(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_add(-rhs.0))
    }
}
