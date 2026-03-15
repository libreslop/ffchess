//! Server-specific primitive wrappers.

use std::fmt;
use uuid::Uuid;

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
    /// Provides a new random connection identifier.
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConnectionId {
    /// Formats the identifier as a UUID string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
