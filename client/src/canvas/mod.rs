//! Canvas rendering utilities for the game board and pieces.

mod board_switch;
pub(crate) mod color;
mod fog;
mod move_blurs;
mod move_hints;
mod pieces;
mod premove_arrows;
mod renderer;
mod shading;
mod types;

pub use board_switch::{BoardSwitchTiming, BoardSwitches};
pub use types::*;
