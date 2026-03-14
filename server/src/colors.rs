use rand::Rng;
use std::collections::HashMap;
use uuid::Uuid;

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

pub struct ColorManager {
    pub player_colors: HashMap<Uuid, String>,
    pub color_last_active: HashMap<String, i64>,
    pub player_last_active: HashMap<Uuid, i64>,
}

impl ColorManager {
    pub fn new() -> Self {
        Self {
            player_colors: HashMap::new(),
            color_last_active: HashMap::new(),
            player_last_active: HashMap::new(),
        }
    }

    pub fn get_or_assign_color(&mut self, player_id: Uuid, active_player_ids: &[Uuid]) -> String {
        let now = chrono::Utc::now().timestamp();

        // Identify currently active colors.
        let active_colors: Vec<String> = active_player_ids
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
            let color = c.to_string();
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
            let color = format!("#{:06x}", rng.gen_range(0..0x1000000));
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
            let color = c.to_string();
            if !active_colors.contains(&color) {
                tracing::info!(?player_id, ?color, "Assigning fallback preferred color");
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                self.player_last_active.insert(player_id, now);
                return color;
            }
        }

        // Ultimate fallback
        let color = format!("#{:06x}", rng.gen_range(0..0x1000000));
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

    pub fn update_activity(&mut self, player_id: Uuid) {
        let now = chrono::Utc::now().timestamp();
        if let Some(color) = self.player_colors.get(&player_id).cloned() {
            self.color_last_active.insert(color, now);
        }
        self.player_last_active.insert(player_id, now);
    }

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
    fn default() -> Self {
        Self::new()
    }
}
