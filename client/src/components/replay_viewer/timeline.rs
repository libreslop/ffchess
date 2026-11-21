//! Replay time: the boards along a replay and the position of playback within it.

use common::models::{GameState, Player, Replay, ReplayKeyframe};
use common::types::{DurationMs, PlayerId, TimestampMs};

/// Every board of a replay, ready to be shown at any moment of it.
///
/// The replay starts when moves were unlocked, skipping the countdown before.
#[derive(Debug, PartialEq)]
pub(super) struct ReplayTimeline {
    keyframes: Vec<ReplayKeyframe>,
    start: TimestampMs,
    end: TimestampMs,
}

impl ReplayTimeline {
    /// Builds the timeline of a replay, or `None` when it recorded no board.
    pub(super) fn new(replay: &Replay) -> Option<Self> {
        let keyframes = replay.keyframes();
        let first = keyframes.first()?.at;
        let end = replay.ended_at.max(first);
        // A match that finished during its countdown starts as late as it ends.
        let start = replay.started_at.clamp(first, end);
        Some(Self {
            keyframes,
            start,
            end,
        })
    }

    /// Returns the moment moves were unlocked.
    pub(super) fn start(&self) -> TimestampMs {
        self.start
    }

    /// Returns the moment the match finished.
    pub(super) fn end(&self) -> TimestampMs {
        self.end
    }

    /// Returns how long the replay lasts.
    pub(super) fn duration(&self) -> DurationMs {
        self.end - self.start()
    }

    /// Returns how far `at` is into the replay.
    pub(super) fn elapsed(&self, at: TimestampMs) -> DurationMs {
        (at - self.start()).max(DurationMs::zero())
    }

    /// Returns `at` moved into the replay.
    pub(super) fn clamp(&self, at: TimestampMs) -> TimestampMs {
        at.clamp(self.start(), self.end)
    }

    /// Returns the index of the board shown at `at`.
    pub(super) fn index_at(&self, at: TimestampMs) -> usize {
        self.keyframes
            .partition_point(|keyframe| keyframe.at <= at)
            .saturating_sub(1)
    }

    /// Returns the boards from the start of the replay on, the first as it stood at the start.
    pub(super) fn keyframes_from_start(&self) -> &[ReplayKeyframe] {
        &self.keyframes[self.index_at(self.start)..]
    }

    /// Returns true when `player_id` played the replay with the board upside down, or `None`
    /// when they did not play it.
    pub(super) fn rotated_180_for(&self, player_id: PlayerId) -> Option<bool> {
        self.keyframes
            .iter()
            .find_map(|keyframe| keyframe.state.players.get(&player_id))
            .map(Player::is_board_rotated_180)
    }

    /// Returns the board with the given index.
    pub(super) fn board(&self, index: usize) -> &GameState {
        &self.keyframes[index.min(self.keyframes.len() - 1)].state
    }
}
