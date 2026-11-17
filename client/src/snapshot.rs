//! Main menu prerendered by the server, shown inert while the app loads.

use crate::app::{GlobalClientConfig, order_modes};
use crate::components::JoinScreen;
use crate::ui_state::{CooldownSeconds, JoinStep};
use common::models::ModeSummary;
use common::types::{ModeId, PlayerCount};
use yew::ServerRenderer;
use yew::prelude::*;

/// Color of the title and play button until the app picks a random player color.
const PENDING_COLOR: &str = "#64748b";

/// Properties for the prerendered menu.
#[derive(Properties, PartialEq)]
struct MenuSnapshotProps {
    /// Modes in display order.
    modes: Vec<ModeSummary>,
    globals: GlobalClientConfig,
}

/// Renders the name entry menu with every control a no-op.
#[function_component(MenuSnapshot)]
fn menu_snapshot(props: &MenuSnapshotProps) -> Html {
    let selected_mode_id = props
        .modes
        .first()
        .map_or_else(|| ModeId::from(""), |mode| mode.id.clone());
    html! {
        <JoinScreen
            player_name={String::new()}
            on_name_input={Callback::noop()}
            on_name_submit={Callback::noop()}
            landing_cooldown={CooldownSeconds::default()}
            landing_cooldown_max={CooldownSeconds::default()}
            join_step={JoinStep::default()}
            on_join={Callback::noop()}
            on_spectate={Callback::noop()}
            error={None}
            queue_status={None}
            is_loading={false}
            mode={None}
            mode_options={props.modes.clone()}
            selected_mode_id={selected_mode_id}
            on_select_mode={Callback::noop()}
            on_cycle_mode={Callback::noop()}
            quick_play_modes={Vec::new()}
            quick_play_reveal_ms={props.globals.quick_play_reveal_ms}
            title_pieces={props.globals.title_pieces.clone()}
            title_color={PENDING_COLOR}
            on_quick_play={Callback::noop()}
        />
    }
}

/// Renders the main menu as HTML, before anything about the live server is known.
///
/// `modes` are listed in the order `global_json`, the client config JSON, gives them, with no
/// players online.
pub async fn render_menu_snapshot(modes: Vec<ModeSummary>, global_json: &str) -> String {
    let globals = GlobalClientConfig::from_json(global_json);
    let offline = modes
        .into_iter()
        .map(|mode| ModeSummary {
            players: PlayerCount::zero(),
            ..mode
        })
        .collect();
    let modes = order_modes(offline, &globals.game_order);
    ServerRenderer::<MenuSnapshot>::with_props(move || MenuSnapshotProps { modes, globals })
        .hydratable(false)
        .render()
        .await
}
