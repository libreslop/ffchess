//! Per-connection rate limiting of client messages.

use common::types::{DurationMs, TimestampMs};

/// Messages a client may send in a burst.
const BURST: f64 = 40.0;

/// Sustained messages per second a client may send.
const REFILL_PER_SEC: f64 = 20.0;

/// Token bucket that allows short bursts (such as several premoves in a row) while capping
/// the sustained message rate.
pub(super) struct MessageThrottle {
    tokens: f64,
    refilled_at: TimestampMs,
}

impl MessageThrottle {
    /// Creates a full bucket.
    pub(super) fn new(now: TimestampMs) -> Self {
        Self {
            tokens: BURST,
            refilled_at: now,
        }
    }

    /// Returns whether a message arriving at `now` is accepted, consuming a token if so.
    pub(super) fn allow(&mut self, now: TimestampMs) -> bool {
        let elapsed = (now - self.refilled_at).max(DurationMs::zero());
        self.tokens = (self.tokens + elapsed.as_secs_f64() * REFILL_PER_SEC).min(BURST);
        self.refilled_at = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}
