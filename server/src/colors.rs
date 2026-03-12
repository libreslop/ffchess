use std::collections::HashMap;
use uuid::Uuid;
use rand::Rng;

pub const PREFERRED_COLORS: &[&str] = &[
    "#2563eb", // Blue
    "#dc2626", // Red
    "#16a34a", // Green
    "#d97706", // Orange
    "#9333ea", // Purple
    "#0891b2", // Cyan
    "#db2777", // Pink
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
        if let Some(color) = self.player_colors.get(&player_id) {
            return color.clone();
        }

        let now = chrono::Utc::now().timestamp();
        let active_colors: Vec<String> = active_player_ids.iter().filter_map(|id| self.player_colors.get(id).cloned()).collect();

        for &c in PREFERRED_COLORS {
            let color = c.to_string();
            if !active_colors.contains(&color) {
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                return color;
            }
        }

        for &c in PREFERRED_COLORS {
            let color = c.to_string();
            if let Some(&last_active) = self.color_last_active.get(&color)
                && now - last_active > 300 {
                self.player_colors.insert(player_id, color.clone());
                self.color_last_active.insert(color.clone(), now);
                return color;
            }
        }

        let mut rng = rand::thread_rng();
        let color = format!("#{:06x}", rng.gen_range(0..0x1000000));
        self.player_colors.insert(player_id, color.clone());
        self.color_last_active.insert(color.clone(), now);
        color
    }
}
