//! Recording of one running match.

use super::db::FrameSeq;
use super::writer::WriteCommand;
use common::models::{GameState, ReplayFrame, ReplayHeader};
use common::types::{ReplayId, TimestampMs};
use std::sync::mpsc::Sender;

/// Records the board of one match, frame by frame, into the replay database.
///
/// Only the board as of the last frame stays in memory, to tell what changed since. A recorder
/// dropped before [`Self::finish`] deletes its recording, since the match never ended.
pub struct ReplayRecorder {
    id: ReplayId,
    writes: Sender<WriteCommand>,
    recorded: GameState,
    /// Time of the last frame, which later frames never go back before.
    recorded_at: TimestampMs,
    next_seq: FrameSeq,
    finished: bool,
}

impl ReplayRecorder {
    /// Starts a recording at `now`.
    pub(super) fn start(
        writes: Sender<WriteCommand>,
        header: &ReplayHeader,
        now: TimestampMs,
    ) -> Self {
        let id = ReplayId::new();
        match serde_json::to_string(header) {
            Ok(header) => Self::queue(
                &writes,
                WriteCommand::Begin {
                    id,
                    header,
                    started_at: now,
                },
            ),
            Err(error) => tracing::error!("Failed to encode replay header: {error}"),
        }
        Self {
            id,
            writes,
            recorded: GameState::default(),
            recorded_at: now,
            next_seq: FrameSeq(0),
            finished: false,
        }
    }

    /// Records the changes of the board since the last frame, if any.
    ///
    /// Times are read before the match is locked, so one taken a little earlier can arrive
    /// after a later one; frames keep their order by never going back in time.
    pub fn record(&mut self, state: &GameState, now: TimestampMs) {
        let at = now.max(self.recorded_at);
        let Some(frame) = ReplayFrame::between(&self.recorded, state, at) else {
            return;
        };
        self.recorded = state.clone();
        self.recorded_at = at;
        match serde_json::to_string(&frame) {
            Ok(frame) => {
                let seq = self.next_seq;
                self.next_seq = seq.next();
                self.send(WriteCommand::Frame {
                    id: self.id,
                    seq,
                    frame,
                });
            }
            Err(error) => {
                tracing::error!("Failed to encode a frame of replay {}: {error}", self.id)
            }
        }
    }

    /// Completes the recording at `now`, making it viewable, and returns its id.
    ///
    /// `started_at` is when moves were unlocked, or `None` when the match had no countdown and
    /// started as the recording began.
    pub fn finish(mut self, started_at: Option<TimestampMs>, now: TimestampMs) -> ReplayId {
        self.finished = true;
        self.send(WriteCommand::Finish {
            id: self.id,
            started_at,
            ended_at: now.max(self.recorded_at),
        });
        self.id
    }

    /// Queues a write of this recording.
    fn send(&self, command: WriteCommand) {
        Self::queue(&self.writes, command);
    }

    /// Queues a write, logging when the writer thread is gone.
    fn queue(writes: &Sender<WriteCommand>, command: WriteCommand) {
        if writes.send(command).is_err() {
            tracing::error!("Replay writer stopped; a replay write was lost");
        }
    }
}

impl Drop for ReplayRecorder {
    /// Deletes the recording of a match that ended without finishing.
    fn drop(&mut self) {
        if !self.finished {
            self.send(WriteCommand::Discard { id: self.id });
        }
    }
}
