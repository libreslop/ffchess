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
            let id = file_stem(entry.path());
            let config: PieceConfig = parse_jsonc_with_id(&content, entry.path(), &id);
            pieces.insert(id, config);
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
            let id = file_stem(entry.path());
            let config: ShopConfig = parse_shop_jsonc_with_id(&content, entry.path(), &id);
            shops.insert(id, config);
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
            let id = file_stem(entry.path());
            let config: GameModeConfig = parse_jsonc_with_id(&content, entry.path(), &id);
            modes.insert(id, config);
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

fn parse_jsonc_with_id<T: DeserializeOwned>(
    content: &str,
    path: &Path,
    id: &str,
) -> T {
    let mut value = parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path));

    if let serde_json::Value::Object(obj) = &mut value {
        obj.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    } else {
        panic!("Expected object in config {:?}", path);
    }

    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}

fn parse_shop_jsonc_with_id(
    content: &str,
    path: &Path,
    id: &str,
) -> ShopConfig {
    let mut value = parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path));

    if let serde_json::Value::Object(obj) = &mut value {
        obj.insert("id".to_string(), serde_json::Value::String(id.to_string()));
        if let Some(default_group) = obj.get_mut("default_group") {
            if let serde_json::Value::Object(group_obj) = default_group {
                group_obj
                    .entry("applies_to")
                    .or_insert_with(|| serde_json::Value::Array(vec![]));
            }
        }
    } else {
        panic!("Expected object in config {:?}", path);
    }

    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
