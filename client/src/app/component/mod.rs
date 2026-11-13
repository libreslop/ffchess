//! Root Yew component orchestrating UI state and network connections.

mod callbacks;
mod effects;
mod view;

use crate::app::browser::PageMetadata;
use crate::app::config::{GlobalClientConfig, load_global_config, order_modes};
use crate::reducer::{ClientPhase, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{SessionStore, local_now, stored_name};
use callbacks::{
    JoinInputs, NameSubmitInputs, RejoinInputs, build_on_join, build_on_name_input,
    build_on_name_submit, build_on_rejoin,
};
use common::models::{GameModeClientConfig, ModeSummary};
use common::types::ModeId;
use effects::{
    KeyboardShortcutEffectInputs, use_disconnected_overlay_effect, use_error_auto_clear_effect,
    use_fatal_error_reset_effect, use_joining_reset_effect, use_keyboard_shortcuts_effect,
    use_landing_cooldown_effect, use_mode_refresh_effect, use_mode_url_navigation_effect,
    use_page_metadata_effect, use_player_name_sync_effect, use_preview_default_effect,
    use_rejoin_cooldown_effect, use_rejoin_flow_reset_effect, use_single_kit_step_reset_effect,
    use_ws_connection_effect,
};
use gloo_utils::document;
use view::{AppViewProps, render_app};
use yew::prelude::*;

/// Renders the application shell and routes UI state to child components.
#[function_component(App)]
pub fn app() -> Html {
    let global_cfg = use_state(load_global_config);
    let reducer = use_reducer(GameStateReducer::default);
    let is_joining = use_state(|| false);
    let rejoin_flow = use_state(RejoinFlow::default);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(stored_name);
    let join_step = use_state(JoinStep::default);
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);
    let preview_default_ref = use_mut_ref(|| None::<bool>);

    let injected_modes = use_memo((), |_| injected_modes());
    let injected_mode_info = injected_modes.first().cloned();
    let mode_options = {
        let order = global_cfg.game_order.clone();
        let injected_modes = injected_modes.clone();
        use_state(move || order_modes((*injected_modes).clone(), &order))
    };

    let initial_mode_id = initial_mode_id(&global_cfg, injected_mode_info.as_ref());
    let current_mode_id = use_state(|| initial_mode_id.clone());
    use_page_metadata_effect(resolve_page_metadata(
        &reducer,
        &current_mode_id,
        &mode_options,
    ));

    use_mode_refresh_effect(mode_options.clone(), injected_mode_info, global_cfg.clone());
    use_joining_reset_effect(is_joining.clone(), reducer.clone());
    use_error_auto_clear_effect(reducer.clone());
    use_fatal_error_reset_effect(reducer.clone(), global_cfg.fatal_auto_hide_ms);

    let reducer_ref = use_mut_ref(|| reducer.clone());
    *reducer_ref.borrow_mut() = reducer.clone();
    let tx_ref = use_mut_ref(|| (*tx).clone());
    *tx_ref.borrow_mut() = (*tx).clone();

    use_mode_url_navigation_effect(
        current_mode_id.clone(),
        initial_mode_id.clone(),
        reducer_ref.clone(),
        join_step.clone(),
        rejoin_flow.clone(),
        tx_ref,
    );

    let landing_cooldown = {
        let initial_mode_id = initial_mode_id.clone();
        use_state(move || {
            let remaining = SessionStore::new(&initial_mode_id)
                .death()
                .remaining(local_now());
            CooldownSeconds::from_duration(remaining)
        })
    };
    let lc_ref = use_mut_ref(|| *landing_cooldown);
    use_landing_cooldown_effect(landing_cooldown.clone(), lc_ref);

    use_ws_connection_effect(
        current_mode_id.clone(),
        reducer_ref.clone(),
        tx.clone(),
        global_cfg.clone(),
    );

    let on_join = build_on_join(JoinInputs {
        tx: tx.clone(),
        player_name: player_name.clone(),
        reducer: reducer_ref,
        is_joining: is_joining.clone(),
        has_interacted: has_interacted.clone(),
        current_mode_id: current_mode_id.clone(),
    });
    let kits = reducer
        .mode
        .as_ref()
        .map(|mode| mode.kits.clone())
        .unwrap_or_default();
    let single_kit = (kits.len() == 1).then(|| kits[0].name.clone());
    use_single_kit_step_reset_effect(
        join_step.clone(),
        reducer.clone(),
        *is_joining,
        single_kit.is_some(),
    );
    let on_name_submit = build_on_name_submit(NameSubmitInputs {
        join_step: join_step.clone(),
        player_name: player_name.clone(),
        landing_cooldown: landing_cooldown.clone(),
        reducer: reducer.clone(),
        has_interacted: has_interacted.clone(),
        single_kit: single_kit.clone(),
        on_join: on_join.clone(),
    });

    let has_match_result = reducer.has_match_result();
    let is_joined = reducer.phase == ClientPhase::Alive || has_match_result;
    let is_queueing = reducer.queue_status.is_some();

    use_player_name_sync_effect(player_name.clone(), reducer.clone());
    use_disconnected_overlay_effect(
        show_disconnected.clone(),
        reducer.clone(),
        is_joined,
        is_queueing,
        global_cfg.disconnected_hide_ms,
    );
    use_rejoin_flow_reset_effect(rejoin_flow.clone(), reducer.clone());
    use_preview_default_effect(
        tx.clone(),
        join_step.clone(),
        reducer.clone(),
        preview_default_ref,
        is_joined,
    );

    let rejoin_cooldown = use_state(CooldownSeconds::zero);
    let rc_ref = use_mut_ref(CooldownSeconds::zero);
    use_rejoin_cooldown_effect(
        rejoin_cooldown.clone(),
        rc_ref.clone(),
        current_mode_id.clone(),
        reducer.clone(),
    );

    let on_rejoin = build_on_rejoin(RejoinInputs {
        rejoin_cooldown: rc_ref.clone(),
        reducer: reducer.clone(),
        join_step: join_step.clone(),
        has_interacted: has_interacted.clone(),
        rejoin_flow: rejoin_flow.clone(),
        single_kit: single_kit.clone(),
        on_join: on_join.clone(),
    });

    let on_cycle_mode = {
        let mode_options = mode_options.clone();
        let current_mode_id = current_mode_id.clone();
        Callback::from(move |delta: i32| {
            let current_index = mode_options
                .iter()
                .position(|mode| mode.id == *current_mode_id)
                .unwrap_or(0);
            let next_mode = current_index
                .checked_add_signed(delta as isize)
                .and_then(|index| mode_options.get(index));
            if let Some(next_mode) = next_mode.filter(|mode| mode.id != *current_mode_id) {
                navigate_to_mode(&next_mode.id);
            }
        })
    };

    use_keyboard_shortcuts_effect(KeyboardShortcutEffectInputs {
        is_joined,
        is_dead: reducer.is_dead,
        is_victory: reducer.is_victory(),
        join_step: join_step.clone(),
        landing_cooldown: landing_cooldown.clone(),
        disconnected: reducer.disconnected,
        queueing: is_queueing,
        kits,
        single_kit,
        player_name: player_name.clone(),
        has_interacted: has_interacted.clone(),
        on_join: on_join.clone(),
        on_cycle_mode: on_cycle_mode.clone(),
        on_rejoin: on_rejoin.clone(),
        rc_ref,
    });

    render_app(AppViewProps {
        global_cfg: (*global_cfg).clone(),
        reducer: reducer.clone(),
        tx: (*tx).clone(),
        show_disconnected: *show_disconnected,
        has_interacted: *has_interacted,
        is_joined,
        force_join_overlay: rejoin_flow.forces_join_overlay(has_match_result),
        player_name: (*player_name).clone(),
        join_step: *join_step,
        on_name_input: build_on_name_input(player_name),
        on_name_submit,
        on_join,
        landing_cooldown: *landing_cooldown,
        is_joining: *is_joining,
        mode_options: (*mode_options).clone(),
        selected_mode_id: (*current_mode_id).clone(),
        on_select_mode: Callback::from(|id: ModeId| navigate_to_mode(&id)),
        on_cycle_mode,
        on_rejoin,
        rejoin_cooldown: *rejoin_cooldown,
    })
}

