use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

/// Total board dimension in tiles (square board).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct BoardSize(pub i32);

impl BoardSize {
    /// Creates a board size, clamping to at least 1 tile.
    pub fn new(value: i32) -> Self {
        Self(value.max(1))
    }

    /// Returns the raw tile dimension for the board.
    pub fn as_i32(self) -> i32 {
        self.0
    }

    /// Returns half the board size in tiles.
    pub fn half(self) -> i32 {
        self.0 / 2
    }

    /// Returns the exclusive coordinate limit for positive positions.
    pub fn limit_pos(self) -> i32 {
        (self.0 + 1) / 2
    }

    /// Returns true when `pos` lies on this board.
    ///
    /// Boards are centred on the origin and span `-half..limit_pos` on both axes.
    pub fn contains(self, pos: BoardCoord) -> bool {
        let range = -self.half()..self.limit_pos();
        range.contains(&pos.0.x) && range.contains(&pos.0.y)
    }
}

impl Default for BoardSize {
    fn default() -> Self {
        Self(40)
    }
}

impl From<i32> for BoardSize {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<BoardSize> for i32 {
    fn from(value: BoardSize) -> Self {
        value.0
    }
}

impl fmt::Display for BoardSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A coordinate on the game board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct BoardCoord(pub IVec2);

impl BoardCoord {
    /// Creates a new board coordinate.
    pub fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }

    /// Returns the underlying `IVec2`.
    pub fn as_ivec2(self) -> IVec2 {
        self.0
    }

    /// Returns the Chebyshev (king-move) distance between two coordinates.
    pub fn chebyshev_distance(self, other: BoardCoord) -> i32 {
        (self.0 - other.0).abs().max_element()
    }

    /// Returns the squared Euclidean distance between two coordinates.
    pub fn distance_squared(self, other: BoardCoord) -> i32 {
        (self.0 - other.0).length_squared()
    }
}

impl Sub<BoardCoord> for BoardCoord {
    type Output = IVec2;

    fn sub(self, rhs: BoardCoord) -> Self::Output {
        self.0 - rhs.0
    }
}

impl Add<IVec2> for BoardCoord {
    type Output = BoardCoord;

    fn add(self, rhs: IVec2) -> Self::Output {
        BoardCoord(self.0 + rhs)
    }
}

impl From<IVec2> for BoardCoord {
    fn from(v: IVec2) -> Self {
        Self(v)
    }
}

impl From<BoardCoord> for IVec2 {
    fn from(coord: BoardCoord) -> Self {
        coord.0
    }
}

impl fmt::Display for BoardCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.0.x, self.0.y)
    }
}
