//! Browser storage and environment helpers for the client.

use common::types::{DurationMs, ModeId, PlayerId, SessionSecret, TimestampMs};
use js_sys::{Function, Reflect};
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Storage;

/// Storage key of the last entered player name.
const NAME_KEY: &str = "ffchess_name";

/// Respawn cooldown assumed when none was stored.
const DEFAULT_RESPAWN_COOLDOWN: DurationMs = DurationMs(5000);

/// Returns the browser's local storage, if available.
fn local_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Reads one local storage entry.
fn read(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok().flatten()
}

/// Writes one local storage entry, ignoring storage failures.
fn write(key: &str, value: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
}

/// Removes one local storage entry, ignoring storage failures.
fn remove(key: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
}

/// Returns the last entered player name, or an empty string.
pub fn stored_name() -> String {
    read(NAME_KEY).unwrap_or_default()
}

/// Remembers the entered player name.
pub fn store_name(name: &str) {
    write(NAME_KEY, name);
}

/// When the player last died in a mode and how long respawning was blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeathRecord {
    pub died_at: TimestampMs,
    pub cooldown: DurationMs,
}

impl DeathRecord {
    /// Returns how long respawning is still blocked at `now`.
    pub fn remaining(&self, now: TimestampMs) -> DurationMs {
        (self.cooldown - (now - self.died_at)).max(DurationMs::zero())
    }
}

/// Per-mode persisted session: player id, rejoin secret, and last death.
pub struct SessionStore {
    suffix: String,
}

impl SessionStore {
    /// Opens the stored session of a mode.
    pub fn new(mode_id: &ModeId) -> Self {
        Self {
            suffix: mode_id.as_ref().to_string(),
        }
    }

    /// Builds the storage key of one session field.
    fn key(&self, field: &str) -> String {
        format!("ffchess_{field}_{}", self.suffix)
    }

    /// Returns the stored player id and rejoin secret.
    pub fn load(&self) -> (Option<PlayerId>, Option<SessionSecret>) {
        let uuid = |field| read(&self.key(field)).and_then(|s| Uuid::parse_str(&s).ok());
        (
            uuid("player_id").map(PlayerId::from),
            uuid("session_secret").map(SessionSecret::from),
        )
    }

    /// Stores the player id and rejoin secret.
    pub fn save(&self, player_id: PlayerId, secret: SessionSecret) {
        write(&self.key("player_id"), &player_id.to_string());
        write(&self.key("session_secret"), &secret.to_string());
    }

    /// Forgets the player id and rejoin secret.
    pub fn clear(&self) {
        remove(&self.key("player_id"));
        remove(&self.key("session_secret"));
    }

    /// Returns the last recorded death.
    pub fn death(&self) -> DeathRecord {
        let millis = |field| read(&self.key(field)).and_then(|s| s.parse::<i64>().ok());
        DeathRecord {
            died_at: TimestampMs::from_millis(millis("death_ts").unwrap_or(0)),
            cooldown: millis("death_cd").map_or(DEFAULT_RESPAWN_COOLDOWN, DurationMs::from_millis),
        }
    }

    /// Records a death and the respawn cooldown that follows it.
    pub fn record_death(&self, record: DeathRecord) {
        write(&self.key("death_ts"), &record.died_at.as_i64().to_string());
        write(&self.key("death_cd"), &record.cooldown.as_i64().to_string());
    }
}

/// Returns the current local wall-clock time.
pub fn local_now() -> TimestampMs {
    TimestampMs::from_millis(js_sys::Date::now() as i64)
}

/// Detects whether the current user agent is a mobile device.
pub fn is_mobile() -> bool {
    let user_agent = web_sys::window()
        .and_then(|window| window.navigator().user_agent().ok())
        .unwrap_or_default()
        .to_lowercase();
    ["mobi", "android", "iphone", "ipad"]
        .iter()
        .any(|marker| user_agent.contains(marker))
}

/// Requests the document to enter fullscreen mode if supported.
pub fn request_fullscreen() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.fullscreen_element().is_some() {
        return;
    }
    let Some(root) = document.document_element() else {
        return;
    };
    if root.request_fullscreen().is_ok() || try_prefixed_fullscreen(&root) {
        return;
    }
    // iOS Safari may expose only prefixed fullscreen APIs, sometimes only on the body.
    if let Some(body) = document.body()
        && try_prefixed_fullscreen(&body.into())
    {
        return;
    }
    // Fallback for iOS when the Fullscreen API is unavailable: collapse browser chrome.
    window.scroll_to_with_x_and_y(0.0, 1.0);
}

/// Tries the vendor-prefixed fullscreen methods on `element`.
fn try_prefixed_fullscreen(element: &web_sys::Element) -> bool {
    ["webkitRequestFullscreen", "webkitEnterFullscreen"]
        .iter()
        .any(|method| call_method_no_args(element.as_ref(), method))
}

/// Calls a zero-argument JS method by name, returning whether the call succeeded.
fn call_method_no_args(target: &JsValue, method_name: &str) -> bool {
    Reflect::get(target, &JsValue::from_str(method_name))
        .ok()
        .and_then(|method| method.dyn_into::<Function>().ok())
        .is_some_and(|function| function.call0(target).is_ok())
}

/// Returns the current time in milliseconds from the high-resolution performance clock.
pub fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now())
}
