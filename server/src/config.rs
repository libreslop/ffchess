use common::models::{GameModeConfig, PieceConfig, ShopConfig};
use jsonc_parser::parse_to_serde_value;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Default, serde::Deserialize, Clone)]
pub struct NamePool {
    pub adjectives: Vec<String>,
    pub nouns: Vec<String>,
}

pub struct ConfigManager {
    pub pieces: HashMap<String, PieceConfig>,
    pub shops: HashMap<String, ShopConfig>,
    pub modes: HashMap<String, GameModeConfig>,
    pub name_pool: NamePool,
}

impl ConfigManager {
    pub fn load(root_path: &Path) -> Self {
        let mut pieces = HashMap::new();
        let mut shops = HashMap::new();
        let mut modes = HashMap::new();
        let mut name_pool = NamePool::default();

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
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read piece config");
            let config: PieceConfig = parse_jsonc(&content, entry.path());
            pieces.insert(config.id.clone(), config);
        }

        // Load shops
        for entry in WalkDir::new(actual_root.join("shops"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read shop config");
            let config: ShopConfig = parse_jsonc(&content, entry.path());
            shops.insert(config.id.clone(), config);
        }

        // Load modes
        for entry in WalkDir::new(actual_root.join("modes"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read mode config");
            let config: GameModeConfig = parse_jsonc(&content, entry.path());
            modes.insert(config.id.clone(), config);
        }

        // Load server global name pool
        let global_server = actual_root.join("global/server.jsonc");
        if global_server.exists() {
            if let Ok(content) = std::fs::read_to_string(&global_server) {
                if let Ok(parsed) = parse_to_serde_value(&content, &Default::default()) {
                    if let Some(v) = parsed {
                        if let Ok(cfg) = serde_json::from_value::<serde_json::Value>(v.clone()) {
                            if let Ok(pool) = serde_json::from_value::<NamePool>(
                                cfg.get("default_name").cloned().unwrap_or_default(),
                            ) {
                                name_pool = pool;
                            }
                        }
                    }
                }
            }
        }

        Self {
            pieces,
            shops,
            modes,
            name_pool,
        }
    }
}

fn parse_jsonc<T: DeserializeOwned>(content: &str, path: &Path) -> T {
    let value = parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path));
    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}
