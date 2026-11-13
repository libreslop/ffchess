//! Loads JSON/JSONC configuration for pieces, shops, modes, and globals.

use common::models::GameModeConfig;
use common::rules::Catalog;
use common::types::{DurationMs, ModeId};
use educe::Educe;
use jsonc_parser::parse_to_serde_value;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

impl ServerGlobalConfig {
    /// Returns how long chat lines are retained.
    pub fn chat_ttl(&self) -> DurationMs {
        DurationMs::from_millis(self.chat_message_ttl_ms.max(1) as i64)
    }

    /// Returns the maximum accepted chat message length in characters.
    pub fn chat_max_chars(&self) -> usize {
        self.chat_message_max_chars.max(1) as usize
    }
}

/// Loads and stores all runtime configuration.
pub struct ConfigManager {
    pub catalog: Arc<Catalog>,
    pub modes: HashMap<ModeId, GameModeConfig>,
    pub name_pool: NamePool,
    pub global: ServerGlobalConfig,
    /// Client global settings re-serialized as plain JSON for page injection.
    pub client_global_json: String,
}

impl ConfigManager {
    /// Loads configuration from the given config root directory.
    ///
    /// Panics with file context when a piece, shop, or mode config is malformed, and when a
    /// special move names an unknown piece type or has an unparsable condition.
    pub fn load(root: &Path) -> Self {
        let catalog = Catalog {
            pieces: load_id_mapped_configs(&root.join("pieces"), |_| {}),
            shops: load_id_mapped_configs(&root.join("shops"), normalize_shop_default_group),
        };
        catalog
            .validate()
            .unwrap_or_else(|e| panic!("Invalid piece config: {e}"));
        let modes = load_id_mapped_configs(&root.join("modes"), |_| {});
        let server_globals = read_jsonc_value(&root.join("global/server.jsonc"));

        Self {
            catalog: Arc::new(catalog),
            modes,
            name_pool: server_globals
                .as_ref()
                .and_then(|value| value.get("default_name").cloned())
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default(),
            global: server_globals
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default(),
            client_global_json: read_jsonc_value(&root.join("global/client.jsonc"))
                .map_or_else(|| "{}".to_string(), |value| value.to_string()),
        }
    }
}

/// Loads id-keyed configs from one directory, using each file stem as the id.
///
/// `mutate` may normalize the raw JSON object before deserialization.
fn load_id_mapped_configs<T, K>(
    dir: &Path,
    mutate: impl Fn(&mut Map<String, Value>),
) -> HashMap<K, T>
where
    T: DeserializeOwned,
    K: Eq + Hash + From<String>,
{
    config_paths(dir)
        .into_iter()
        .map(|path| {
            let id = file_stem(&path);
            let config = parse_config_file(&path, &id, &mutate);
            (K::from(id), config)
        })
        .collect()
}

/// Returns all JSON/JSONC config file paths under `dir`, sorted.
fn config_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "json" || ext == "jsonc")
        })
        .collect();
    paths.sort();
    paths
}

/// Reads one JSONC document as a serde value, returning `None` on any failure.
fn read_jsonc_value(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_to_serde_value(&content, &Default::default())
        .ok()
        .flatten()
}

/// Parses one config file, injects its file-derived `id`, and deserializes it.
fn parse_config_file<T: DeserializeOwned>(
    path: &Path,
    id: &str,
    mutate: &impl Fn(&mut Map<String, Value>),
) -> T {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read config {path:?}: {e}"));
    let mut value = parse_to_serde_value(&content, &Default::default())
        .unwrap_or_else(|e| panic!("Failed to parse config {path:?}: {e}"))
        .unwrap_or_else(|| panic!("Failed to parse config {path:?}: empty document"));
    let object = value
        .as_object_mut()
        .unwrap_or_else(|| panic!("Expected object in config {path:?}"));
    object.insert("id".to_string(), Value::String(id.to_string()));
    mutate(object);
    serde_json::from_value(value)
        .unwrap_or_else(|e| panic!("Failed to deserialize config {path:?}: {e}"))
}

/// Ensures a shop default group deserializes like a regular group without `applies_to`.
fn normalize_shop_default_group(object: &mut Map<String, Value>) {
    if let Some(Value::Object(group)) = object.get_mut("default_group") {
        group
            .entry("applies_to")
            .or_insert_with(|| Value::Array(vec![]));
    }
}

/// Extracts the filename stem (without extension) as a string.
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
