use crate::types::BoardCoord;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Position of an item within a resolved shop group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShopItemIndex(pub usize);

impl fmt::Display for ShopItemIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One thing a player can ask a single piece to do.
///
/// Actions are executed in order per piece; moves wait for the piece cooldown while purchases
/// run as soon as every earlier action of that piece has resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceAction {
    /// Move (or capture) to `target`.
    Move { target: BoardCoord },
    /// Guard the piece of the same owner standing on `target`: the capture of `target` waits
    /// until no piece of that owner stands there, so the guard takes back the square as soon
    /// as the guarded piece is captured. The guard fails once the square is left empty.
    ///
    /// A guard only stands while it is the last action of its piece; queuing anything after
    /// it supersedes it.
    Guard { target: BoardCoord },
    /// Buy an item from the shop the piece is standing on.
    Buy { item: ShopItemIndex },
}

impl PieceAction {
    /// Returns true when the action must wait for the piece's movement cooldown.
    pub fn needs_cooldown(&self) -> bool {
        self.move_target().is_some()
    }

    /// Returns the square the action moves its piece to, if it is a move or a guard.
    pub fn move_target(&self) -> Option<BoardCoord> {
        match self {
            Self::Move { target } | Self::Guard { target } => Some(*target),
            Self::Buy { .. } => None,
        }
    }
}
