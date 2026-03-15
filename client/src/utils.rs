use common::types::{DurationMs, ModeId, PlayerId, SessionSecret, TimestampMs};
use uuid::Uuid;

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

pub fn set_stored_name(name: &str) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_name", name);
    }
}

fn storage_key(base: &str, mode_id: &ModeId) -> String {
    format!("{base}_{}", mode_id.as_ref())
}

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

pub fn set_stored_id(mode_id: &ModeId, id: PlayerId) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_player_id", mode_id);
        let _ = storage.set_item(&key, &id.to_string());
        let _ = storage.remove_item("ffchess_player_id");
    }
}

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

pub fn set_stored_secret(mode_id: &ModeId, secret: SessionSecret) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let key = storage_key("ffchess_session_secret", mode_id);
        let _ = storage.set_item(&key, &secret.to_string());
        let _ = storage.remove_item("ffchess_session_secret");
    }
}

pub fn clear_stored_session(mode_id: &ModeId) {
    if let Ok(Some(storage)) = web_sys::window()
        .expect("no global `window` exists")
        .local_storage()
    {
        let _ = storage.remove_item(&storage_key("ffchess_player_id", mode_id));
        let _ = storage.remove_item(&storage_key("ffchess_session_secret", mode_id));
    }
}

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

pub fn is_mobile() -> bool {
    let window = web_sys::window().expect("no global `window` exists");
    let navigator = window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default().to_lowercase();

    user_agent.contains("mobi")
        || user_agent.contains("android")
        || user_agent.contains("iphone")
        || user_agent.contains("ipad")
}

pub fn request_fullscreen() {
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let element = document
        .document_element()
        .expect("should have a document element");

    let _ = element.request_fullscreen();
}
