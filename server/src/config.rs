//! Loads JSON/JSONC configuration for pieces, shops, and modes.

use common::models::{GameModeConfig, PieceConfig, ShopConfig};
use common::types::{BoardCoord, ModeId, PieceTypeId, ShopId};
use glam::IVec2;
use jsonc_parser::parse_to_serde_value;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Name parts used for generating default player names.
#[derive(Default, serde::Deserialize, Clone)]
pub struct NamePool {
    pub adjectives: Vec<String>,
    pub nouns: Vec<String>,
}

/// Global server settings.
#[derive(Default, serde::Deserialize, Clone)]
pub struct ServerGlobalConfig {
    #[serde(default = "default_sync_interval")]
    pub sync_interval_ms: u32,
}

fn default_sync_interval() -> u32 {
    10000
}

/// One piece placement in a queue-mode preset layout.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QueuePresetPieceConfig {
    pub piece_id: PieceTypeId,
    pub position: [i32; 2],
}

impl QueuePresetPieceConfig {
    /// Returns the configured board coordinate for this piece.
    pub fn board_coord(&self) -> BoardCoord {
        BoardCoord(IVec2::new(self.position[0], self.position[1]))
    }
}

/// Piece placement set for a single queued player slot.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QueuePresetPlayerConfig {
    pub pieces: Vec<QueuePresetPieceConfig>,
}

/// Fixed queue spawn layout keyed by mode id.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QueuePresetLayoutConfig {
    pub players: Vec<QueuePresetPlayerConfig>,
}

/// Loads and stores all runtime configuration.
pub struct ConfigManager {
    pub pieces: HashMap<PieceTypeId, PieceConfig>,
    pub shops: HashMap<ShopId, ShopConfig>,
    pub modes: HashMap<ModeId, GameModeConfig>,
    pub queue_layouts: HashMap<ModeId, QueuePresetLayoutConfig>,
    pub name_pool: NamePool,
    pub global: ServerGlobalConfig,
}

impl ConfigManager {
    /// Loads configuration from the given root directory.
    ///
    /// `root_path` points to the config folder. Returns a populated `ConfigManager`.
    pub fn load(root_path: &Path) -> Self {
        let mut pieces = HashMap::new();
        let mut shops = HashMap::new();
        let mut modes = HashMap::new();
        let mut queue_layouts = HashMap::new();
        let mut name_pool = NamePool::default();
        let mut global = ServerGlobalConfig::default();

        // Try to find the config directory by going up if not found
        let mut actual_root = root_path.to_path_buf();
        if !actual_root.exists()
            && let Ok(cwd) = std::env::current_dir()
            && let Some(parent) = cwd.parent()
        {
            let parent_root = parent.join(root_path);
            if parent_root.exists() {
                actual_root = parent_root;
            }
        }

        // Load pieces
        for entry in WalkDir::new(actual_root.join("pieces"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read piece config");
            let id = file_stem(entry.path());
            let config: PieceConfig = parse_jsonc_with_id(&content, entry.path(), &id);
            pieces.insert(PieceTypeId::from(id), config);
        }

        // Load shops
        for entry in WalkDir::new(actual_root.join("shops"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read shop config");
            let id = file_stem(entry.path());
            let config: ShopConfig = parse_shop_jsonc_with_id(&content, entry.path(), &id);
            shops.insert(ShopId::from(id), config);
        }

        // Load modes
        for entry in WalkDir::new(actual_root.join("modes"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonc")
            })
        {
            let content =
                std::fs::read_to_string(entry.path()).expect("Failed to read mode config");
            let id = file_stem(entry.path());
            let config: GameModeConfig = parse_jsonc_with_id(&content, entry.path(), &id);
            modes.insert(ModeId::from(id), config);
        }

        // Load queue preset layouts
        let queue_layouts_dir = actual_root.join("queue_layouts");
        if queue_layouts_dir.exists() {
            for entry in WalkDir::new(queue_layouts_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "json" || ext == "jsonc")
                })
            {
                let content = std::fs::read_to_string(entry.path())
                    .expect("Failed to read queue layout config");
                let id = file_stem(entry.path());
                let config: QueuePresetLayoutConfig = parse_jsonc(&content, entry.path());
                queue_layouts.insert(ModeId::from(id), config);
            }
        }

        // Load server global name pool and other settings
        let global_server = actual_root.join("global/server.jsonc");
        if global_server.exists()
            && let Ok(content) = std::fs::read_to_string(&global_server)
            && let Ok(parsed) = parse_to_serde_value(&content, &Default::default())
            && let Some(v) = parsed
            && let Ok(cfg) = serde_json::from_value::<serde_json::Value>(v)
        {
            if let Ok(pool) = serde_json::from_value::<NamePool>(
                cfg.get("default_name").cloned().unwrap_or_default(),
            ) {
                name_pool = pool;
            }
            if let Ok(g) = serde_json::from_value::<ServerGlobalConfig>(cfg) {
                global = g;
            }
        }

        Self {
            pieces,
            shops,
            modes,
            queue_layouts,
            name_pool,
            global,
        }
    }
}

/// Parses a JSONC file and injects the `id` field before deserializing.
///
/// `content` is the file contents, `path` is used for error context, and `id` is injected.
/// Returns the deserialized config type `T`.
fn parse_jsonc_with_id<T: DeserializeOwned>(content: &str, path: &Path, id: &str) -> T {
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

/// Parses a JSONC file into a deserialized value without injecting an id.
fn parse_jsonc<T: DeserializeOwned>(content: &str, path: &Path) -> T {
    let value = parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path));

    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}

/// Parses a shop config JSONC file and normalizes the default group.
///
/// `content` is the file contents, `path` is used for error context, and `id` is injected.
/// Returns the deserialized `ShopConfig`.
fn parse_shop_jsonc_with_id(content: &str, path: &Path, id: &str) -> ShopConfig {
    let mut value = parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path));

    if let serde_json::Value::Object(obj) = &mut value {
        obj.insert("id".to_string(), serde_json::Value::String(id.to_string()));
        if let Some(serde_json::Value::Object(group_obj)) = obj.get_mut("default_group") {
            group_obj
                .entry("applies_to")
                .or_insert_with(|| serde_json::Value::Array(vec![]));
        }
    } else {
        panic!("Expected object in config {:?}", path);
    }

    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}

/// Extracts the filename stem (without extension) as a string.
///
/// `path` is the file path to inspect. Returns the stem or an empty string.
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
