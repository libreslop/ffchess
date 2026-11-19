//! Playback of a replay: where it is, how fast it goes, and the commands that move it.

use super::history::{EntryIndex, HistoryStep, MoveHistory};
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

    /// Returns twice this speed, staying at the fastest.
    fn faster(self) -> Self {
        Self((self.0 * 2).min(Self::MAX))
    }

    /// Returns half this speed, staying at real time.
    fn slower(self) -> Self {
        Self((self.0 / 2).max(Self::NORMAL.0))
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
    /// Stops at `at`, ready to play at normal speed.
    pub(super) fn paused_at(at: TimestampMs, now_local_ms: f64) -> Self {
        Self {
            anchor: at,
            anchor_local_ms: now_local_ms,
            speed: PlaybackSpeed::NORMAL,
            playing: false,
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

    /// Stops at `at`.
    pub(super) fn shown(
        self,
        timeline: &ReplayTimeline,
        at: TimestampMs,
        now_local_ms: f64,
    ) -> Self {
        self.anchored(timeline.clamp(at), now_local_ms, false)
    }

    /// Stops at the current position.
    pub(super) fn paused(self, timeline: &ReplayTimeline, now_local_ms: f64) -> Self {
        let at = self.position(timeline, now_local_ms);
        self.anchored(at, now_local_ms, false)
    }

    /// Switches to the speed `change` makes of the current one, without jumping.
    pub(super) fn with_speed(
        self,
        change: fn(PlaybackSpeed) -> PlaybackSpeed,
        timeline: &ReplayTimeline,
        now_local_ms: f64,
    ) -> Self {
        let at = self.position(timeline, now_local_ms);
        Self {
            speed: change(self.speed),
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
    /// Stops at an entry picked from the history.
    Show(EntryIndex),
    /// Stops at the move of the history a step away from the one shown.
    Step(HistoryStep),
    NextSpeed,
    Faster,
    Slower,
    /// Pauses; sent once playing reached the end.
    Pause,
}

/// A button of the playback controls, lit while a key or the wheel presses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaybackButton {
    Toggle,
    Speed,
    Step(HistoryStep),
}

/// Playback of one replay, as a reducer so every command sees the latest position.
#[derive(Debug, PartialEq)]
pub(super) struct PlaybackState {
    pub(super) timeline: Rc<ReplayTimeline>,
    pub(super) history: Rc<MoveHistory>,
    pub(super) playback: Playback,
    /// Entry of the history playback stopped on when it was picked or stepped to, which tells
    /// apart entries of the same moment, such as the last move and the end.
    shown: Option<EntryIndex>,
}

impl PlaybackState {
    /// Stops at the beginning of `timeline`, waiting to be played.
    pub(super) fn new(timeline: Rc<ReplayTimeline>, history: Rc<MoveHistory>) -> Self {
        Self {
            playback: Playback::paused_at(timeline.start(), performance_now()),
            timeline,
            history,
            shown: None,
        }
    }

    /// Returns the replay time shown at local time `now_local_ms`.
    pub(super) fn position(&self, now_local_ms: f64) -> TimestampMs {
        self.playback.position(&self.timeline, now_local_ms)
    }

    /// Returns the move of the history shown at local time `now_local_ms`.
    pub(super) fn current_entry(&self, now_local_ms: f64) -> EntryIndex {
        self.shown
            .unwrap_or_else(|| self.history.entry_at(self.position(now_local_ms)))
    }

    /// Returns the playback stopped on the entry `index` of the history, and that entry.
    fn shown_entry(&self, index: EntryIndex, now_local_ms: f64) -> (Playback, Option<EntryIndex>) {
        let at = self.history.entries()[index].at;
        let playback = self.playback.shown(&self.timeline, at, now_local_ms);
        (playback, Some(index))
    }
}

impl Reducible for PlaybackState {
    type Action = PlaybackCommand;

    /// Applies a command at the current local time.
    fn reduce(self: Rc<Self>, command: PlaybackCommand) -> Rc<Self> {
        let now = performance_now();
        let timeline = &self.timeline;
        let playback = self.playback;
        // Showing and stepping stop on an entry and changing speed stays on it; every other
        // command leaves the entry to follow from the position.
        let (playback, shown) = match command {
            PlaybackCommand::Show(index) => self.shown_entry(index, now),
            PlaybackCommand::Step(step) => {
                self.shown_entry(self.history.step(self.current_entry(now), step), now)
            }
            PlaybackCommand::Toggle => (playback.toggled(timeline, now), None),
            PlaybackCommand::Seek(elapsed) => (
                playback.seeked(timeline, timeline.start() + elapsed, now),
                None,
            ),
            PlaybackCommand::NextSpeed => (
                playback.with_speed(PlaybackSpeed::next, timeline, now),
                self.shown,
            ),
            PlaybackCommand::Faster => (
                playback.with_speed(PlaybackSpeed::faster, timeline, now),
                self.shown,
            ),
            PlaybackCommand::Slower => (
                playback.with_speed(PlaybackSpeed::slower, timeline, now),
                self.shown,
            ),
            PlaybackCommand::Pause => (playback.paused(timeline, now), None),
        };
        Rc::new(Self {
            timeline: self.timeline.clone(),
            history: self.history.clone(),
            playback,
            shown,
        })
    }
}
