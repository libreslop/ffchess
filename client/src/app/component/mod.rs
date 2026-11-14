//! Root Yew component orchestrating UI state and network connections.

mod callbacks;
mod effects;
mod quick_play;
mod spectate;
mod view;

use crate::app::browser::PageMetadata;
use crate::app::config::{GlobalClientConfig, load_global_config, order_modes, quick_play_modes};
use crate::app::focus::{ModeFocus, PerSlot, Slot};
use crate::components::NestedQueue;
use crate::reducer::{ClientPhase, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{SessionStore, local_now, stored_name};
use callbacks::{
    JoinInputs, LeaveInputs, NameSubmitInputs, RejoinInputs, SlotSession, build_on_join,
    build_on_leave, build_on_name_input, build_on_name_submit, build_on_rejoin,
};
use common::models::{GameModeClientConfig, ModeSummary};
use common::types::ModeId;
use effects::{
    KeyboardShortcutEffectInputs, PreviewDefaultRef, use_disconnected_overlay_effect,
    use_error_auto_clear_effect, use_fatal_error_reset_effect, use_joining_reset_effect,
    use_keyboard_shortcuts_effect, use_landing_cooldown_effect, use_mode_refresh_effect,
    use_mode_url_navigation_effect, use_page_metadata_effect, use_player_name_sync_effect,
    use_preview_default_effect, use_rejoin_cooldown_effect, use_rejoin_flow_reset_effect,
    use_single_kit_step_reset_effect, use_ws_connection_effect,
};
use gloo_utils::document;
use quick_play::{
    QuickPlayInputs, build_on_quick_play, build_on_stop_queuing, use_nested_queue_effect,
};
use spectate::{SpectateCallbacks, SpectateInputs, build_spectate_callbacks};
use view::{AppViewProps, render_app};
use yew::prelude::*;

/// Renders the application shell and routes UI state to child components.
///
/// The client holds up to two connections: the mode picked from the menu, and a quick-play
/// game nested in it while its queue waits. Everything below the connections works on the
/// focused one, the game the player sees.
#[function_component(App)]
pub fn app() -> Html {
    let global_cfg = use_state(load_global_config);
    let reducers = PerSlot::new(
        use_reducer(GameStateReducer::default),
        use_reducer(GameStateReducer::default),
    );
    let txs = PerSlot::new(
        use_state(|| None::<MsgSender>),
        use_state(|| None::<MsgSender>),
    );
    let is_joining = use_state(|| false);
    let rejoin_flow = use_state(RejoinFlow::default);
    let player_name = use_state(stored_name);
    let join_step = use_state(JoinStep::default);
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);
    let preview_default_ref: PreviewDefaultRef = use_mut_ref(|| None);

    let injected_modes = use_memo((), |_| injected_modes());
    let injected_mode_info = injected_modes.first().cloned();
    let mode_options = {
        let order = global_cfg.game_order.clone();
        let injected_modes = injected_modes.clone();
        use_state(move || order_modes((*injected_modes).clone(), &order))
    };

    let initial_mode_id = initial_mode_id(&global_cfg, injected_mode_info.as_ref());
    let focus = {
        let initial_mode_id = initial_mode_id.clone();
        use_reducer(move || ModeFocus::new(initial_mode_id))
    };
    use_ws_connection_effect(
        focus.mode_of(Slot::First),
        reducers.get(Slot::First).clone(),
        txs.get(Slot::First).clone(),
        global_cfg.clone(),
    );
    use_ws_connection_effect(
        focus.mode_of(Slot::Second),
        reducers.get(Slot::Second).clone(),
        txs.get(Slot::Second).clone(),
        global_cfg.clone(),
    );
    let sessions = PerSlot::new(
        slot_session(&reducers, &txs, Slot::First),
        slot_session(&reducers, &txs, Slot::Second),
    );
    let focused_slot = focus.focused_slot();
    let reducer = reducers.get(focused_slot).clone();
    let tx = txs.get(focused_slot).clone();
    let focused_session = sessions.get(focused_slot).clone();
    let current_mode_id = focus.focused_mode().clone();
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
    let open_sessions = [Slot::First, Slot::Second]
        .into_iter()
        .filter(|&slot| focus.mode_of(slot).is_some())
        .map(|slot| sessions.get(slot).clone())
        .collect::<Vec<_>>();
    let open_sessions_ref = use_mut_ref(Vec::new);
    *open_sessions_ref.borrow_mut() = open_sessions;

    let leave_inputs = LeaveInputs {
        join_step: join_step.clone(),
        rejoin_flow: rejoin_flow.clone(),
    };
    use_mode_url_navigation_effect(
        focus.clone(),
        initial_mode_id.clone(),
        open_sessions_ref,
        leave_inputs.clone(),
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

    let on_join = build_on_join(JoinInputs {
        tx: tx.clone(),
        player_name: player_name.clone(),
        reducer: reducer_ref,
        is_joining: is_joining.clone(),
        has_interacted: has_interacted.clone(),
        mode_id: current_mode_id.clone(),
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
        focus.is_nested(),
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
    // Spectators sit on a board like players do, so menu effects leave them alone.
    let is_on_board = is_joined || reducer.is_spectating();
    let is_queueing = reducer.queue_status.is_some();

    let lc_ref = use_mut_ref(|| *landing_cooldown);
    use_landing_cooldown_effect(
        landing_cooldown.clone(),
        lc_ref,
        current_mode_id.clone(),
        is_joined,
    );

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
        focused_slot,
        tx.clone(),
        join_step.clone(),
        reducer.clone(),
        preview_default_ref,
        is_on_board,
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

    let base_session = sessions.get(focus.base_slot()).clone();
    use_nested_queue_effect(
        focus.clone(),
        base_session.clone(),
        sessions.get(focus.base_slot().other()).clone(),
    );
    let on_quick_play = build_on_quick_play(QuickPlayInputs {
        focus: focus.clone(),
        join_step: join_step.clone(),
        rejoin_flow: rejoin_flow.clone(),
        has_interacted: has_interacted.clone(),
        base_queued: base_session.reducer.queue_status.is_some(),
    });
    let nested_queue = nested_queue(&focus, &base_session, &mode_options);

    let on_cycle_mode = {
        let mode_options = mode_options.clone();
        let base_mode_id = focus.base_mode().clone();
        Callback::from(move |delta: i32| {
            let current_index = mode_options
                .iter()
                .position(|mode| mode.id == base_mode_id)
                .unwrap_or(0);
            let next_mode = current_index
                .checked_add_signed(delta as isize)
                .and_then(|index| mode_options.get(index));
            if let Some(next_mode) = next_mode.filter(|mode| mode.id != base_mode_id) {
                navigate_to_mode(&next_mode.id);
            }
        })
    };

    let SpectateCallbacks {
        on_spectate,
        on_menu_spectate,
    } = build_spectate_callbacks(SpectateInputs {
        session: focused_session.clone(),
        player_name: player_name.clone(),
        join_step: join_step.clone(),
        rejoin_flow: rejoin_flow.clone(),
        has_interacted: has_interacted.clone(),
    });

    use_keyboard_shortcuts_effect(KeyboardShortcutEffectInputs {
        is_joined: is_on_board,
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
        slot: focused_slot,
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
        quick_play_modes: quick_play_modes(&mode_options, &global_cfg.quick_play_modes),
        mode_options: (*mode_options).clone(),
        selected_mode_id: focus.base_mode().clone(),
        on_select_mode: Callback::from(|id: ModeId| navigate_to_mode(&id)),
        on_cycle_mode,
        on_rejoin,
        rejoin_cooldown: *rejoin_cooldown,
        on_leave: build_on_leave(leave_inputs, focused_session, focus.clone()),
        on_spectate,
        on_menu_spectate,
        on_quick_play,
        nested_queue,
    })
}

/// Returns the session of `slot`.
fn slot_session(
    reducers: &PerSlot<UseReducerHandle<GameStateReducer>>,
    txs: &PerSlot<UseStateHandle<Option<MsgSender>>>,
    slot: Slot,
) -> SlotSession {
    SlotSession {
        reducer: reducers.get(slot).clone(),
        tx: (**txs.get(slot)).clone(),
    }
}

/// Describes the queue a quick-play game is nested in, for the banner over the game.
fn nested_queue(
    focus: &UseReducerHandle<ModeFocus>,
    base: &SlotSession,
    mode_options: &[ModeSummary],
) -> Option<NestedQueue> {
    let status = base
        .reducer
        .queue_status
        .clone()
        .filter(|_| focus.is_nested())?;
    let mode_name = selected_mode_name(focus.base_mode(), mode_options, base.reducer.mode.as_ref())
        .unwrap_or_else(|| focus.base_mode().to_string());
    Some(NestedQueue {
        mode_name,
        status,
        on_stop: build_on_stop_queuing(base.clone(), focus.clone()),
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
