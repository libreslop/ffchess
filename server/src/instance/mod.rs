//! Game instance submodules split by responsibility.

mod captures;
mod game_instance;
mod hooks;
mod moves;
mod npcs;
mod players;
mod shops;
mod ticks;

pub use game_instance::GameInstance;
