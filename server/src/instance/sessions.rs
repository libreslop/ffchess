//! Rejoin secrets and respawn timers of players who passed through an instance.

use common::protocol::GameError;
use common::types::{DurationMs, PlayerId, SessionSecret, TimestampMs};
use std::collections::HashMap;

/// Unused session secrets expire after this long.
const SECRET_TTL: DurationMs = DurationMs(5 * 60 * 1000);

/// Death timestamps are forgotten after this long.
const DEATH_TTL: DurationMs = DurationMs(10 * 60 * 1000);

/// A session secret and the last time it was presented.
struct StoredSecret {
    secret: SessionSecret,
    last_used: TimestampMs,
}

/// Remembers per-player secrets and deaths so returning players can be recognized.
#[derive(Default)]
pub(super) struct SessionBook {
    secrets: HashMap<PlayerId, StoredSecret>,
    deaths: HashMap<PlayerId, TimestampMs>,
}

impl SessionBook {
    /// Checks the secret a player presents and returns the secret of their session.
    ///
    /// Unknown players get a fresh secret; known players must present the stored one.
    pub(super) fn authenticate(
        &mut self,
        player_id: PlayerId,
        provided: Option<SessionSecret>,
        now: TimestampMs,
    ) -> Result<SessionSecret, GameError> {
        match self.secrets.get_mut(&player_id) {
            Some(stored) if Some(stored.secret) != provided => Err(GameError::InvalidSession),
            Some(stored) => {
                stored.last_used = now;
                Ok(stored.secret)
            }
            None => {
                let secret = SessionSecret::new();
                self.secrets.insert(
                    player_id,
                    StoredSecret {
                        secret,
                        last_used: now,
                    },
                );
                Ok(secret)
            }
        }
    }

    /// Returns how much longer `player_id` must wait before respawning.
    pub(super) fn respawn_wait(
        &self,
        player_id: PlayerId,
        cooldown: DurationMs,
        now: TimestampMs,
    ) -> Option<DurationMs> {
        let elapsed = now - *self.deaths.get(&player_id)?;
        (elapsed < cooldown).then(|| cooldown - elapsed)
    }

    /// Records that `player_id` left the board at `now`.
    pub(super) fn record_death(&mut self, player_id: PlayerId, now: TimestampMs) {
        self.deaths.insert(player_id, now);
    }

    /// Clears the death record of a player who respawned.
    pub(super) fn clear_death(&mut self, player_id: PlayerId) {
        self.deaths.remove(&player_id);
    }

    /// Forgets stale secrets and deaths.
    pub(super) fn prune(&mut self, now: TimestampMs) {
        self.secrets
            .retain(|_, stored| now - stored.last_used <= SECRET_TTL);
        self.deaths.retain(|_, died_at| now - *died_at <= DEATH_TTL);
    }
}
