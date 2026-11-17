//! Whether a match is still played, and what happens when it finishes.

use super::world::World;
use common::protocol::ServerMessage;
use common::types::TimestampMs;

/// Progress of a match.
///
/// Shared worlds never finish. A private match finishes once no player competes in it anymore,
/// such as when one side of a duel won, and stays finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStatus {
    Running,
    Finished { at: TimestampMs },
}

impl MatchStatus {
    /// Returns true once the match finished.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Finished { .. })
    }
}

impl World {
    /// Records the board of this tick into the replay, while the match is recorded.
    pub(super) fn record_replay_frame(&mut self, now: TimestampMs) {
        if let Some(recorder) = &mut self.replay {
            recorder.record(&self.state, now);
        }
    }

    /// Finishes a private match once nobody competes in it anymore.
    ///
    /// Everyone still on the board learns the match finished and where its replay is. The
    /// spectators are then released, so watching never keeps a finished match alive.
    pub(super) fn finish_if_over(&mut self, now: TimestampMs) {
        let over = self.setup.is_private && self.active_player_count() == 0;
        if self.status.is_finished() || !over {
            return;
        }
        self.status = MatchStatus::Finished { at: now };
        let replay = self.replay.take().map(|recorder| recorder.finish(now));
        self.outbox
            .broadcast(&ServerMessage::MatchFinished { replay });
        self.release_spectators();
    }
}
