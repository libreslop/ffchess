//! Game rules shared by the authoritative server and predicting clients.

mod catalog;
mod movement;
mod shading;
mod simulation;
mod special;
mod threats;

pub use catalog::Catalog;
pub use movement::{LegalMove, MoveKind};
pub use shading::{PaintColor, Shade, ShadeMark};
pub use simulation::{ActionEffects, PieceMove, Simulation};
pub use threats::ThreatMap;
