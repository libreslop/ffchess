use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Sub;

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

    /// Returns the inclusive coordinate limit for positive positions.
    pub fn limit_pos(self) -> i32 {
        (self.0 + 1) / 2
    }
}

impl Default for BoardSize {
    /// Provides the default board size.
    fn default() -> Self {
        Self(40)
    }
}

impl From<i32> for BoardSize {
    /// Wraps a raw tile size as a board size.
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<BoardSize> for i32 {
    /// Unwraps a board size into its raw tile dimension.
    fn from(value: BoardSize) -> Self {
        value.0
    }
}

impl fmt::Display for BoardSize {
    /// Formats the board size as a decimal string.
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
}

impl PartialEq<IVec2> for BoardCoord {
    fn eq(&self, other: &IVec2) -> bool {
        self.0 == *other
    }
}

impl PartialEq<BoardCoord> for IVec2 {
    fn eq(&self, other: &BoardCoord) -> bool {
        *self == other.0
    }
}

impl Sub<IVec2> for BoardCoord {
    type Output = IVec2;

    fn sub(self, rhs: IVec2) -> Self::Output {
        self.0 - rhs
    }
}

impl Sub<BoardCoord> for BoardCoord {
    type Output = IVec2;

    fn sub(self, rhs: BoardCoord) -> Self::Output {
        self.0 - rhs.0
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
