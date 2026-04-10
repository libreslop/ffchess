use serde::{Deserialize, Serialize};
use std::fmt;

/// String expression evaluated at runtime for numeric values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExprString(pub String);

impl From<&str> for ExprString {
    /// Wraps a string slice as an expression string.
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ExprString {
    /// Wraps an owned string as an expression string.
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for ExprString {
    /// Returns a borrowed view of the expression.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExprString {
    /// Formats the expression as its raw string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
