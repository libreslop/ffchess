//! Background thread that writes recordings to the replay database.
//!
//! Game ticks only queue writes, so a slow disk never holds up a match.

use super::REPLAY_RETENTION;
use super::db::{FrameSeq, ReplayDb};
use crate::time::now_ms;
use common::types::{ReplayId, TimestampMs};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Expired replays are deleted about this often.
const PRUNE_INTERVAL: Duration = Duration::from_secs(60);

/// One queued change of the replay database.
pub(super) enum WriteCommand {
    Begin {
        id: ReplayId,
        header: String,
        started_at: TimestampMs,
    },
    Frame {
        id: ReplayId,
        seq: FrameSeq,
        frame: String,
    },
    Finish {
        id: ReplayId,
        /// When moves were unlocked, if later than the start of the recording.
        started_at: Option<TimestampMs>,
        ended_at: TimestampMs,
    },
    Discard {
        id: ReplayId,
    },
}

impl WriteCommand {
    /// Applies this change to the database.
    fn apply(&self, db: &ReplayDb) -> rusqlite::Result<()> {
        match self {
            Self::Begin {
                id,
                header,
                started_at,
            } => db.begin(*id, header, *started_at),
            Self::Frame { id, seq, frame } => db.append(*id, *seq, frame),
            Self::Finish {
                id,
                started_at,
                ended_at,
            } => db.finish(*id, *started_at, *ended_at),
            Self::Discard { id } => db.discard(*id),
        }
    }
}

/// Writes queued commands until every sender is gone, deleting expired replays on the way.
///
/// The store already pruned once when it opened, so the first periodic prune waits a full
/// interval.
pub(super) fn run(db: Arc<Mutex<ReplayDb>>, commands: Receiver<WriteCommand>) {
    let mut last_prune = Instant::now();
    loop {
        if last_prune.elapsed() >= PRUNE_INTERVAL {
            last_prune = Instant::now();
            prune(&db);
        }
        match commands.recv_timeout(PRUNE_INTERVAL) {
            Ok(first) => {
                // Everything queued meanwhile goes into the same transaction.
                let batch: Vec<WriteCommand> =
                    std::iter::once(first).chain(commands.try_iter()).collect();
                write_batch(&db, &batch);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Applies a batch of commands in one transaction.
///
/// A failing command is skipped, so it cannot take the other matches' writes down with it.
fn write_batch(db: &Mutex<ReplayDb>, batch: &[WriteCommand]) {
    let mut db = db.lock().unwrap_or_else(PoisonError::into_inner);
    let written = db.transaction(|db| {
        for command in batch {
            if let Err(error) = command.apply(db) {
                tracing::error!("Failed to write a replay: {error}");
            }
        }
        Ok(())
    });
    if let Err(error) = written {
        tracing::error!("Failed to commit replays: {error}");
    }
}

/// Deletes replays saved longer ago than the retention period.
pub(super) fn prune(db: &Mutex<ReplayDb>) {
    let db = db.lock().unwrap_or_else(PoisonError::into_inner);
    match db.delete_saved_before(now_ms() - REPLAY_RETENTION) {
        Ok(0) => {}
        Ok(deleted) => tracing::info!("Deleted {deleted} expired replays"),
        Err(error) => tracing::error!("Failed to delete expired replays: {error}"),
    }
}
