//! Room chat history with retention rules.

use common::protocol::ChatLine;
use common::types::{DurationMs, TimestampMs};
use std::collections::VecDeque;

/// Extra time a message remains after expiry so the client can finish fading it out.
const CHAT_FADE_OUT: DurationMs = DurationMs(500);

/// Maximum number of chat lines retained per room.
const MAX_CHAT_HISTORY: usize = 120;

/// Retained chat messages for one room.
pub(super) struct ChatHistory {
    ttl: DurationMs,
    lines: VecDeque<ChatLine>,
}

impl ChatHistory {
    /// Creates an empty chat history whose lines live for `ttl`.
    pub(super) fn new(ttl: DurationMs) -> Self {
        Self {
            ttl,
            lines: VecDeque::new(),
        }
    }

    /// Appends one line and trims stale or excess entries.
    pub(super) fn push(&mut self, line: ChatLine, now: TimestampMs) {
        self.prune(now);
        self.lines.push_back(line);
        while self.lines.len() > MAX_CHAT_HISTORY {
            self.lines.pop_front();
        }
    }

    /// Returns all currently retained lines after pruning expired entries.
    pub(super) fn snapshot(&mut self, now: TimestampMs) -> Vec<ChatLine> {
        self.prune(now);
        self.lines.iter().cloned().collect()
    }

    /// Drops lines older than the TTL plus the client fade-out window.
    fn prune(&mut self, now: TimestampMs) {
        let retention = self.ttl + CHAT_FADE_OUT;
        while self
            .lines
            .front()
            .is_some_and(|line| now - line.sent_at >= retention)
        {
            self.lines.pop_front();
        }
    }
}
