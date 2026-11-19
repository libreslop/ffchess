//! Move history of a replay: every moment a piece moved, grouped into rows of equal length.

use super::timeline::ReplayTimeline;
use common::models::{GameState, Piece};
use common::rules::Catalog;
use common::types::{BoardCoord, BoardSize, ColorHex, DurationMs, PieceTypeId, TimestampMs};
use std::ops::Range;

/// Length of the stretch of a replay one history row covers.
const ROW_SPAN: DurationMs = DurationMs(1000);

/// One piece moving, written in algebraic notation without the piece, which is shown by its
/// icon instead.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PieceMove {
    pub(super) notation: String,
    pub(super) piece_type: PieceTypeId,
    /// Color of the player owning the piece, or `None` for pieces nobody owns.
    pub(super) color: Option<ColorHex>,
}

impl PieceMove {
    /// Describes `piece` moving from where it stood on `before` to where it stands on `after`.
    fn new(before: &GameState, after: &GameState, piece: &Piece, catalog: &Catalog) -> Self {
        let from = before.pieces[&piece.id].position;
        let captured = before
            .piece_at(piece.position)
            .is_some_and(|other| other.id != piece.id);
        let size = after.board_size;
        // Like a pawn in chess notation, a piece drawn as a pawn names the file it left when
        // it takes.
        let drawn_as_pawn = catalog
            .pieces
            .get(&piece.piece_type)
            .is_some_and(|config| config.svg_path == "pawn.svg");
        let prefix = match drawn_as_pawn && captured {
            true => Square(from).file(size),
            false => String::new(),
        };
        let capture = if captured { "x" } else { "" };
        let color = piece.owner_id.and_then(|owner| {
            after
                .players
                .get(&owner)
                .or_else(|| before.players.get(&owner))
                .map(|player| player.color.clone())
        });
        Self {
            notation: format!("{prefix}{capture}{}", Square(piece.position).name(size)),
            piece_type: piece.piece_type.clone(),
            color,
        }
    }
}

/// A square named the chess way: files are letters from the left, ranks count up from the
/// bottom.
struct Square(BoardCoord);

impl Square {
    /// Returns the letters of the file, continuing `aa`, `ab`, ... past `z` on wide boards.
    fn file(&self, size: BoardSize) -> String {
        let mut index = (self.0.0.x + size.half()).max(0) as u32;
        let mut letters = Vec::new();
        loop {
            letters.push(char::from(b'a' + (index % 26) as u8));
            if index < 26 {
                break;
            }
            index = index / 26 - 1;
        }
        letters.into_iter().rev().collect()
    }

    /// Returns the number of the rank.
    fn rank(&self, size: BoardSize) -> i32 {
        size.limit_pos() - self.0.0.y
    }

    /// Returns the full name, such as `e4`.
    fn name(&self, size: BoardSize) -> String {
        format!("{}{}", self.file(size), self.rank(size))
    }
}

/// What happened at one moment of a replay.
#[derive(Debug, PartialEq)]
pub(super) enum HistoryEvent {
    /// Moves were unlocked.
    Start,
    /// Pieces moved, in the order they moved; moves made by one action, such as castling, are
    /// in the order of their notation.
    Moves(Vec<PieceMove>),
    /// The match finished.
    End,
}

/// A moment of the replay in which pieces moved, or its start or end.
#[derive(Debug, PartialEq)]
pub(super) struct HistoryEntry {
    pub(super) at: TimestampMs,
    pub(super) event: HistoryEvent,
}

/// Entries that fall into the same stretch of the replay.
#[derive(Debug, PartialEq)]
pub(super) struct HistoryRow {
    /// Time into the replay at which the stretch ends.
    pub(super) ends_after: DurationMs,
    /// Indices of the entries in the row.
    pub(super) entries: Range<EntryIndex>,
}

/// Index of an entry of the move history.
pub(super) type EntryIndex = usize;

/// A way to move through the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HistoryStep {
    PreviousMove,
    NextMove,
    /// To the nearest move before that ends a row.
    PreviousRow,
    /// To the nearest move after that ends a row.
    NextRow,
}

