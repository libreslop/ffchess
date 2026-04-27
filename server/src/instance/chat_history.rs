//! Typed room-chat history with retention rules.

use common::protocol::ChatLine;
use common::types::TimestampMs;
use std::collections::VecDeque;

/// Extra time a message remains after expiry so the client can finish fading it out.
const CHAT_FADE_OUT_MS: i64 = 500;

/// Maximum number of chat lines retained per room.
const MAX_CHAT_HISTORY: usize = 120;

/// Retained chat messages for one room.
#[derive(Debug, Default)]
pub(super) struct ChatHistory {
    ttl_ms: u32,
    lines: VecDeque<ChatLine>,
}

impl ChatHistory {
    /// Creates an empty chat history with one configured TTL.
    pub fn new(ttl_ms: u32) -> Self {
        Self {
            ttl_ms,
            lines: VecDeque::new(),
        }
    }

    /// Appends one line and trims stale or excess entries.
    pub fn push(&mut self, line: ChatLine, now: TimestampMs) {
        self.prune(now);
        self.lines.push_back(line);
        while self.lines.len() > MAX_CHAT_HISTORY {
            self.lines.pop_front();
        }
    }

    /// Returns all currently retained lines after pruning expired entries.
    pub fn snapshot(&mut self, now: TimestampMs) -> Vec<ChatLine> {
        self.prune(now);
        self.lines.iter().cloned().collect()
    }

    fn prune(&mut self, now: TimestampMs) {
        let prune_after_ms = self.ttl_ms.max(1) as i64 + CHAT_FADE_OUT_MS;
        while self.lines.front().is_some_and(|line| {
            now.as_i64().saturating_sub(line.sent_at.as_i64()) >= prune_after_ms
        }) {
            self.lines.pop_front();
        }
    }
}
