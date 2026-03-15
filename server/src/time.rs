use common::types::TimestampMs;

/// Current time in milliseconds since epoch.
pub fn now_ms() -> TimestampMs {
    TimestampMs::from_millis(chrono::Utc::now().timestamp_millis())
}
