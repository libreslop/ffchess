//! Playback of a replay: where it is, how fast it goes, and the commands that move it.

use super::timeline::ReplayTimeline;
use crate::utils::performance_now;
use common::types::{DurationMs, TimestampMs};
use std::rc::Rc;
use yew::prelude::*;

/// How many times faster than real time a replay plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PlaybackSpeed(u32);

impl PlaybackSpeed {
    /// Real time.
    pub(super) const NORMAL: Self = Self(1);

    /// Fastest speed offered.
    const MAX: u32 = 8;

    /// Returns the next speed in the cycle 1x, 2x, 4x, 8x.
    pub(super) fn next(self) -> Self {
        if self.0 >= Self::MAX {
            Self::NORMAL
        } else {
            Self(self.0 * 2)
        }
    }

    /// Returns the label shown on the speed button.
    pub(super) fn label(self) -> String {
        format!("{}x", self.0)
    }
}

/// Where playback is in a replay and whether it moves on.
///
/// Playback keeps the replay time it had at one local instant; while playing, replay time
/// advances from there with local time, so it needs no timer of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Playback {
    /// Replay time at `anchor_local_ms`.
    anchor: TimestampMs,
    /// Local `performance.now()` time the anchor was taken.
    anchor_local_ms: f64,
    speed: PlaybackSpeed,
    playing: bool,
}

impl Playback {
    /// Starts playing from `at` at normal speed.
    pub(super) fn playing_from(at: TimestampMs, now_local_ms: f64) -> Self {
        Self {
            anchor: at,
            anchor_local_ms: now_local_ms,
            speed: PlaybackSpeed::NORMAL,
            playing: true,
        }
    }

    /// Returns the replay time at local time `now_local_ms`, kept within `timeline`.
    pub(super) fn position(&self, timeline: &ReplayTimeline, now_local_ms: f64) -> TimestampMs {
        let elapsed = if self.playing {
            (now_local_ms - self.anchor_local_ms).max(0.0) * self.speed.0 as f64
        } else {
            0.0
        };
        timeline.clamp(self.anchor + DurationMs::from_millis(elapsed as i64))
    }

    /// Returns true while playback moves on.
    pub(super) fn is_playing(&self) -> bool {
        self.playing
    }

    /// Returns the playback speed.
    pub(super) fn speed(&self) -> PlaybackSpeed {
        self.speed
    }

    /// Returns true once a playing replay reached its end.
    pub(super) fn ran_out(&self, timeline: &ReplayTimeline, now_local_ms: f64) -> bool {
        self.playing && self.position(timeline, now_local_ms) >= timeline.end()
    }

    /// Pauses or resumes; resuming at the end starts over.
    pub(super) fn toggled(self, timeline: &ReplayTimeline, now_local_ms: f64) -> Self {
        let at = self.position(timeline, now_local_ms);
        let at = if !self.playing && at >= timeline.end() {
            timeline.start()
        } else {
            at
        };
        self.anchored(at, now_local_ms, !self.playing)
    }

    /// Jumps to `at`, keeping on playing or staying paused.
    pub(super) fn seeked(
        self,
        timeline: &ReplayTimeline,
        at: TimestampMs,
        now_local_ms: f64,
    ) -> Self {
        self.anchored(timeline.clamp(at), now_local_ms, self.playing)
    }

    /// Jumps by `offset` from the current position.
    pub(super) fn skipped(
        self,
        timeline: &ReplayTimeline,
        offset: DurationMs,
        now_local_ms: f64,
    ) -> Self {
        let at = self.position(timeline, now_local_ms) + offset;
        self.seeked(timeline, at, now_local_ms)
    }

    /// Stops at the current position.
    pub(super) fn paused(self, timeline: &ReplayTimeline, now_local_ms: f64) -> Self {
        let at = self.position(timeline, now_local_ms);
        self.anchored(at, now_local_ms, false)
    }

    /// Switches to the next speed without jumping.
    pub(super) fn sped_up(self, timeline: &ReplayTimeline, now_local_ms: f64) -> Self {
        let at = self.position(timeline, now_local_ms);
        Self {
            speed: self.speed.next(),
            ..self.anchored(at, now_local_ms, self.playing)
        }
    }

    /// Returns this playback re-anchored at `at` now.
    fn anchored(self, at: TimestampMs, now_local_ms: f64, playing: bool) -> Self {
        Self {
            anchor: at,
            anchor_local_ms: now_local_ms,
            playing,
            ..self
        }
    }
}

/// A request to move playback, dispatched by the controls and the keyboard.
pub(super) enum PlaybackCommand {
    Toggle,
    /// Jumps to a time into the replay.
    Seek(DurationMs),
    /// Jumps forward, or back for a negative offset.
    Skip(DurationMs),
    NextSpeed,
    /// Pauses; sent once playing reached the end.
    Pause,
}

/// Playback of one replay, as a reducer so every command sees the latest position.
#[derive(Debug, PartialEq)]
pub(super) struct PlaybackState {
    pub(super) timeline: Rc<ReplayTimeline>,
    pub(super) playback: Playback,
}

impl PlaybackState {
    /// Starts playing `timeline` from its beginning.
    pub(super) fn new(timeline: Rc<ReplayTimeline>) -> Self {
        Self {
            playback: Playback::playing_from(timeline.start(), performance_now()),
            timeline,
        }
    }
}

impl Reducible for PlaybackState {
    type Action = PlaybackCommand;

    /// Applies a command at the current local time.
    fn reduce(self: Rc<Self>, command: PlaybackCommand) -> Rc<Self> {
        let now = performance_now();
        let timeline = &self.timeline;
        let playback = self.playback;
        let playback = match command {
            PlaybackCommand::Toggle => playback.toggled(timeline, now),
            PlaybackCommand::Seek(elapsed) => {
                playback.seeked(timeline, timeline.start() + elapsed, now)
            }
            PlaybackCommand::Skip(offset) => playback.skipped(timeline, offset, now),
            PlaybackCommand::NextSpeed => playback.sped_up(timeline, now),
            PlaybackCommand::Pause => playback.paused(timeline, now),
        };
        Rc::new(Self {
            timeline: self.timeline.clone(),
            playback,
        })
    }
}
