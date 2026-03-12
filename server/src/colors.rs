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
}

impl ColorManager {
    pub fn new() -> Self {
        Self {
            player_colors: HashMap::new(),
            color_last_active: HashMap::new(),
        }
    }

    pub fn get_or_assign_color(&mut self, player_id: Uuid, active_player_ids: &[Uuid]) -> String {
        let now = chrono::Utc::now().timestamp();

        // Identify currently active colors.
        let active_colors: Vec<String> = active_player_ids
            .iter()
            .filter_map(|id| self.player_colors.get(id).cloned())
            .collect();

        // 1. Re-use player's color if it's NOT active.
        if let Some(color) = self.player_colors.get(&player_id).cloned()
            && !active_colors.contains(&color)
        {
            self.color_last_active.insert(color.clone(), now);
            return color;
        }

        // 2. Try to find a preferred color that is NOT active and NOT claimed.
        // If a color is expired, it is NO LONGER claimed.
        for &c in PREFERRED_COLORS {
            let color = c.to_string();
            if !active_colors.contains(&color) {
                let last_active = self.color_last_active.get(&color);
                let claimed = last_active.is_some_and(|&last| now - last < 60);
                if !claimed {
                    self.player_colors.insert(player_id, color.clone());
                    self.color_last_active.insert(color.clone(), now);
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
                    self.player_colors.insert(player_id, color.clone());
                    self.color_last_active.insert(color.clone(), now);
                    return color;
                }
            }
        }
        
        // Fallback to anything not active if we are really crowded
        for &c in PREFERRED_COLORS {
             let color = c.to_string();
             if !active_colors.contains(&color) {
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                return color;
             }
        }

        // Ultimate fallback
        let color = format!("#{:06x}", rng.gen_range(0..0x1000000));
        self.player_colors.insert(player_id, color.clone());
        self.color_last_active.insert(color.clone(), now);
        color
    }

    pub fn update_activity(&mut self, player_id: Uuid) {
        if let Some(color) = self.player_colors.get(&player_id).cloned() {
            self.color_last_active.insert(color, chrono::Utc::now().timestamp());
        }
    }
}

impl Default for ColorManager {
    fn default() -> Self {
        Self::new()
    }
}
