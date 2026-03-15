//! Color assignment and reuse logic for player identities.

use common::types::{ColorHex, PlayerId};
use rand::Rng;
use std::collections::HashMap;

/// Ordered list of preferred color hex values for player assignment.
pub const PREFERRED_COLORS: &[&str] = &[
    "#dc2626", // Red
    "#2563eb", // Blue
    "#16a34a", // Green
    "#db2777", // Pink
    "#0891b2", // Cyan
    "#d97706", // Orange
    "#9333ea", // Purple
    "#ca8a04", // Yellow
    "#4d7c0f", // Lime
    "#b91c1c", // Dark Red
    "#1d4ed8", // Dark Blue
    "#15803d", // Dark Green
    "#ea580c", // Dark Orange
];

/// Tracks and assigns player colors while avoiding recent duplicates.
pub struct ColorManager {
    pub player_colors: HashMap<PlayerId, ColorHex>,
    pub color_last_active: HashMap<ColorHex, i64>,
    pub player_last_active: HashMap<PlayerId, i64>,
}

impl ColorManager {
    /// Creates a new empty color manager.
    pub fn new() -> Self {
        Self {
            player_colors: HashMap::new(),
            color_last_active: HashMap::new(),
            player_last_active: HashMap::new(),
        }
    }

    /// Returns the player's color, assigning one if needed.
    ///
    /// `player_id` identifies the player, `active_player_ids` lists currently active players.
    /// Returns the assigned `ColorHex`.
    pub fn get_or_assign_color(
        &mut self,
        player_id: PlayerId,
        active_player_ids: &[PlayerId],
    ) -> ColorHex {
        let now = chrono::Utc::now().timestamp();

        // Identify currently active colors.
        let active_colors: Vec<ColorHex> = active_player_ids
            .iter()
            .filter(|&id| *id != player_id)
            .filter_map(|id| self.player_colors.get(id).cloned())
            .collect();

        // 1. Re-use player's color if it's NOT active.
        if let Some(color) = self.player_colors.get(&player_id).cloned() {
            if !active_colors.contains(&color) {
                tracing::info!(?player_id, ?color, "Re-using player's previous color");
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                self.player_last_active.insert(player_id, now);
                return color;
            } else {
                tracing::info!(
                    ?player_id,
                    ?color,
                    "Player's previous color is currently active, assigning new one"
                );
            }
        }

        // 2. Try to find a preferred color that is NOT active and NOT claimed.
        // If a color is expired, it is NO LONGER claimed.
        for &c in PREFERRED_COLORS {
            let color = ColorHex::from(c);
            if !active_colors.contains(&color) {
                let last_active = self.color_last_active.get(&color);
                let claimed = last_active.is_some_and(|&last| now - last < 60);
                if !claimed {
                    tracing::info!(?player_id, ?color, "Assigning unclaimed preferred color");
                    self.player_colors.insert(player_id, color.clone());
                    self.color_last_active.insert(color.clone(), now);
                    self.player_last_active.insert(player_id, now);
                    return color;
                }
            }
        }

        // 3. Random color that is NOT active and NOT claimed.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let color = ColorHex::from(format!("#{:06x}", rng.gen_range(0..0x1000000)));
            if !active_colors.contains(&color) {
                let last_active = self.color_last_active.get(&color);
                let claimed = last_active.is_some_and(|&last| now - last < 60);
                if !claimed {
                    tracing::info!(?player_id, ?color, "Assigning random color");
                    self.player_colors.insert(player_id, color.clone());
                    self.color_last_active.insert(color.clone(), now);
                    self.player_last_active.insert(player_id, now);
                    return color;
                }
            }
        }

        // Fallback to anything not active if we are really crowded
        for &c in PREFERRED_COLORS {
            let color = ColorHex::from(c);
            if !active_colors.contains(&color) {
                tracing::info!(?player_id, ?color, "Assigning fallback preferred color");
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                self.player_last_active.insert(player_id, now);
                return color;
            }
        }

        // Ultimate fallback
        let color = ColorHex::from(format!("#{:06x}", rng.gen_range(0..0x1000000)));
        tracing::info!(
            ?player_id,
            ?color,
            "Assigning ultimate fallback random color"
        );
        self.player_colors.insert(player_id, color.clone());
        self.color_last_active.insert(color.clone(), now);
        self.player_last_active.insert(player_id, now);
        color
    }

    /// Marks a player (and their color) as recently active.
    ///
    /// `player_id` identifies the active player. Returns nothing.
    pub fn update_activity(&mut self, player_id: PlayerId) {
        let now = chrono::Utc::now().timestamp();
        if let Some(color) = self.player_colors.get(&player_id).cloned() {
            self.color_last_active.insert(color, now);
        }
        self.player_last_active.insert(player_id, now);
    }

    /// Removes inactive players and color claims older than `max_age_secs`.
    ///
    /// `now` is the current timestamp in seconds. Returns nothing.
    pub fn cleanup(&mut self, now: i64, max_age_secs: i64) {
        self.player_last_active.retain(|id, last_active| {
            if now - *last_active > max_age_secs {
                self.player_colors.remove(id);
                false
            } else {
                true
            }
        });

        self.color_last_active
            .retain(|_, last_active| now - *last_active <= max_age_secs);
    }
}

impl Default for ColorManager {
    /// Provides a new empty color manager.
    fn default() -> Self {
        Self::new()
    }
}
