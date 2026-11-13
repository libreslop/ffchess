use crate::types::{
    BoardCoord, BoardSize, ColorHex, DurationMs, ModeId, MoveCount, PieceId, PieceTypeId, PlayerId,
    Score, ShopId, TimestampMs,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mutable piece state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub id: PieceId,
    /// Owning player, or `None` for NPCs.
    pub owner_id: Option<PlayerId>,
    pub piece_type: PieceTypeId,
    pub position: BoardCoord,
    pub last_move_time: TimestampMs,
    pub cooldown_ms: DurationMs,
    /// Moves made since the piece was placed, including moves made as a special move partner.
    #[serde(default)]
    pub moves_made: MoveCount,
}

impl Piece {
    /// Creates a piece that is ready to move immediately.
    pub fn new(owner_id: Option<PlayerId>, piece_type: PieceTypeId, position: BoardCoord) -> Self {
        Self {
            id: PieceId::new(),
            owner_id,
            piece_type,
            position,
            last_move_time: TimestampMs::from_millis(0),
            cooldown_ms: DurationMs::zero(),
            moves_made: MoveCount::default(),
        }
    }

    /// Returns true when the piece's cooldown has elapsed at `now`.
    pub fn is_ready(&self, now: TimestampMs) -> bool {
        now - self.last_move_time >= self.cooldown_ms
    }

    /// Returns true when the piece is controlled by a player rather than the server.
    pub fn is_player_owned(&self) -> bool {
        self.owner_id.is_some()
    }
}

/// Player state for the active match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub score: Score,
    pub kills: u32,
    pub pieces_captured: u32,
    pub join_time: TimestampMs,
    pub king_id: PieceId,
    pub color: ColorHex,
    #[serde(default)]
    pub board_rotation_deg: i32,
}

impl Player {
    /// Returns true when this player views the board upside down.
    pub fn is_board_rotated_180(&self) -> bool {
        self.board_rotation_deg.rem_euclid(360) == 180
    }
}

/// Shop state on the board.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub position: BoardCoord,
    pub uses_remaining: u32,
    pub shop_id: ShopId,
}

/// Snapshot of the board state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub players: HashMap<PlayerId, Player>,
    pub pieces: HashMap<PieceId, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: BoardSize,
    pub mode_id: ModeId,
}

impl Default for GameState {
    /// Creates an empty game state using the default board and mode.
    fn default() -> Self {
        Self {
            players: HashMap::new(),
            pieces: HashMap::new(),
            shops: Vec::new(),
            board_size: BoardSize::default(),
            mode_id: ModeId::from("ffa"),
        }
    }
}

impl GameState {
    /// Returns the piece standing on `pos`, if any.
    pub fn piece_at(&self, pos: BoardCoord) -> Option<&Piece> {
        self.pieces.values().find(|piece| piece.position == pos)
    }

    /// Returns the shop placed on `pos`, if any.
    pub fn shop_at(&self, pos: BoardCoord) -> Option<&Shop> {
        self.shops.iter().find(|shop| shop.position == pos)
    }

    /// Returns true when `pos` is on the board and holds neither a piece nor a shop.
    pub fn is_free(&self, pos: BoardCoord) -> bool {
        self.board_size.contains(pos) && self.piece_at(pos).is_none() && self.shop_at(pos).is_none()
    }

    /// Iterates over the pieces owned by `player_id`.
    pub fn pieces_of(&self, player_id: PlayerId) -> impl Iterator<Item = &Piece> {
        self.pieces
            .values()
            .filter(move |piece| piece.owner_id == Some(player_id))
    }

    /// Returns the king of `player_id` while it is still on the board.
    pub fn king_of(&self, player_id: PlayerId) -> Option<&Piece> {
        let player = self.players.get(&player_id)?;
        self.pieces.get(&player.king_id)
    }

    /// Removes every piece matching `predicate` and returns the removed ids.
    pub fn remove_pieces_where(&mut self, predicate: impl Fn(&Piece) -> bool) -> Vec<PieceId> {
        let removed: Vec<PieceId> = self
            .pieces
            .values()
            .filter(|piece| predicate(piece))
            .map(|piece| piece.id)
            .collect();
        for id in &removed {
            self.pieces.remove(id);
        }
        removed
    }

    /// Removes pieces and shops that are outside the current board bounds.
    ///
    /// Returns the ids of removed pieces.
    pub fn prune_out_of_bounds(&mut self) -> Vec<PieceId> {
        let board_size = self.board_size;
        self.shops.retain(|shop| board_size.contains(shop.position));
        self.remove_pieces_where(|piece| !board_size.contains(piece.position))
    }
}
