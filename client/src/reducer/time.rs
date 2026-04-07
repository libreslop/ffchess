//! Time helpers for reducer modules.

use common::types::TimestampMs;

/// Returns the current wall-clock timestamp in milliseconds.
pub fn now_timestamp_ms() -> TimestampMs {
    #[cfg(target_arch = "wasm32")]
    {
        TimestampMs::from_millis(js_sys::Date::now() as i64)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        TimestampMs::from_millis(chrono::Utc::now().timestamp_millis())
    }
}
