use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use uuid::Uuid;

/// Total points or currency awarded to a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Score(pub u64);

impl Score {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for Score {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<u64> for Score {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Score> for u64 {
    fn from(value: Score) -> Self {
        value.0
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for Score {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for Score {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for Score {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Score {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// Total board dimension in tiles (square board).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct BoardSize(pub i32);

impl BoardSize {
    pub fn new(value: i32) -> Self {
        Self(value.max(1))
    }

    pub fn as_i32(self) -> i32 {
        self.0
    }

    pub fn half(self) -> i32 {
        self.0 / 2
    }

    pub fn limit_pos(self) -> i32 {
        (self.0 + 1) / 2
    }
}

impl Default for BoardSize {
    fn default() -> Self {
        Self(40)
    }
}

impl From<i32> for BoardSize {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<BoardSize> for i32 {
    fn from(value: BoardSize) -> Self {
        value.0
    }
}

impl fmt::Display for BoardSize {
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
    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for TimestampMs {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<TimestampMs> for i64 {
    fn from(value: TimestampMs) -> Self {
        value.0
    }
}

/// Duration measured in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct DurationMs(pub i64);

impl DurationMs {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn from_millis(value: i64) -> Self {
        Self(value)
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }

    pub fn as_u64(self) -> u64 {
        self.0.max(0) as u64
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }
}

impl Default for DurationMs {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<i64> for DurationMs {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<DurationMs> for i64 {
    fn from(value: DurationMs) -> Self {
        value.0
    }
}

impl Add for DurationMs {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for DurationMs {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign for DurationMs {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for DurationMs {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Sub for TimestampMs {
    type Output = DurationMs;

    fn sub(self, rhs: Self) -> Self::Output {
        DurationMs(self.0.saturating_sub(rhs.0))
    }
}

impl Add<DurationMs> for TimestampMs {
    type Output = Self;

    fn add(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub<DurationMs> for TimestampMs {
    type Output = Self;

    fn sub(self, rhs: DurationMs) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// String expression evaluated at runtime for numeric values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExprString(pub String);

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

impl PieceTypeId {
    pub fn is_king(&self) -> bool {
        self.0 == "king"
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
