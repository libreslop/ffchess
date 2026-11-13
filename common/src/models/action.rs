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
    /// Buy an item from the shop the piece is standing on.
    Buy { item: ShopItemIndex },
}

impl PieceAction {
    /// Returns true when the action must wait for the piece's movement cooldown.
    pub fn needs_cooldown(&self) -> bool {
        matches!(self, Self::Move { .. })
    }
}
