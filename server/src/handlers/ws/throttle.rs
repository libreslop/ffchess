use common::types::{DurationMs, TimestampMs};

const MIN_MESSAGE_INTERVAL_MS: i64 = 50;

/// Enforces a minimum interval between accepted websocket messages.
pub(super) struct MessageThrottle {
    last_message_at: Option<TimestampMs>,
}

impl MessageThrottle {
    /// Creates a fresh message throttle with no prior accepted message.
    pub(super) fn new() -> Self {
        Self {
            last_message_at: None,
        }
    }

    /// Returns whether the next message should be accepted.
    pub(super) fn allow_next(&mut self) -> bool {
        let now = crate::time::now_ms();
        if let Some(last_message_at) = self.last_message_at
            && now - last_message_at < DurationMs::from_millis(MIN_MESSAGE_INTERVAL_MS)
        {
            return false;
        }

        self.last_message_at = Some(now);
        true
    }
}
