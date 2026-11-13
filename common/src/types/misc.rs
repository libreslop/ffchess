use super::MoveCount;
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Value,
    eval_boolean_with_context, eval_number_with_context,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Expression written in config files and evaluated at runtime with [`evalexpr`].
///
/// Numeric fields such as prices and board sizes evaluate it with [`ExprString::evaluate`];
/// conditions such as those of special moves evaluate it with [`ExprString::holds`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExprString(pub String);

impl ExprString {
    /// Evaluates the expression as a number with the provided variable bindings.
    ///
    /// Returns `0.0` when the expression is malformed, references unknown variables, or does
    /// not produce a number.
    pub fn evaluate(&self, vars: &ExprVars) -> f64 {
        eval_number_with_context(&self.0, &vars.context()).unwrap_or(0.0)
    }

    /// Evaluates the expression as a condition with the provided variable bindings.
    ///
    /// Returns `false` when the expression is malformed, references unknown variables, or does
    /// not produce a boolean.
    pub fn holds(&self, vars: &ExprVars) -> bool {
        eval_boolean_with_context(&self.0, &vars.context()).unwrap_or(false)
    }

    /// Returns true when `condition` is unset or holds for `vars`.
    pub fn holds_if_set(condition: Option<&Self>, vars: &ExprVars) -> bool {
        condition.is_none_or(|condition| condition.holds(vars))
    }

    /// Checks that the expression is a valid condition when its variables hold `sample`.
    ///
    /// `sample` binds every variable the condition may use, so missing operands, misspelled
    /// variables, and non-boolean results surface when configs load instead of silently
    /// never holding.
    pub fn check_condition(&self, sample: &ExprVars) -> Result<(), String> {
        eval_boolean_with_context(&self.0, &sample.context())
            .map(drop)
            .map_err(|error| format!("invalid condition `{}`: {error}", self.0))
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

/// Value bound to an expression variable.
///
/// Counts are integers so that conditions like `moves == 0` compare exactly; note that
/// dividing two integers rounds down (`count / 2`), while `count / 2.0` keeps the fraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExprValue {
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl From<bool> for ExprValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<usize> for ExprValue {
    fn from(value: usize) -> Self {
        Self::Int(i64::try_from(value).unwrap_or(i64::MAX))
    }
}

impl From<u32> for ExprValue {
    fn from(value: u32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<MoveCount> for ExprValue {
    fn from(value: MoveCount) -> Self {
        Self::from(value.as_u32())
    }
}

impl From<f64> for ExprValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<ExprValue> for Value<DefaultNumericTypes> {
    fn from(value: ExprValue) -> Self {
        match value {
            ExprValue::Int(int) => Value::Int(int),
            ExprValue::Float(float) => Value::Float(float),
            ExprValue::Bool(boolean) => Value::Boolean(boolean),
        }
    }
}

/// Named variables available to an [`ExprString`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExprVars(HashMap<String, ExprValue>);

impl ExprVars {
    /// Creates an empty variable set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the set with `name` bound to `value`.
    pub fn with(mut self, name: impl Into<String>, value: impl Into<ExprValue>) -> Self {
        self.set(name, value);
        self
    }

    /// Binds `name` to `value`, replacing any previous binding.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<ExprValue>) {
        self.0.insert(name.into(), value.into());
    }

    /// Returns the value bound to `name`, if any.
    pub fn get(&self, name: &str) -> Option<ExprValue> {
        self.0.get(name).copied()
    }

    /// Builds the evaluation context holding these bindings and the built-in functions.
    fn context(&self) -> HashMapContext<DefaultNumericTypes> {
        let mut context = HashMapContext::new();
        for (name, value) in &self.0 {
            // Setting a fresh variable only fails on a type clash with an earlier binding,
            // which cannot happen in a new context.
            let _ = context.set_value(name.clone(), (*value).into());
        }
        context
    }
}
