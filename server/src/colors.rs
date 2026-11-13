//! Color assignment and reuse logic for player identities.

use common::types::{ColorHex, PLAYER_COLORS, PlayerId, TimestampMs};
use rand::Rng;
use std::collections::HashMap;

/// How long a released color stays reserved for its previous owner, in seconds.
const CLAIM_SECS: i64 = 60;

/// How long inactive players keep their remembered color, in seconds.
const MEMORY_SECS: i64 = 24 * 60 * 60;

/// Attempts at finding an unused random color before giving up on uniqueness.
const RANDOM_ATTEMPTS: usize = 100;

/// Tracks and assigns player colors while avoiding recent duplicates.
#[derive(Default)]
pub struct ColorManager {
    player_colors: HashMap<PlayerId, ColorHex>,
    color_last_active: HashMap<ColorHex, i64>,
    player_last_active: HashMap<PlayerId, i64>,
}

impl ColorManager {
    /// Creates a new empty color manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the player's color, assigning one if needed.
    ///
    /// Colors held by `active_player_ids` are never handed out twice. A player's previous color
    /// is reused when free, then unclaimed preferred colors, then random ones.
    pub fn get_or_assign_color(
        &mut self,
        player_id: PlayerId,
        active_player_ids: &[PlayerId],
        now: TimestampMs,
    ) -> ColorHex {
        let now = now.as_i64() / 1000;
        let active_colors: Vec<ColorHex> = active_player_ids
            .iter()
            .filter(|id| **id != player_id)
            .filter_map(|id| self.player_colors.get(id).cloned())
            .collect();
        let is_free = |color: &ColorHex| !active_colors.contains(color);
        let is_unclaimed = |color: &ColorHex| {
            self.color_last_active
                .get(color)
                .is_none_or(|last| now - last >= CLAIM_SECS)
        };

        let mut rng = rand::thread_rng();
        let mut random_color = || ColorHex::from(format!("#{:06x}", rng.gen_range(0..0x1000000)));
        let preferred = || PLAYER_COLORS.iter().map(|c| ColorHex::from(*c));

        let color = self
            .player_colors
            .get(&player_id)
            .filter(|color| is_free(color))
            .cloned()
            .or_else(|| preferred().find(|c| is_free(c) && is_unclaimed(c)))
            .or_else(|| {
                (0..RANDOM_ATTEMPTS)
                    .map(|_| random_color())
                    .find(|c| is_free(c) && is_unclaimed(c))
            })
            .or_else(|| preferred().find(is_free))
            .unwrap_or_else(random_color);

        self.player_colors.insert(player_id, color.clone());
        self.color_last_active.insert(color.clone(), now);
        self.player_last_active.insert(player_id, now);
        color
    }

    /// Marks a player (and their color) as recently active.
    pub fn update_activity(&mut self, player_id: PlayerId, now: TimestampMs) {
        let now = now.as_i64() / 1000;
        if let Some(color) = self.player_colors.get(&player_id) {
            self.color_last_active.insert(color.clone(), now);
        }
        self.player_last_active.insert(player_id, now);
    }

    /// Forgets players and color claims that have been inactive for a day.
    pub fn cleanup(&mut self, now: TimestampMs) {
        let now = now.as_i64() / 1000;
        let player_colors = &mut self.player_colors;
        self.player_last_active.retain(|id, last_active| {
            let keep = now - *last_active <= MEMORY_SECS;
            if !keep {
                player_colors.remove(id);
            }
            keep
        });
        self.color_last_active
            .retain(|_, last_active| now - *last_active <= MEMORY_SECS);
    }
}
