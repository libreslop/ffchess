//! Hook helpers for the root application component.

mod browser;
mod connection;
mod keyboard;
mod ui;

pub use browser::use_page_metadata_effect;
pub use connection::{
    use_mode_refresh_effect, use_mode_url_navigation_effect, use_ws_connection_effect,
};
pub use keyboard::{KeyboardShortcutEffectInputs, use_keyboard_shortcuts_effect};
pub use ui::{
    use_disconnected_overlay_effect, use_fatal_error_reset_effect, use_joining_reset_effect,
    use_landing_cooldown_effect, use_player_name_sync_effect, use_preview_default_effect,
    use_rejoin_cooldown_effect, use_rejoin_flow_reset_effect,
};
