use common::models::{GameModeConfig, PieceConfig, ShopConfig};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

pub struct ConfigManager {
    pub pieces: HashMap<String, PieceConfig>,
    pub shops: HashMap<String, ShopConfig>,
    pub modes: HashMap<String, GameModeConfig>,
}

impl ConfigManager {
    pub fn load(root_path: &Path) -> Self {
        let mut pieces = HashMap::new();
        let mut shops = HashMap::new();
        let mut modes = HashMap::new();

        // Try to find the config directory by going up if not found
        let mut actual_root = root_path.to_path_buf();
        if !actual_root.exists() {
            if let Ok(cwd) = std::env::current_dir() {
                if let Some(parent) = cwd.parent() {
                    let parent_root = parent.join(root_path);
                    if parent_root.exists() {
                        actual_root = parent_root;
                    }
                }
            }
        }

        // Load pieces
        for entry in WalkDir::new(actual_root.join("pieces"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json" || ext == "jsonc"))
        {
            let content = std::fs::read_to_string(entry.path()).expect("Failed to read piece config");
            let stripped = strip_comments(&content);
            let config: PieceConfig = serde_json::from_str(&stripped)
                .map_err(|e| format!("Failed to parse piece config {:?}: {}", entry.path(), e))
                .unwrap();
            pieces.insert(config.id.clone(), config);
        }

        // Load shops
        for entry in WalkDir::new(actual_root.join("shops"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json" || ext == "jsonc"))
        {
            let content = std::fs::read_to_string(entry.path()).expect("Failed to read shop config");
            let stripped = strip_comments(&content);
            let config: ShopConfig = serde_json::from_str(&stripped)
                .map_err(|e| format!("Failed to parse shop config {:?}: {}", entry.path(), e))
                .unwrap();
            shops.insert(config.id.clone(), config);
        }

        // Load modes
        for entry in WalkDir::new(actual_root.join("modes"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json" || ext == "jsonc"))
        {
            let content = std::fs::read_to_string(entry.path()).expect("Failed to read mode config");
            let stripped = strip_comments(&content);
            let config: GameModeConfig = serde_json::from_str(&stripped)
                .map_err(|e| format!("Failed to parse mode config {:?}: {}", entry.path(), e))
                .unwrap();
            modes.insert(config.id.clone(), config);
        }

        Self { pieces, shops, modes }
    }
}

fn strip_comments(json: &str) -> String {
    json.lines()
        .map(|line| {
            if let Some(pos) = line.find("//") {
                &line[..pos]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