/// Every move of a replay, from the start on.
#[derive(Debug, PartialEq)]
pub(super) struct MoveHistory {
    /// Entries in time order; the first is the start of the replay and the last its end.
    entries: Vec<HistoryEntry>,
    /// Rows in time order, leaving out stretches without moves.
    rows: Vec<HistoryRow>,
}

impl MoveHistory {
    /// Collects the moves of `timeline`, naming pieces as `catalog` draws them.
    pub(super) fn new(timeline: &ReplayTimeline, catalog: &Catalog) -> Self {
        let mut entries = vec![HistoryEntry {
            at: timeline.start(),
            event: HistoryEvent::Start,
        }];
        // Each action is a frame of its own, so actions of one moment, such as a capture and
        // the guard taking the square back, join one entry in the order they were made.
        for (at, moves) in Self::frame_moves(timeline, catalog) {
            match entries.last_mut() {
                Some(HistoryEntry {
                    at: last_at,
                    event: HistoryEvent::Moves(last),
                }) if *last_at == at => last.extend(moves),
                _ => entries.push(HistoryEntry {
                    at,
                    event: HistoryEvent::Moves(moves),
                }),
            }
        }
        entries.push(HistoryEntry {
            at: timeline.end(),
            event: HistoryEvent::End,
        });
        let rows = Self::rows_of(&entries, timeline);
        Self { entries, rows }
    }

    /// Returns the pieces that moved in each frame from the start on, leaving out frames
    /// without moves.
    fn frame_moves<'a>(
        timeline: &'a ReplayTimeline,
        catalog: &'a Catalog,
    ) -> impl Iterator<Item = (TimestampMs, Vec<PieceMove>)> + 'a {
        timeline
            .keyframes_from_start()
            .windows(2)
            .filter_map(move |pair| {
                let (before, after) = (&pair[0].state, &pair[1].state);
                let mut moves: Vec<PieceMove> = after
                    .pieces
                    .values()
                    .filter(|piece| {
                        before
                            .pieces
                            .get(&piece.id)
                            .is_some_and(|old| old.position != piece.position)
                    })
                    .map(|piece| PieceMove::new(before, after, piece, catalog))
                    .collect();
                moves.sort_by(|a, b| a.notation.cmp(&b.notation));
                (!moves.is_empty()).then_some((pair[1].at, moves))
            })
    }

    /// Groups consecutive entries by the stretch of the replay they fall into.
    fn rows_of(entries: &[HistoryEntry], timeline: &ReplayTimeline) -> Vec<HistoryRow> {
        // The start alone makes the row ending at zero; moves in the first stretch make the
        // row ending one span in.
        let ends_after = |entry: &HistoryEntry| match entry.event {
            HistoryEvent::Start => DurationMs::zero(),
            _ => {
                let span = ROW_SPAN.as_i64();
                let elapsed = timeline.elapsed(entry.at).as_i64();
                DurationMs::from_millis((elapsed / span + 1) * span)
            }
        };
        let mut rows: Vec<HistoryRow> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let ends_after = ends_after(entry);
            match rows.last_mut() {
                Some(row) if row.ends_after == ends_after => row.entries.end = index + 1,
                _ => rows.push(HistoryRow {
                    ends_after,
                    entries: index..index + 1,
                }),
            }
        }
        rows
    }

    /// Returns every entry.
    pub(super) fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Returns every row.
    pub(super) fn rows(&self) -> &[HistoryRow] {
        &self.rows
    }

    /// Returns the latest entry at or before `at`.
    pub(super) fn entry_at(&self, at: TimestampMs) -> EntryIndex {
        self.entries
            .partition_point(|entry| entry.at <= at)
            .saturating_sub(1)
    }

    /// Returns the entry `step` leads to from `current`, staying put past either end.
    pub(super) fn step(&self, current: EntryIndex, step: HistoryStep) -> EntryIndex {
        let mut row_ends = self.rows.iter().map(|row| row.entries.end - 1);
        match step {
            HistoryStep::PreviousMove => current.saturating_sub(1),
            HistoryStep::NextMove => (current + 1).min(self.entries.len() - 1),
            HistoryStep::PreviousRow => row_ends.rfind(|&end| end < current).unwrap_or(current),
            HistoryStep::NextRow => row_ends.find(|&end| end > current).unwrap_or(current),
        }
    }
}
