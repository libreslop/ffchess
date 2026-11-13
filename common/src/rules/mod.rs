//! Game rules shared by the authoritative server and predicting clients.

mod catalog;
mod movement;
mod simulation;

pub use catalog::Catalog;
pub use movement::MoveKind;
pub use simulation::{ActionEffects, Simulation};
