//! Share links of replays: the page address with `?replay=<id>`, which opens the replay
//! viewer as soon as the page loads.

use common::types::{ModeId, ReplayId};
use uuid::Uuid;
use wasm_bindgen::JsValue;
use web_sys::Url;

/// Query parameter naming the replay to open.
const REPLAY_PARAM: &str = "replay";

/// The replay link of the page address.
pub struct ReplayLink;

impl ReplayLink {
    /// Returns the replay the page address asks to open, if any.
    pub fn requested() -> Option<ReplayId> {
        let value = page_url()?.search_params().get(REPLAY_PARAM)?;
        Uuid::parse_str(&value).ok().map(ReplayId::from)
    }

    /// Returns the link that opens `replay_id` over the menu of `mode_id`.
    pub fn share_url(replay_id: ReplayId, mode_id: &ModeId) -> Option<String> {
        let url = page_url()?;
        url.set_search("");
        url.search_params()
            .set(REPLAY_PARAM, &replay_id.to_string());
        url.set_hash(mode_id.as_ref());
        Some(url.href())
    }

    /// Drops the replay from the page address, so reloading the page no longer opens it.
    pub fn clear() {
        let Some(url) = page_url().filter(|url| url.search_params().has(REPLAY_PARAM)) else {
            return;
        };
        url.search_params().delete(REPLAY_PARAM);
        let _ = web_sys::window().and_then(|window| {
            window
                .history()
                .ok()?
                .replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
                .ok()
        });
    }
}

/// Returns the address of the page.
fn page_url() -> Option<Url> {
    let href = web_sys::window()?.location().href().ok()?;
    Url::new(&href).ok()
}
