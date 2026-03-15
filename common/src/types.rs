//! Strongly typed domain primitives used across client and server.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use uuid::Uuid;

/// Total points or currency awarded to a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Score(pub u64);

impl Score {
    /// Returns a zero-valued score.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Returns the underlying numeric score value.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for Score {
    /// Provides the default score (zero).
    fn default() -> Self {
        Self::zero()
    }
}

impl From<u64> for Score {
    /// Wraps a raw `u64` as a score.
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Score> for u64 {
    /// Unwraps a score into its raw numeric value.
    fn from(value: Score) -> Self {
        value.0
    }
}

impl fmt::Display for Score {
    /// Formats the score as a decimal string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for Score {
    type Output = Self;

    /// Adds two scores, saturating on overflow.
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Score {
    type Output = Self;

    /// Subtracts two scores, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for Score {
    /// Adds another score in-place, saturating on overflow.
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Score {
    /// Subtracts another score in-place, saturating on underflow.
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// Total board dimension in tiles (square board).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct BoardSize(pub i32);

impl BoardSize {
    /// Creates a board size, clamping to at least 1 tile.
    pub fn new(value: i32) -> Self {
        Self(value.max(1))
    }

    /// Returns the raw tile dimension for the board.
    pub fn as_i32(self) -> i32 {
        self.0
    }

    /// Returns half the board size in tiles.
    pub fn half(self) -> i32 {
        self.0 / 2
    }

    /// Returns the inclusive coordinate limit for positive positions.
    pub fn limit_pos(self) -> i32 {
        (self.0 + 1) / 2
    }
}

impl Default for BoardSize {
    /// Provides the default board size.
    fn default() -> Self {
        Self(40)
    }
}

impl From<i32> for BoardSize {
    /// Wraps a raw tile size as a board size.
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<BoardSize> for i32 {
    /// Unwraps a board size into its raw tile dimension.
    fn from(value: BoardSize) -> Self {
        value.0
    }
}

impl fmt::Display for BoardSize {
    /// Formats the board size as a decimal string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Absolute time in milliseconds since epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
#[serde(transparent)]
pub struct TimestampMs(pub i64);

impl TimestampMs {
    /// Wraps a raw millisecond timestamp.
    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    /// Returns the raw millisecond timestamp.
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for TimestampMs {
    /// Wraps a raw millisecond timestamp.
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<TimestampMs> for i64 {
    /// Unwraps the timestamp into raw milliseconds.
    fn from(value: TimestampMs) -> Self {
        value.0
    }
}

/// Duration measured in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct DurationMs(pub i64);

impl DurationMs {
    /// Returns a zero-length duration.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Wraps a raw millisecond duration.
    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    /// Returns the raw millisecond duration.
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Returns the duration clamped to a non-negative `u64`.
    pub fn as_u64(self) -> u64 {
        self.0.max(0) as u64
    }

    /// Returns the duration in seconds as a floating point value.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }
}

impl Default for DurationMs {
    /// Provides the default duration (zero).
    fn default() -> Self {
        Self::zero()
    }
}

impl From<i64> for DurationMs {
    /// Wraps a raw millisecond duration.
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<DurationMs> for i64 {
    /// Unwraps the duration into raw milliseconds.
    fn from(value: DurationMs) -> Self {
        value.0
    }
}

impl Add for DurationMs {
    type Output = Self;

    /// Adds two durations, saturating on overflow.
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for DurationMs {
    type Output = Self;

    /// Subtracts two durations, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for DurationMs {
    /// Adds another duration in-place, saturating on overflow.
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for DurationMs {
    /// Subtracts another duration in-place, saturating on underflow.
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Sub for TimestampMs {
    type Output = DurationMs;

    /// Returns the duration between two timestamps, saturating on underflow.
    fn sub(self, rhs: Self) -> Self::Output {
        DurationMs(self.0.saturating_sub(rhs.0))
    }
}

impl Add<DurationMs> for TimestampMs {
    type Output = Self;

    /// Adds a duration to a timestamp, saturating on overflow.
    fn add(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub<DurationMs> for TimestampMs {
    type Output = Self;

    /// Subtracts a duration from a timestamp, saturating on underflow.
    fn sub(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

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
        self.0 == "king"
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
