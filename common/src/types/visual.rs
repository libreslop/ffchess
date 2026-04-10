use serde::{Deserialize, Serialize};
use std::fmt;

/// Hex-encoded RGB color string (e.g., "#ff0000").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorHex(pub String);

impl From<&str> for ColorHex {
    /// Wraps a string slice as a hex color.
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ColorHex {
    /// Wraps an owned string as a hex color.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ColorHex {
    /// Returns a borrowed view of the color string.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ColorHex {
    /// Formats the color string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
