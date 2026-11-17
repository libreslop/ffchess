//! SQLite schema and queries of the replay database.

use common::types::{ReplayId, TimestampMs};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::value::RawValue;

/// Creates the tables and tunes the connection for many small writes.
const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA foreign_keys = ON;
    CREATE TABLE IF NOT EXISTS replays (
        id TEXT PRIMARY KEY,
        header TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        -- When the match finished and the replay was saved; NULL while it is still recorded.
        saved_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS replays_saved_at ON replays (saved_at);
    CREATE TABLE IF NOT EXISTS replay_frames (
        replay_id TEXT NOT NULL REFERENCES replays (id) ON DELETE CASCADE,
        seq INTEGER NOT NULL,
        frame TEXT NOT NULL,
        PRIMARY KEY (replay_id, seq)
    ) WITHOUT ROWID;
";

/// Position of a frame within its replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameSeq(pub(super) u32);

impl FrameSeq {
    /// Returns the position after this one.
    pub(super) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A replay as it is sent to viewers, assembled from stored JSON without re-encoding it.
///
/// Serializes like [`common::models::Replay`].
#[derive(Serialize)]
struct StoredReplay {
    header: Box<RawValue>,
    ended_at: TimestampMs,
    frames: Vec<Box<RawValue>>,
}

/// The replay database behind one connection.
pub(super) struct ReplayDb {
    conn: Connection,
}

impl ReplayDb {
    /// Prepares `conn` for replays and drops recordings of matches that never finished, which
    /// a previous server run left behind.
    pub(super) fn new(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        conn.execute("DELETE FROM replays WHERE saved_at IS NULL", [])?;
        Ok(Self { conn })
    }

    /// Runs `f` inside one transaction, so a batch of writes costs one commit.
    pub(super) fn transaction<R>(
        &mut self,
        f: impl FnOnce(&ReplayDb) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<R> {
        self.conn.execute_batch("BEGIN")?;
        let result = f(self);
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        self.conn.execute_batch(end)?;
        result
    }

    /// Starts the recording of a match.
    pub(super) fn begin(
        &self,
        id: ReplayId,
        header: &str,
        started_at: TimestampMs,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO replays (id, header, started_at) VALUES (?1, ?2, ?3)",
            params![id.to_string(), header, started_at.as_i64()],
        )?;
        Ok(())
    }

    /// Appends one frame to a recording.
    pub(super) fn append(&self, id: ReplayId, seq: FrameSeq, frame: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO replay_frames (replay_id, seq, frame) VALUES (?1, ?2, ?3)",
            params![id.to_string(), seq.0, frame],
        )?;
        Ok(())
    }

    /// Saves a recording as complete at `ended_at`, which makes it viewable.
    pub(super) fn finish(&self, id: ReplayId, ended_at: TimestampMs) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE replays SET saved_at = ?2 WHERE id = ?1",
            params![id.to_string(), ended_at.as_i64()],
        )?;
        Ok(())
    }

    /// Deletes a recording and its frames.
    pub(super) fn discard(&self, id: ReplayId) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM replays WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    /// Deletes every replay saved before `cutoff`.
    pub(super) fn delete_saved_before(&self, cutoff: TimestampMs) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM replays WHERE saved_at < ?1",
            params![cutoff.as_i64()],
        )
    }

    /// Returns the JSON of a replay saved at or after `cutoff`.
    pub(super) fn finished_replay(
        &self,
        id: ReplayId,
        cutoff: TimestampMs,
    ) -> rusqlite::Result<Option<String>> {
        let key = id.to_string();
        let Some((header, ended_at)) = self
            .conn
            .query_row(
                "SELECT header, saved_at FROM replays WHERE id = ?1 AND saved_at >= ?2",
                params![key, cutoff.as_i64()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let frames = self
            .conn
            .prepare("SELECT frame FROM replay_frames WHERE replay_id = ?1 ORDER BY seq")?
            .query_map(params![key], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;

        // Stored JSON was written by the server itself, so it only fails to parse when the
        // database was edited by hand; such a replay is treated as missing.
        let raw = |json: String| RawValue::from_string(json).ok();
        let replay = raw(header)
            .zip(frames.into_iter().map(raw).collect::<Option<Vec<_>>>())
            .map(|(header, frames)| StoredReplay {
                header,
                ended_at: TimestampMs::from_millis(ended_at),
                frames,
            });
        Ok(replay.and_then(|replay| serde_json::to_string(&replay).ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a fresh in-memory database.
    fn db() -> ReplayDb {
        ReplayDb::new(Connection::open_in_memory().expect("open")).expect("schema")
    }

    /// Saves an empty replay at `saved_at`.
    fn save(db: &ReplayDb, saved_at: i64) -> ReplayId {
        let id = ReplayId::new();
        let at = TimestampMs::from_millis(saved_at);
        db.begin(id, "{}", at).expect("begin");
        db.append(id, FrameSeq(0), "{\"at\":0}").expect("frame");
        db.finish(id, at).expect("finish");
        id
    }

    #[test]
    /// Verifies replays saved before the cutoff are deleted along with their frames.
    fn overdue_replays_are_deleted() {
        let db = db();
        let old = save(&db, 1_000);
        let fresh = save(&db, 5_000);
        let cutoff = TimestampMs::from_millis(2_000);

        assert_eq!(db.delete_saved_before(cutoff).expect("prune"), 1);
        assert_eq!(db.finished_replay(old, cutoff).expect("read"), None);
        assert!(db.finished_replay(fresh, cutoff).expect("read").is_some());
        let frames: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM replay_frames", [], |row| row.get(0))
            .expect("count");
        assert_eq!(frames, 1, "frames of deleted replays go with them");
    }

    #[test]
    /// Verifies a recording is only viewable once it is saved.
    fn unsaved_replays_are_hidden() {
        let db = db();
        let id = ReplayId::new();
        db.begin(id, "{}", TimestampMs::from_millis(0)).expect("begin");
        assert_eq!(db.finished_replay(id, TimestampMs::from_millis(0)).expect("read"), None);
    }
}
