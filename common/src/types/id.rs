use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a player across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub Uuid);

impl PlayerId {
    /// Generates a new random player identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns a nil player identifier for testing or placeholders.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for PlayerId {
    /// Provides a new random player identifier.
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PlayerId {
    /// Wraps a UUID as a player identifier.
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PlayerId> for Uuid {
    /// Unwraps the player identifier to its UUID.
    fn from(value: PlayerId) -> Self {
        value.0
    }
}

impl fmt::Display for PlayerId {
    /// Formats the identifier as a UUID string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a piece instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(pub Uuid);

impl PieceId {
    /// Generates a new random piece identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns a nil piece identifier for testing or placeholders.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for PieceId {
    /// Provides a new random piece identifier.
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PieceId {
    /// Wraps a UUID as a piece identifier.
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PieceId> for Uuid {
    /// Unwraps the piece identifier to its UUID.
    fn from(value: PieceId) -> Self {
        value.0
    }
}

impl fmt::Display for PieceId {
    /// Formats the identifier as a UUID string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a piece type defined in config (e.g., "pawn").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceTypeId(pub String);

impl From<&str> for PieceTypeId {
    /// Wraps a string slice as a piece type identifier.
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PieceTypeId {
    /// Wraps an owned string as a piece type identifier.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for PieceTypeId {
    /// Returns a borrowed view of the piece type id.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PieceTypeId {
    /// Returns true when the piece type is the configured king.
    pub fn is_king(&self) -> bool {
        self.0 == "king" || self.0.ends_with("_king")
    }
}

impl fmt::Display for PieceTypeId {
    /// Formats the piece type id as a string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a shop config entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShopId(pub String);

impl From<&str> for ShopId {
    /// Wraps a string slice as a shop identifier.
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ShopId {
    /// Wraps an owned string as a shop identifier.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ShopId {
    /// Returns a borrowed view of the shop id.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShopId {
    /// Formats the shop id as a string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a starting kit configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KitId(pub String);

impl From<&str> for KitId {
    /// Wraps a string slice as a kit identifier.
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for KitId {
    /// Wraps an owned string as a kit identifier.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for KitId {
    /// Returns a borrowed view of the kit id.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Identifier for a game mode configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModeId(pub String);

impl From<&str> for ModeId {
    /// Wraps a string slice as a mode identifier.
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ModeId {
    /// Wraps an owned string as a mode identifier.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ModeId {
    /// Returns a borrowed view of the mode id.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModeId {
    /// Formats the mode id as a string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Private session token used to authenticate re-joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionSecret(pub Uuid);

impl SessionSecret {
    /// Generates a new random session secret.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns a nil session secret for testing or placeholders.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for SessionSecret {
    /// Provides a new random session secret.
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for SessionSecret {
    /// Wraps a UUID as a session secret.
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<SessionSecret> for Uuid {
    /// Unwraps the session secret into its UUID.
    fn from(value: SessionSecret) -> Self {
        value.0
    }
}

impl fmt::Display for SessionSecret {
    /// Formats the session secret as a UUID string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
