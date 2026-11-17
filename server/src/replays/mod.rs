//! Replays of finished matches, kept in a SQLite database for a day after the match ends.
//!
//! Matches record their board changes while they run. Frames go straight to the database
//! through a background writer, so a recording never builds up in memory. A replay is saved
//! when its match finishes, can only be viewed from then on, and is deleted
//! [`REPLAY_RETENTION`] after it was saved: overdue replays are cleaned up when the server
//! starts and every minute while it runs.

mod db;
mod recorder;
mod writer;

pub use recorder::ReplayRecorder;

use crate::time::now_ms;
use common::models::ReplayHeader;
use common::types::{DurationMs, ReplayId};
use db::ReplayDb;
use rusqlite::Connection;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use writer::WriteCommand;

/// How long a replay stays viewable after it was saved.
pub const REPLAY_RETENTION: DurationMs = DurationMs(24 * 60 * 60 * 1000);

/// File name of the replay database inside the data directory.
const DB_FILE_NAME: &str = "replays.sqlite3";

/// Handle to the replay database.
pub struct ReplayStore {
    db: Arc<Mutex<ReplayDb>>,
    writes: Sender<WriteCommand>,
}

impl ReplayStore {
    /// Opens the replay database in `dir`, creating both when missing.
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {dir:?}: {e}"))?;
        let path = dir.join(DB_FILE_NAME);
        let conn = Connection::open(&path).map_err(|e| format!("cannot open {path:?}: {e}"))?;
        Self::with_connection(conn).map_err(|e| format!("cannot prepare {path:?}: {e}"))
    }

    /// Opens a database that lives in memory only, for tests.
    pub fn in_memory() -> Self {
        Connection::open_in_memory()
            .and_then(Self::with_connection)
            .expect("in-memory replay database")
    }

    /// Prepares the database behind `conn`, deletes the replays that expired while the server
    /// was down, and starts the writer thread.
    fn with_connection(conn: Connection) -> rusqlite::Result<Self> {
        let db = Arc::new(Mutex::new(ReplayDb::new(conn)?));
        writer::prune(&db);
        let (writes, commands) = mpsc::channel();
        let writer_db = db.clone();
        std::thread::Builder::new()
            .name("replay-writer".to_string())
            .spawn(move || writer::run(writer_db, commands))
            .expect("spawn replay writer thread");
        Ok(Self { db, writes })
    }

    /// Starts recording a new match played with the rules in `header`.
    pub fn record(&self, header: &ReplayHeader) -> ReplayRecorder {
        ReplayRecorder::start(self.writes.clone(), header, now_ms())
    }

    /// Returns the JSON of a [`common::models::Replay`] saved within the retention period.
    ///
    /// Blocks on the database; call it off the async runtime.
    pub fn finished_replay(&self, id: ReplayId) -> Option<String> {
        let db = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        db.finished_replay(id, now_ms() - REPLAY_RETENTION)
            .inspect_err(|error| tracing::error!("Failed to read replay {id}: {error}"))
            .ok()
            .flatten()
    }
}
