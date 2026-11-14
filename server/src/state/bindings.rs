//! Links between connections and the players they control.

use crate::instance::GameInstance;
use common::types::PlayerId;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The player a connection controls and the instance that player is in.
#[derive(Clone)]
pub(super) struct Binding {
    pub(super) player_id: PlayerId,
    pub(super) instance: Arc<GameInstance>,
}

/// The player a connection controls, readable without the lobby lock.
///
/// Moves only need this, so they skip the lobby lock that every other request takes. The lock
/// order is lobby before slot, and the slot is never held while calling into an instance.
#[derive(Clone, Default)]
pub(super) struct BindingSlot(Arc<Mutex<Option<Binding>>>);

impl BindingSlot {
    /// Locks the slot, ignoring poisoning like the other server locks do.
    fn lock(&self) -> MutexGuard<'_, Option<Binding>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the current binding.
    pub(super) fn get(&self) -> Option<Binding> {
        self.lock().clone()
    }

    /// Links the connection to the player it controls.
    pub(super) fn bind(&self, player_id: PlayerId, instance: Arc<GameInstance>) {
        *self.lock() = Some(Binding {
            player_id,
            instance,
        });
    }

    /// Unlinks the connection, returning the binding it had.
    pub(super) fn take(&self) -> Option<Binding> {
        self.lock().take()
    }

    /// Removes the player of the connection from its match so it can join again elsewhere.
    pub(super) fn detach(&self) {
        if let Some(binding) = self.take() {
            binding.instance.detach(binding.player_id);
        }
    }
}
