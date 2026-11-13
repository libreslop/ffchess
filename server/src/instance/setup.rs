//! Immutable configuration of one game instance.

use common::models::{GameModeClientConfig, GameModeConfig};
use common::rules::Catalog;
use common::types::{ModeId, PlayerCount};
use std::sync::Arc;
use uuid::Uuid;

/// Configuration an instance is created with and never changes afterwards.
pub(super) struct InstanceSetup {
    /// Rules of this instance; private matches use a copy with their own id.
    pub(super) mode: GameModeConfig,
    /// Id of the public mode this instance belongs to.
    pub(super) public_mode_id: ModeId,
    /// Mode config sent to clients, always describing the public mode.
    pub(super) client_mode: GameModeClientConfig,
    pub(super) catalog: Arc<Catalog>,
    /// True for matches created from a matchmaking queue.
    pub(super) is_private: bool,
}

impl InstanceSetup {
    /// Builds the setup of the shared public instance of `mode`.
    pub(super) fn public(mode: GameModeConfig, catalog: Arc<Catalog>) -> Self {
        Self {
            public_mode_id: mode.id.clone(),
            client_mode: mode.to_client_config(),
            mode,
            catalog,
            is_private: false,
        }
    }

    /// Builds the setup of a fresh private match of the queue mode `template`.
    pub(super) fn private_match(template: &GameModeConfig, catalog: Arc<Catalog>) -> Self {
        let mut mode = template.clone();
        mode.id = ModeId::from(format!("{}__{}", template.id, Uuid::new_v4()));
        mode.queue_players = PlayerCount::zero();
        Self {
            mode,
            public_mode_id: template.id.clone(),
            client_mode: template.to_client_config(),
            catalog,
            is_private: true,
        }
    }

    /// Returns the chat room key represented by this instance.
    pub(super) fn chat_room_key(&self) -> String {
        if self.is_private {
            format!("game:{}", self.mode.id)
        } else {
            format!("mode:{}", self.public_mode_id)
        }
    }
}
