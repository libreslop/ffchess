//! Client configuration loading and ordering helpers.

use common::models::ModeSummary;
use common::types::ModeId;
use gloo_utils::document;
use serde::Deserialize;

/// Client-wide configuration loaded from the server-injected JSON.
#[derive(Clone, Deserialize, Default, PartialEq)]
pub struct GlobalClientConfig {
    pub game_order: Vec<ModeId>,
    pub modes_refresh_ms: u32,
    pub ping_interval_ms: u32,
    pub tick_interval_ms: u32,
    pub render_interval_ms: u32,
    pub disconnected_hide_ms: u32,
    pub fatal_auto_hide_ms: u32,
    pub camera_zoom_min: f64,
    pub camera_zoom_max: f64,
    pub zoom_lerp: f64,
    pub inertia_decay: f64,
    pub velocity_cutoff: f64,
    pub pan_lerp_alive: f64,
    pub pan_lerp_dead: f64,
    pub tile_size_px: f64,
    pub death_zoom: f64,
    pub scroll_zoom_base: f64,
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
    GlobalClientConfig {
        game_order: vec![],
        modes_refresh_ms: 5000,
        ping_interval_ms: 2000,
        tick_interval_ms: 50,
        render_interval_ms: 16,
        disconnected_hide_ms: 300,
        fatal_auto_hide_ms: 5000,
        camera_zoom_min: 0.2,
        camera_zoom_max: 2.0,
        zoom_lerp: 0.15,
        inertia_decay: 0.94,
        velocity_cutoff: 0.1,
        pan_lerp_alive: 0.15,
        pan_lerp_dead: 0.08,
        tile_size_px: 40.0,
        death_zoom: 1.3,
        scroll_zoom_base: 1.2,
    }
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
