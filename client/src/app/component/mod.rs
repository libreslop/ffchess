//! Root Yew component orchestrating UI state and network connections.

mod callbacks;
mod effects;
mod view;

use callbacks::{build_on_join, build_on_name_input, build_on_name_submit, build_on_rejoin};
use effects::{
    KeyboardShortcutEffectInputs, use_disconnected_overlay_effect, use_fatal_error_reset_effect,
    use_joining_reset_effect, use_keyboard_shortcuts_effect, use_landing_cooldown_effect,
    use_mode_refresh_effect, use_mode_url_navigation_effect, use_player_name_sync_effect,
    use_preview_default_effect, use_rejoin_cooldown_effect, use_rejoin_flow_reset_effect,
    use_ws_connection_effect,
};
use view::{AppViewProps, render_app};

use crate::app::config::{load_global_config, order_modes};
use crate::reducer::{ClientPhase, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{get_death_info, get_stored_name};
use common::models::ModeSummary;
use common::types::{ModeId, PlayerId};
use gloo_utils::document;
use yew::prelude::*;

#[function_component(App)]
/// Renders the application shell and routes UI state to child components.
pub fn app() -> Html {
    let global_cfg = use_state(load_global_config);
    let reducer = use_reducer(GameStateReducer::default);
    let is_joining = use_state(|| false);
    let rejoin_flow = use_state(RejoinFlow::default);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(get_stored_name);
    let join_step = use_state(JoinStep::default);
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);
    let preview_default_ref = use_mut_ref(|| None::<bool>);
    // Read initial mode list injected into index.html for immediate render
    let injected_modes: Vec<ModeSummary> = {
        let doc = document();
        if let Some(el) = doc.get_element_by_id("initial-modes") {
            if let Some(text) = el.text_content() {
                serde_json::from_str::<Vec<ModeSummary>>(&text).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };
    let injected_modes_copy = injected_modes.clone();

    let mode_options = {
        let order = global_cfg.game_order.clone();
        use_state(move || order_modes(injected_modes.clone(), &order))
    };

    // Read current mode info injected in index.html
    let injected_mode_info: Option<ModeSummary> = injected_modes_copy.first().cloned();

    // Determine current mode id from hash or global order/injected info
    let initial_mode_id = {
        let hash = web_sys::window()
            .unwrap()
            .location()
            .hash()
            .unwrap_or_default()
            .trim_start_matches('#')
            .to_string();
        if !hash.is_empty() {
            ModeId::from(hash)
        } else if let Some(first) = global_cfg.game_order.first() {
            first.clone()
        } else if let Some(m) = injected_mode_info.as_ref() {
            m.id.clone()
        } else {
            ModeId::from("ffa")
        }
    };
    let current_mode_id = use_state(|| initial_mode_id.clone());

    use_mode_refresh_effect(
        mode_options.clone(),
        injected_mode_info.clone(),
        global_cfg.clone(),
    );
    use_joining_reset_effect(is_joining.clone(), reducer.clone());
    use_fatal_error_reset_effect(reducer.clone());

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
        tx_ref.clone(),
    );

    let landing_cooldown = {
        let initial_mode_id = initial_mode_id.clone();
        use_state(move || {
            let (ts, cd_ms) = get_death_info(&initial_mode_id);
            let now = common::types::TimestampMs::from_millis(js_sys::Date::now() as i64);
            let diff_ms = cd_ms - (now - ts);
            if diff_ms > common::types::DurationMs::zero() {
                CooldownSeconds::from_seconds((diff_ms.as_u64() / 1000) as u32)
            } else {
                CooldownSeconds::zero()
            }
        })
    };
    let lc_ref = use_mut_ref(|| *landing_cooldown);
    use_landing_cooldown_effect(landing_cooldown.clone(), lc_ref.clone());

    use_ws_connection_effect(
        current_mode_id.clone(),
        reducer_ref.clone(),
        tx.clone(),
        global_cfg.clone(),
    );

    let on_join = build_on_join(
        tx.clone(),
        player_name.clone(),
        reducer_ref.clone(),
        is_joining.clone(),
        has_interacted.clone(),
        current_mode_id.clone(),
    );
    let kits = reducer
        .mode
        .as_ref()
        .map(|m| m.kits.clone())
        .unwrap_or_default();
    let single_kit = (kits.len() == 1).then(|| kits[0].name.clone());
    let on_name_input = build_on_name_input(player_name.clone());
    let on_name_submit = build_on_name_submit(
        join_step.clone(),
        player_name.clone(),
        landing_cooldown.clone(),
        reducer.clone(),
        has_interacted.clone(),
        single_kit.clone(),
        on_join.clone(),
    );

    let player_id = reducer.player_id.unwrap_or_else(PlayerId::nil);
    let is_dead = reducer.is_dead;
    let is_victory = reducer.is_victory;
    let has_match_result = is_dead || is_victory;
    let is_joined = reducer.phase == ClientPhase::Alive || has_match_result;
    let is_queueing = reducer.queue_status.is_some();
    let force_join_overlay = rejoin_flow.forces_join_overlay(has_match_result);

    use_player_name_sync_effect(player_name.clone(), reducer.clone());
    use_disconnected_overlay_effect(
        show_disconnected.clone(),
        reducer.clone(),
        is_joined,
        is_queueing,
        has_match_result,
    );
    use_rejoin_flow_reset_effect(rejoin_flow.clone(), reducer.clone(), has_match_result);
    use_preview_default_effect(
        tx.clone(),
        join_step.clone(),
        reducer.clone(),
        preview_default_ref.clone(),
        is_joined,
    );

    let rejoin_cooldown = use_state(CooldownSeconds::zero);
    let rc_ref = use_mut_ref(CooldownSeconds::zero);
    use_rejoin_cooldown_effect(
        rejoin_cooldown.clone(),
        rc_ref.clone(),
        current_mode_id.clone(),
        reducer.clone(),
        has_match_result,
    );

    let on_rejoin = build_on_rejoin(
        rc_ref.clone(),
        reducer.clone(),
        join_step.clone(),
        has_interacted.clone(),
        rejoin_flow.clone(),
    );

    let disconnected = reducer.disconnected;
    let queueing = is_queueing;
    use_keyboard_shortcuts_effect(KeyboardShortcutEffectInputs {
        is_joined,
        is_dead,
        is_victory,
        join_step: join_step.clone(),
        landing_cooldown: landing_cooldown.clone(),
        disconnected,
        queueing,
        kits,
        single_kit,
        player_name: player_name.clone(),
        has_interacted: has_interacted.clone(),
        on_join: on_join.clone(),
        on_rejoin: on_rejoin.clone(),
        rc_ref: rc_ref.clone(),
    });

    let on_select_mode = {
        Callback::from(move |id: ModeId| {
            let window = web_sys::window().unwrap();
            let _ = window.location().set_hash(&format!("#{}", id.as_ref()));
        })
    };

    let view_props = AppViewProps {
        global_cfg: (*global_cfg).clone(),
        reducer: reducer.clone(),
        tx: (*tx).clone(),
        show_disconnected: *show_disconnected,
        has_interacted: *has_interacted,
        has_match_result,
        is_joined,
        is_victory,
        force_join_overlay,
        player_id,
        player_name: (*player_name).clone(),
        join_step: *join_step,
        on_name_input,
        on_name_submit,
        on_join,
        landing_cooldown: *landing_cooldown,
        queue_status: reducer.queue_status.clone(),
        is_joining: *is_joining,
        mode: reducer.mode.clone(),
        mode_options: (*mode_options).clone(),
        selected_mode_id: (*current_mode_id).clone(),
        on_select_mode,
        on_rejoin,
        rejoin_cooldown: *rejoin_cooldown,
    };

    render_app(view_props)
}
