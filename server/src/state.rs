use crate::config::ConfigManager;
use crate::instance::GameInstance;
use common::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ServerState {
    pub games: Arc<RwLock<HashMap<ModeId, Arc<GameInstance>>>>,
    pub config_manager: Arc<ConfigManager>,
}

impl ServerState {
    pub fn new() -> Self {
        let config_manager = Arc::new(ConfigManager::load(std::path::Path::new("config")));
        let mut games = HashMap::new();

        let piece_configs = Arc::new(config_manager.pieces.clone());
        let shop_configs = Arc::new(config_manager.shops.clone());

        for (mode_id, mode_config) in &config_manager.modes {
            let instance = Arc::new(GameInstance::new(
                mode_config.clone(),
                piece_configs.clone(),
                shop_configs.clone(),
            ));
            games.insert(mode_id.clone(), instance);
        }

        Self {
            games: Arc::new(RwLock::new(games)),
            config_manager,
        }
    }

    pub async fn get_game(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        self.games.read().await.get(mode_id).cloned()
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
