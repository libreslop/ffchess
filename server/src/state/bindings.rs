//! Connection binding helpers for active players.

use super::ServerState;
use crate::instance::GameInstance;
use crate::types::ConnectionId;
use common::types::PlayerId;
use std::sync::Arc;

/// Active game binding for a websocket connection.
#[derive(Clone)]
pub struct ActivePlayerBinding {
    player_id: PlayerId,
    instance: Arc<GameInstance>,
}

impl ActivePlayerBinding {
    /// Returns the bound player id.
    pub fn player_id(&self) -> PlayerId {
        self.player_id
    }

    /// Returns the bound game instance.
    pub fn instance(&self) -> &Arc<GameInstance> {
        &self.instance
    }

    /// Splits the binding into its parts.
    pub fn into_parts(self) -> (PlayerId, Arc<GameInstance>) {
        (self.player_id, self.instance)
    }
}

impl ServerState {
    /// Stores an active connection->player binding.
    pub async fn bind_connection(
        &self,
        conn_id: ConnectionId,
        player_id: PlayerId,
        instance: Arc<GameInstance>,
    ) {
        self.connection_bindings.write().await.insert(
            conn_id,
            ActivePlayerBinding {
                player_id,
                instance,
            },
        );
    }

    /// Looks up an active connection->player binding.
    pub async fn get_binding(&self, conn_id: ConnectionId) -> Option<ActivePlayerBinding> {
        self.connection_bindings.read().await.get(&conn_id).cloned()
    }

    /// Removes and returns an active connection->player binding.
    pub async fn unbind_connection(&self, conn_id: ConnectionId) -> Option<ActivePlayerBinding> {
        self.connection_bindings.write().await.remove(&conn_id)
    }
}
