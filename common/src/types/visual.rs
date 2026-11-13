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

/// Opacity written as a percentage, from 0 (invisible) to 100 (opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpacityPercent(pub u8);

impl OpacityPercent {
    /// Returns a see-through opacity that leaves what is behind readable.
    pub fn translucent() -> Self {
        Self(35)
    }

    /// Returns full opacity.
    pub fn opaque() -> Self {
        Self(100)
    }

    /// Returns the opacity as an alpha value between 0 and 1.
    pub fn alpha(self) -> f64 {
        f64::from(self.0.min(100)) / 100.0
    }
}
