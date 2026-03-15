//! Time helpers for server-side clocks.

use common::types::TimestampMs;

/// Current time in milliseconds since epoch.
///
/// Returns the current UTC timestamp as `TimestampMs`.
pub fn now_ms() -> TimestampMs {
    TimestampMs::from_millis(chrono::Utc::now().timestamp_millis())
}
