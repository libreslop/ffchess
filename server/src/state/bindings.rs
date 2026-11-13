//! Links between connections and the players they control.

use crate::instance::GameInstance;
use common::types::PlayerId;
use std::sync::Arc;

/// The player a connection controls and the instance that player is in.
#[derive(Clone)]
pub(super) struct Binding {
    pub(super) player_id: PlayerId,
    pub(super) instance: Arc<GameInstance>,
}
