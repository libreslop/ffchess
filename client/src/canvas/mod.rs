//! Canvas rendering utilities for the game board and pieces.

pub(crate) mod color;
mod move_hints;
pub mod pieces;
mod premove_arrows;
mod renderer;
mod shading;
pub mod types;

pub use types::Renderer;
pub use types::*;
