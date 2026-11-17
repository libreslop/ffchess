//! Immutable configuration of one game instance.

use common::models::{GameModeClientConfig, GameModeConfig, ReplayHeader};
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

    /// Returns true when the matches of this setup are recorded for replays.
    ///
    /// Only private matches finish, so only they can be replayed.
    pub(super) fn records_replays(&self) -> bool {
        self.is_private && self.mode.allow_replays
    }

    /// Returns the rules a replay of this instance is played back with.
    pub(super) fn replay_header(&self) -> ReplayHeader {
        ReplayHeader {
            mode: self.client_mode.clone(),
            catalog: (*self.catalog).clone(),
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
