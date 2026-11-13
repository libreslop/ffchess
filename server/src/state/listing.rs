//! Mode list shown on the landing screen.

use super::lobby::Lobby;
use common::models::ModeSummary;
use common::types::{ModeId, PlayerCount};

impl Lobby {
    /// Returns the current status of every public mode.
    pub(super) fn mode_summaries(&self) -> Vec<ModeSummary> {
        self.games
            .values()
            .filter(|instance| !instance.is_private())
            .map(|instance| {
                let mode_id = instance.public_mode_id();
                let queue_size = self.mode(mode_id).and_then(|mode| mode.queue_requirement());
                ModeSummary {
                    id: mode_id.clone(),
                    display_name: instance.display_name().to_string(),
                    players: self.population(mode_id),
                    max_players: queue_size.unwrap_or_else(|| instance.max_players()),
                    queue_players: queue_size.unwrap_or_else(PlayerCount::zero),
                    respawn_cooldown_ms: instance.respawn_cooldown(),
                }
            })
            .collect()
    }

    /// Counts the players of a mode: queued plus playing for queue modes.
    fn population(&self, mode_id: &ModeId) -> PlayerCount {
        if self.is_queue_mode(mode_id) {
            self.instances_of(mode_id)
                .filter(|instance| instance.is_private())
                .fold(self.queue_len(mode_id), |total, instance| {
                    total + instance.player_count()
                })
        } else {
            self.public_instance(mode_id)
                .map_or_else(PlayerCount::zero, |instance| instance.player_count())
        }
    }
}
