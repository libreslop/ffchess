//! Server-specific primitive wrappers.

use common::protocol::ServerMessage;
use std::fmt;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Outbound message channel of one websocket connection.
pub type ClientTx = mpsc::Sender<ServerMessage>;

/// Unique identifier for a live websocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    /// Generates a new connection identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
