//! Client configuration loading and ordering helpers.

use common::models::ModeSummary;
use common::types::ModeId;
use educe::Educe;
use gloo_utils::document;
use serde::Deserialize;

/// Client-wide configuration loaded from the server-injected JSON.
#[derive(Clone, Deserialize, PartialEq, Educe)]
#[serde(default)]
#[educe(Default)]
pub struct GlobalClientConfig {
    #[educe(Default = vec![])]
    pub game_order: Vec<ModeId>,
    /// Modes offered to play while waiting in a matchmaking queue, in display order.
    #[educe(Default = vec![])]
    pub quick_play_modes: Vec<ModeId>,
    /// Piece icons, by file stem under `assets/pieces`, one of which is picked at random to
    /// sit beside the title.
    #[educe(Default = vec![])]
    pub title_pieces: Vec<String>,
    #[educe(Default = 5000)]
    pub modes_refresh_ms: u32,
    #[educe(Default = 2000)]
    pub ping_interval_ms: u32,
    #[educe(Default = 50)]
    pub tick_interval_ms: u32,
    #[educe(Default = 16)]
    pub render_interval_ms: u32,
    #[educe(Default = 300)]
    pub disconnected_hide_ms: u32,
    #[educe(Default = 5000)]
    pub fatal_auto_hide_ms: u32,
    #[educe(Default = 0.2)]
    pub camera_zoom_min: f64,
    #[educe(Default = 2.0)]
    pub camera_zoom_max: f64,
    #[educe(Default = 0.15)]
    pub zoom_lerp: f64,
    #[educe(Default = 0.94)]
    pub inertia_decay: f64,
    #[educe(Default = 0.1)]
    pub velocity_cutoff: f64,
    #[educe(Default = 0.15)]
    pub pan_lerp_alive: f64,
    #[educe(Default = 0.08)]
    pub pan_lerp_dead: f64,
    #[educe(Default = 40.0)]
    pub tile_size_px: f64,
    #[educe(Default = 1.3)]
    pub death_zoom: f64,
    #[educe(Default = 1.2)]
    pub scroll_zoom_base: f64,
    #[educe(Default = 10000)]
    pub chat_message_ttl_ms: u32,
    #[educe(Default = 150)]
    pub chat_message_max_chars: u32,
    #[educe(Default = 100)]
    pub chat_warning_chars: u32,
}

/// Loads the global client config from the server-injected DOM payload.
///
/// Returns the parsed `GlobalClientConfig`, falling back to defaults on error.
pub fn load_global_config() -> GlobalClientConfig {
    let doc = document();
    if let Some(el) = doc.get_element_by_id("initial-global")
        && let Some(text) = el.text_content()
        && let Ok(cfg) = serde_json::from_str::<GlobalClientConfig>(&text)
    {
        return cfg;
    }
    GlobalClientConfig::default()
}

/// Picks the configured quick-play modes out of the mode list, in configured order.
///
/// `modes` is the live mode list, `quick_play` the configured ids. Unknown ids are skipped.
pub fn quick_play_modes(modes: &[ModeSummary], quick_play: &[ModeId]) -> Vec<ModeSummary> {
    quick_play
        .iter()
        .filter_map(|id| modes.iter().find(|mode| &mode.id == id))
        .cloned()
        .collect()
}

/// Sorts mode summaries using a preferred order list.
///
/// `list` is the modes to sort, `order` is the preferred id sequence.
/// Returns the sorted mode list.
pub fn order_modes(mut list: Vec<ModeSummary>, order: &[ModeId]) -> Vec<ModeSummary> {
    list.sort_by_key(|m| {
        order
            .iter()
            .position(|id| id == &m.id)
            .unwrap_or(order.len())
    });
    list
}
