//! Recordings of finished matches, played back by the replay viewer.

use crate::models::state::{GameState, Piece, Player, Shop};
use crate::models::summary::GameModeClientConfig;
use crate::rules::Catalog;
use crate::types::{BoardSize, PieceId, TimestampMs};
use serde::{Deserialize, Serialize};

/// What a replay viewer needs before the first frame: the rules the match was played with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayHeader {
    pub mode: GameModeClientConfig,
    pub catalog: Catalog,
}

/// The changes of the board during one recorded tick.
///
/// Unchanged parts are left out, so a frame of a quiet tick is small. The first frame of a
/// replay holds the whole board.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplayFrame {
    /// Server time of the tick.
    pub at: TimestampMs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_size: Option<BoardSize>,
    /// Every player on the board, present whenever any player changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<Vec<Player>>,
    /// Every shop on the board, present whenever any shop changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shops: Option<Vec<Shop>>,
    /// Pieces that were placed or changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<Piece>,
    /// Pieces that left the board.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<PieceId>,
}

impl ReplayFrame {
    /// Returns the frame that turns `before` into `after` at `at`, or `None` when the board did
    /// not change.
    pub fn between(before: &GameState, after: &GameState, at: TimestampMs) -> Option<Self> {
        let frame = Self {
            at,
            board_size: (before.board_size != after.board_size).then_some(after.board_size),
            players: (before.players != after.players)
                .then(|| after.players.values().cloned().collect()),
            shops: (before.shops != after.shops).then(|| after.shops.clone()),
            pieces: after
                .pieces
                .values()
                .filter(|piece| before.pieces.get(&piece.id) != Some(*piece))
                .cloned()
                .collect(),
            removed: before
                .pieces
                .keys()
                .filter(|id| !after.pieces.contains_key(id))
                .copied()
                .collect(),
        };
        (!frame.is_empty()).then_some(frame)
    }

    /// Returns true when the frame changes nothing.
    fn is_empty(&self) -> bool {
        self.board_size.is_none()
            && self.players.is_none()
            && self.shops.is_none()
            && self.pieces.is_empty()
            && self.removed.is_empty()
    }

    /// Applies the changes of this frame to `state`.
    pub fn apply(&self, state: &mut GameState) {
        if let Some(board_size) = self.board_size {
            state.board_size = board_size;
        }
        if let Some(players) = &self.players {
            state.players = players.iter().map(|p| (p.id, p.clone())).collect();
        }
        if let Some(shops) = &self.shops {
            state.shops = shops.clone();
        }
        for id in &self.removed {
            state.pieces.remove(id);
        }
        for piece in &self.pieces {
            state.pieces.insert(piece.id, piece.clone());
        }
    }
}

/// A finished match as stored by the server: its rules and every change of its board.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Replay {
    pub header: ReplayHeader,
    /// Server time moves were unlocked, after the countdown; the board before is a wait.
    pub started_at: TimestampMs,
    /// Server time the match finished.
    pub ended_at: TimestampMs,
    /// Frames in the order they were recorded.
    pub frames: Vec<ReplayFrame>,
}

/// The board as it stood from one moment of a replay until the next.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayKeyframe {
    pub at: TimestampMs,
    pub state: GameState,
}

impl Replay {
    /// Returns the full board after every frame, in time order.
    pub fn keyframes(&self) -> Vec<ReplayKeyframe> {
        let mut state = GameState {
            mode_id: self.header.mode.id.clone(),
            ..GameState::default()
        };
        self.frames
            .iter()
            .map(|frame| {
                frame.apply(&mut state);
                ReplayKeyframe {
                    at: frame.at,
                    state: state.clone(),
                }
            })
            .collect()
    }
}
