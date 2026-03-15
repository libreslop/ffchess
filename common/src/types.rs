use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a player across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for PlayerId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PlayerId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PlayerId> for Uuid {
    fn from(value: PlayerId) -> Self {
        value.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a piece instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(pub Uuid);

impl PieceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for PieceId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PieceId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PieceId> for Uuid {
    fn from(value: PieceId) -> Self {
        value.0
    }
}

impl fmt::Display for PieceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a piece type defined in config (e.g., "pawn").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceTypeId(pub String);

impl From<&str> for PieceTypeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PieceTypeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for PieceTypeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PieceTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a shop config entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShopId(pub String);

impl From<&str> for ShopId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ShopId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ShopId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier for a starting kit configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KitId(pub String);

impl From<&str> for KitId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for KitId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for KitId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Identifier for a game mode configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModeId(pub String);

impl From<&str> for ModeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ModeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ModeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Private session token used to authenticate re-joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionSecret(pub Uuid);

impl SessionSecret {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for SessionSecret {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for SessionSecret {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<SessionSecret> for Uuid {
    fn from(value: SessionSecret) -> Self {
        value.0
    }
}

impl fmt::Display for SessionSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Hex-encoded RGB color string (e.g., "#ff0000").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorHex(pub String);

impl From<&str> for ColorHex {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ColorHex {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ColorHex {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ColorHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