/// Switches modes by changing the URL hash; the navigation effect does the rest.
fn navigate_to_mode(mode_id: &ModeId) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&format!("#{mode_id}"));
    }
}

/// Reads the mode list the server injected into the page.
fn injected_modes() -> Vec<ModeSummary> {
    document()
        .get_element_by_id("initial-modes")
        .and_then(|element| element.text_content())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Picks the mode from the URL hash, the configured order, or the injected list.
fn initial_mode_id(
    global_cfg: &GlobalClientConfig,
    injected_mode_info: Option<&ModeSummary>,
) -> ModeId {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    let hash = hash.trim_start_matches('#');
    if !hash.is_empty() {
        ModeId::from(hash)
    } else if let Some(first) = global_cfg.game_order.first() {
        first.clone()
    } else if let Some(mode) = injected_mode_info {
        mode.id.clone()
    } else {
        ModeId::from("ffa")
    }
}

/// Builds the browser title and favicon for the current view.
fn resolve_page_metadata(
    reducer: &GameStateReducer,
    current_mode_id: &ModeId,
    mode_options: &[ModeSummary],
) -> PageMetadata {
    PageMetadata::new(
        selected_mode_name(current_mode_id, mode_options, reducer.mode.as_ref()),
        local_team_color(reducer),
    )
}

/// Returns the display name of the selected mode.
fn selected_mode_name(
    current_mode_id: &ModeId,
    mode_options: &[ModeSummary],
    reducer_mode: Option<&GameModeClientConfig>,
) -> Option<String> {
    mode_options
        .iter()
        .find(|mode| mode.id == *current_mode_id)
        .map(|mode| mode.display_name.clone())
        .or_else(|| {
            reducer_mode
                .filter(|mode| mode.id == *current_mode_id)
                .map(|mode| mode.display_name.clone())
        })
}

/// Returns the local player's team color while alive.
fn local_team_color(reducer: &GameStateReducer) -> Option<String> {
    if reducer.phase != ClientPhase::Alive || reducer.has_match_result() {
        return None;
    }
    reducer
        .active_player_id()
        .and_then(|player_id| reducer.state.players.get(&player_id))
        .map(|player| player.color.as_ref().to_string())
}
