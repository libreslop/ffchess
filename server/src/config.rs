//! Loads JSON/JSONC configuration for pieces, shops, and modes.

use common::models::{GameModeConfig, PieceConfig, ShopConfig};
use common::types::{ModeId, PieceTypeId, ShopId};
use educe::Educe;
use jsonc_parser::parse_to_serde_value;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Name parts used for generating default player names.
#[derive(Default, serde::Deserialize, Clone)]
pub struct NamePool {
    pub adjectives: Vec<String>,
    pub nouns: Vec<String>,
}

/// Global server settings.
#[derive(serde::Deserialize, Clone, Educe)]
#[serde(default)]
#[educe(Default)]
pub struct ServerGlobalConfig {
    #[educe(Default = 10000)]
    pub sync_interval_ms: u32,
    #[educe(Default = 10000)]
    pub chat_message_ttl_ms: u32,
    #[educe(Default = 150)]
    pub chat_message_max_chars: u32,
}

/// Loads and stores all runtime configuration.
pub struct ConfigManager {
    pub pieces: HashMap<PieceTypeId, PieceConfig>,
    pub shops: HashMap<ShopId, ShopConfig>,
    pub modes: HashMap<ModeId, GameModeConfig>,
    pub name_pool: NamePool,
    pub global: ServerGlobalConfig,
}

impl ConfigManager {
    /// Loads configuration from the given root directory.
    ///
    /// `root_path` points to the config folder. Returns a populated `ConfigManager`.
    pub fn load(root_path: &Path) -> Self {
        let actual_root = resolve_config_root(root_path);
        let pieces = load_id_mapped_configs(
            &actual_root.join("pieces"),
            "piece",
            parse_jsonc_with_id,
            PieceTypeId::from,
        );
        let shops = load_id_mapped_configs(
            &actual_root.join("shops"),
            "shop",
            parse_shop_jsonc_with_id,
            ShopId::from,
        );
        let modes = load_id_mapped_configs(
            &actual_root.join("modes"),
            "mode",
            parse_jsonc_with_id,
            ModeId::from,
        );

        let (name_pool, global) = load_server_globals(&actual_root.join("global/server.jsonc"));

        Self {
            pieces,
            shops,
            modes,
            name_pool,
            global,
        }
    }
}

/// Resolves the config root path from common run locations.
fn resolve_config_root(root_path: &Path) -> PathBuf {
    if root_path.exists() {
        return root_path.to_path_buf();
    }

    if let Ok(cwd) = std::env::current_dir()
        && let Some(parent) = cwd.parent()
    {
        let parent_root = parent.join(root_path);
        if parent_root.exists() {
            return parent_root;
        }
    }

    root_path.to_path_buf()
}

/// Loads id-keyed configs from one directory of JSON/JSONC files.
fn load_id_mapped_configs<T, K, Parse, Key>(
    dir: &Path,
    kind: &str,
    parse: Parse,
    key_from_id: Key,
) -> HashMap<K, T>
where
    Parse: Fn(&str, &Path, &str) -> T,
    Key: Fn(String) -> K,
    K: Eq + Hash,
{
    let mut loaded = HashMap::new();

    for path in config_paths(dir) {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to read {kind} config: {:?}", path));
        let id = file_stem(&path);
        let config = parse(&content, &path, &id);
        loaded.insert(key_from_id(id), config);
    }

    loaded
}

/// Returns all JSON/JSONC config file paths under `dir`.
fn config_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "json" || ext == "jsonc")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Loads optional server-global settings from one JSONC file.
fn load_server_globals(path: &Path) -> (NamePool, ServerGlobalConfig) {
    let Some(value) = read_jsonc_value(path) else {
        return (NamePool::default(), ServerGlobalConfig::default());
    };

    let name_pool = value
        .get("default_name")
        .cloned()
        .and_then(|v| serde_json::from_value::<NamePool>(v).ok())
        .unwrap_or_default();

    let global = serde_json::from_value::<ServerGlobalConfig>(value).unwrap_or_default();

    (name_pool, global)
}

/// Reads one JSONC document as a serde value.
fn read_jsonc_value(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_to_serde_value(&content, &Default::default())
        .ok()
        .flatten()
}

/// Parses a JSONC file and injects the `id` field before deserializing.
///
/// `content` is the file contents, `path` is used for error context, and `id` is injected.
/// Returns the deserialized config type `T`.
fn parse_jsonc_with_id<T: DeserializeOwned>(content: &str, path: &Path, id: &str) -> T {
    parse_jsonc_with_id_mutator(content, path, id, |_| {})
}

/// Parses a shop config JSONC file and normalizes the default group.
///
/// `content` is the file contents, `path` is used for error context, and `id` is injected.
/// Returns the deserialized `ShopConfig`.
fn parse_shop_jsonc_with_id(content: &str, path: &Path, id: &str) -> ShopConfig {
    parse_jsonc_with_id_mutator(content, path, id, normalize_shop_default_group)
}

/// Parses a JSONC object, injects the file-derived id, applies one mutation, and deserializes it.
fn parse_jsonc_with_id_mutator<T, F>(content: &str, path: &Path, id: &str, mutate: F) -> T
where
    T: DeserializeOwned,
    F: FnOnce(&mut serde_json::Map<String, serde_json::Value>),
{
    let mut value = parse_jsonc_document(content, path);
    let obj = value
        .as_object_mut()
        .unwrap_or_else(|| panic!("Expected object in config {:?}", path));
    obj.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    mutate(obj);
    deserialize_config_value(value, path)
}

/// Parses raw JSONC content into one serde value with config-specific error context.
fn parse_jsonc_document(content: &str, path: &Path) -> serde_json::Value {
    parse_to_serde_value(content, &Default::default())
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
        .unwrap()
        .unwrap_or_else(|| panic!("Failed to parse config {:?}: empty document", path))
}

/// Deserializes one config value into its target type with config-specific error context.
fn deserialize_config_value<T: DeserializeOwned>(value: serde_json::Value, path: &Path) -> T {
    serde_json::from_value(value)
        .map_err(|e| format!("Failed to deserialize config {:?}: {}", path, e))
        .unwrap()
}

/// Ensures a shop default group behaves like a regular empty group during deserialization.
fn normalize_shop_default_group(obj: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(default_group) = obj.get_mut("default_group")
        && let serde_json::Value::Object(group_obj) = default_group
    {
        group_obj
            .entry("applies_to")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
    }
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
