use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// String expression evaluated at runtime for numeric values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExprString(pub String);

impl ExprString {
    /// Evaluates the expression with the provided variable bindings.
    ///
    /// Returns `0.0` when the expression is malformed or references unknown variables.
    pub fn evaluate(&self, vars: &ExprVars) -> f64 {
        let mut context = meval::Context::new();
        for (name, value) in &vars.0 {
            context.var(name, *value);
        }
        meval::eval_str_with_context(&self.0, &context).unwrap_or(0.0)
    }
}

impl From<&str> for ExprString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ExprString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for ExprString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExprString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Named numeric variables available to an [`ExprString`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExprVars(HashMap<String, f64>);

impl ExprVars {
    /// Creates an empty variable set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the set with `name` bound to `value`.
    pub fn with(mut self, name: impl Into<String>, value: f64) -> Self {
        self.set(name, value);
        self
    }

    /// Binds `name` to `value`, replacing any previous binding.
    pub fn set(&mut self, name: impl Into<String>, value: f64) {
        self.0.insert(name.into(), value);
    }

    /// Returns the value bound to `name`, if any.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.0.get(name).copied()
    }
}
