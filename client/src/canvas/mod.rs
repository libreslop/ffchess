//! Canvas rendering utilities for the game board and pieces.

pub(crate) mod color;
mod fog;
mod move_blurs;
mod move_hints;
pub mod pieces;
mod premove_arrows;
mod renderer;
mod shading;
pub mod types;
mod wipe;

pub use types::Renderer;
pub use wipe::{BoardWipes, WipeTiming};
pub use types::*;
