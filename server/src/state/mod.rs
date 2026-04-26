//! Shared server state and mode instance registry.

mod bindings;
mod games;
mod preview;
mod queue;

use crate::config::ConfigManager;
use crate::instance::GameInstance;
use crate::types::ConnectionId;
use common::models::{PieceConfig, ShopConfig};
use common::types::{ModeId, PieceTypeId, ShopId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

pub use bindings::ActivePlayerBinding;
pub use queue::{MatchQueueEntry, QueueJoinResult};

use preview::ModePreviewState;

/// Shared server state containing mode instances and configuration.
pub struct ServerState {
    games: Arc<RwLock<HashMap<ModeId, Arc<GameInstance>>>>,
    config_manager: Arc<ConfigManager>,
    piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
    shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    private_game_ids: Arc<RwLock<HashSet<ModeId>>>,
    queue_entries: Arc<RwLock<HashMap<ModeId, Vec<MatchQueueEntry>>>>,
    connection_bindings: Arc<RwLock<HashMap<ConnectionId, ActivePlayerBinding>>>,
    preview_state: Arc<RwLock<HashMap<ModeId, ModePreviewState>>>,
}

impl ServerState {
    /// Builds the global server state and loads configuration.
    ///
    /// Returns a `ServerState` with one `GameInstance` per configured mode.
    pub fn new() -> Self {
        let config_manager = Arc::new(ConfigManager::load(std::path::Path::new("config")));
        let mut games = HashMap::new();

        let piece_configs = Arc::new(config_manager.pieces.clone());
        let shop_configs = Arc::new(config_manager.shops.clone());

        for (mode_id, mode_config) in &config_manager.modes {
            let instance = Arc::new(GameInstance::new(
                mode_config.clone(),
                mode_id.clone(),
                config_manager
                    .queue_layouts
                    .get(mode_id)
                    .cloned()
                    .map(Arc::new),
                piece_configs.clone(),
                shop_configs.clone(),
            ));
            games.insert(mode_id.clone(), instance);
        }

        Self {
            games: Arc::new(RwLock::new(games)),
            config_manager,
            piece_configs,
            shop_configs,
            private_game_ids: Arc::new(RwLock::new(HashSet::new())),
            queue_entries: Arc::new(RwLock::new(HashMap::new())),
            connection_bindings: Arc::new(RwLock::new(HashMap::new())),
            preview_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ServerState {
    /// Provides a default server state by loading configuration.
    fn default() -> Self {
        Self::new()
    }
}
