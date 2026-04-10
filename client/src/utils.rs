//! Browser storage and environment helpers for the client.

use common::types::{DurationMs, ModeId, PlayerId, SessionSecret, TimestampMs};
use js_sys::{Function, Reflect};
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};

/// Reads the stored player name from local storage.
///
/// Returns the stored name or an empty string if missing.
pub fn get_stored_name() -> String {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage
            .get_item("ffchess_name")
            .unwrap_or_default()
            .unwrap_or_default();
    }
    String::new()
}

/// Stores the player name in local storage.
///
/// `name` is the display name to persist. Returns nothing.
pub fn set_stored_name(name: &str) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_name", name);
    }
}

/// Builds a namespaced local storage key for a mode.
///
/// `base` is the key prefix and `mode_id` is the mode identifier.
/// Returns the composed storage key string.
fn storage_key(base: &str, mode_id: &ModeId) -> String {
    format!("{base}_{}", mode_id.as_ref())
}

/// Reads a stored player id for the given mode.
///
/// `mode_id` selects the mode. Returns the stored `PlayerId` if present.
pub fn get_stored_id(mode_id: &ModeId) -> Option<PlayerId> {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_player_id", mode_id);
        return storage
            .get_item(&key)
            .unwrap_or_else(|_| storage.get_item("ffchess_player_id").ok().flatten())
            .and_then(|s| Uuid::parse_str(&s).ok())
            .map(PlayerId::from);
    }
    None
}

/// Stores a player id for the given mode.
///
/// `mode_id` selects the mode and `id` is the player id to persist.
/// Returns nothing.
pub fn set_stored_id(mode_id: &ModeId, id: PlayerId) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_player_id", mode_id);
        let _ = storage.set_item(&key, &id.to_string());
        let _ = storage.remove_item("ffchess_player_id");
    }
}

/// Reads a stored session secret for the given mode.
///
/// `mode_id` selects the mode. Returns the stored secret if present.
pub fn get_stored_secret(mode_id: &ModeId) -> Option<SessionSecret> {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_session_secret", mode_id);
        return storage
            .get_item(&key)
            .unwrap_or_else(|_| storage.get_item("ffchess_session_secret").ok().flatten())
            .and_then(|s| Uuid::parse_str(&s).ok())
            .map(SessionSecret::from);
    }
    None
}

/// Stores a session secret for the given mode.
///
/// `mode_id` selects the mode and `secret` is the session token to persist.
/// Returns nothing.
pub fn set_stored_secret(mode_id: &ModeId, secret: SessionSecret) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_session_secret", mode_id);
        let _ = storage.set_item(&key, &secret.to_string());
        let _ = storage.remove_item("ffchess_session_secret");
    }
}

/// Clears stored player id and session secret for the given mode.
///
/// `mode_id` selects the mode. Returns nothing.
pub fn clear_stored_session(mode_id: &ModeId) {
    if let Ok(Some(storage)) = web_sys::window()
        .expect("no global `window` exists")
        .local_storage()
    {
        let _ = storage.remove_item(&storage_key("ffchess_player_id", mode_id));
        let _ = storage.remove_item(&storage_key("ffchess_session_secret", mode_id));
    }
}

/// Reads the last death timestamp and cooldown for a mode.
///
/// `mode_id` selects the mode. Returns a tuple of `(TimestampMs, DurationMs)`.
pub fn get_death_info(mode_id: &ModeId) -> (TimestampMs, DurationMs) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let ts = storage
            .get_item(&storage_key("ffchess_death_ts", mode_id))
            .unwrap_or_default()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let cd = storage
            .get_item(&storage_key("ffchess_death_cd", mode_id))
            .unwrap_or_default()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(5000);
        return (TimestampMs::from_millis(ts), DurationMs::from_millis(cd));
    }
    (TimestampMs::from_millis(0), DurationMs::from_millis(5000))
}

/// Stores the latest death timestamp and respawn cooldown.
///
/// `mode_id` selects the mode, `ts` is the death time, `cooldown_ms` is the cooldown.
/// Returns nothing.
pub fn set_death_timestamp(mode_id: &ModeId, ts: TimestampMs, cooldown_ms: DurationMs) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(
            &storage_key("ffchess_death_ts", mode_id),
            &ts.as_i64().to_string(),
        );
        let _ = storage.set_item(
            &storage_key("ffchess_death_cd", mode_id),
            &cooldown_ms.as_i64().to_string(),
        );
    }
}

/// Detects whether the current user agent is a mobile device.
///
/// Returns `true` for mobile-like user agents.
pub fn is_mobile() -> bool {
    let window = web_sys::window().expect("no global `window` exists");
    let navigator = window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default().to_lowercase();

    user_agent.contains("mobi")
        || user_agent.contains("android")
        || user_agent.contains("iphone")
        || user_agent.contains("ipad")
}

/// Requests the document to enter fullscreen mode if supported.
///
/// Returns nothing.
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
    if root.request_fullscreen().is_ok() {
        return;
    }

    // iOS Safari may expose only prefixed fullscreen APIs.
    if try_prefixed_fullscreen(&root) {
        return;
    }
    if let Some(body) = document.body() {
        let body_element: web_sys::Element = body.into();
        if try_prefixed_fullscreen(&body_element) {
            return;
        }
    }

    // Fallback for iOS when Fullscreen API is unavailable: collapse browser chrome.
    let _ = window.scroll_to_with_x_and_y(0.0, 1.0);
}

fn try_prefixed_fullscreen(element: &web_sys::Element) -> bool {
    for method in ["webkitRequestFullscreen", "webkitEnterFullscreen"] {
        if call_method_no_args(element.as_ref(), method) {
            return true;
        }
    }
    false
}

fn call_method_no_args(target: &JsValue, method_name: &str) -> bool {
    let Ok(method) = Reflect::get(target, &JsValue::from_str(method_name)) else {
        return false;
    };
    let Some(function) = method.dyn_ref::<Function>() else {
        return false;
    };
    function.call0(target).is_ok()
}
