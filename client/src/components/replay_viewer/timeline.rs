//! Replay time: the boards along a replay and the position of playback within it.

use common::models::{GameState, Replay, ReplayKeyframe};
use common::types::{DurationMs, TimestampMs};

/// Every board of a replay, ready to be shown at any moment of it.
#[derive(Debug, PartialEq)]
pub(super) struct ReplayTimeline {
    keyframes: Vec<ReplayKeyframe>,
    end: TimestampMs,
    /// Moments a piece left the board, marked on the scrub bar.
    captures: Vec<TimestampMs>,
}

impl ReplayTimeline {
    /// Builds the timeline of a replay, or `None` when it recorded no board.
    pub(super) fn new(replay: &Replay) -> Option<Self> {
        let keyframes = replay.keyframes();
        let start = keyframes.first()?.at;
        let captures = keyframes
            .windows(2)
            .filter(|pair| pair[1].state.pieces.len() < pair[0].state.pieces.len())
            .map(|pair| pair[1].at)
            .collect();
        Some(Self {
            end: replay.ended_at.max(start),
            keyframes,
            captures,
        })
    }

    /// Returns the moment of the first recorded board.
    pub(super) fn start(&self) -> TimestampMs {
        self.keyframes[0].at
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

    /// Returns how far into the replay each piece was taken.
    pub(super) fn capture_times(&self) -> Vec<DurationMs> {
        self.captures.iter().map(|at| self.elapsed(*at)).collect()
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

    /// Returns the board with the given index.
    pub(super) fn board(&self, index: usize) -> &GameState {
        &self.keyframes[index.min(self.keyframes.len() - 1)].state
    }
}
