//! Game rules shared by the authoritative server and predicting clients.

mod catalog;
mod movement;
mod simulation;
mod special;

pub use catalog::Catalog;
pub use movement::{LegalMove, MoveKind};
pub use simulation::{ActionEffects, PieceMove, Simulation};
