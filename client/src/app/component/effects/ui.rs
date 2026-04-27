//! Effects related to local UI state, cooldowns, and overlays.

use crate::reducer::{ClientPhase, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{set_death_timestamp, set_stored_name};
use common::protocol::ClientMessage;
use common::types::{ModeId, PlayerId};
use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use yew::hook;
use yew::prelude::*;

/// Clears the joining spinner when join-related state updates.
#[hook]
pub fn use_joining_reset_effect(
    is_joining: UseStateHandle<bool>,
    reducer: UseReducerHandle<GameStateReducer>,
) {
    use_effect_with(
        (
            reducer.player_id,
            reducer.error.clone(),
            reducer.queue_status.clone(),
            reducer.disconnected,
        ),
        move |_| {
            is_joining.set(false);
        },
    );
}

/// Resets fatal error flags after a delay.
#[hook]
pub fn use_fatal_error_reset_effect(reducer: UseReducerHandle<GameStateReducer>) {
    let fatal_error = reducer.fatal_error;
    let reducer_handle = reducer.clone();
    use_effect_with(fatal_error, move |&fatal| {
        let mut timeout = None;
        if fatal {
            timeout = Some(Timeout::new(5000, move || {
                reducer_handle.dispatch(GameAction::SetDisconnected {
                    disconnected: false,
                    is_fatal: false,
                    title: None,
                    msg: None,
                });
            }));
        }
        move || {
            if let Some(t) = timeout {
                drop(t);
            }
        }
    });
}

/// Ticks the landing cooldown once per second while active.
#[hook]
pub fn use_landing_cooldown_effect(
    landing_cooldown: UseStateHandle<CooldownSeconds>,
    lc_ref: Rc<RefCell<CooldownSeconds>>,
) {
    let lc = landing_cooldown.clone();
    let lc_ref = lc_ref.clone();
    use_effect_with(*lc, move |&initial_lc| {
        let mut interval = None;
        if initial_lc.is_active() {
            *lc_ref.borrow_mut() = initial_lc;
            let lc_inner = lc.clone();
            let lr = lc_ref.clone();
            interval = Some(Interval::new(1000, move || {
                let mut cur = *lr.borrow();
                if cur.is_active() {
                    cur = cur.decrement();
                    *lr.borrow_mut() = cur;
                    lc_inner.set(cur);
                }
            }));
        }
        || drop(interval)
    });
}

/// Syncs the player name to the stored name when loading from server state.
#[hook]
pub fn use_player_name_sync_effect(
    player_name: UseStateHandle<String>,
    reducer: UseReducerHandle<GameStateReducer>,
) {
    let player_name = player_name.clone();
    use_effect_with(
        (
            reducer.player_id,
            reducer.state.players.clone(),
            (*player_name).clone(),
        ),
        move |(player_id, players, current_name)| {
            if current_name.trim().is_empty()
                && let Some(pid) = *player_id
                && pid != PlayerId::nil()
                && let Some(player) = players.get(&pid)
            {
                let server_name = player.name.trim();
                if !server_name.is_empty() {
                    player_name.set(server_name.to_string());
                    set_stored_name(server_name);
                }
            }
        },
    );
}

/// Animates the disconnected overlay visibility.
#[hook]
pub fn use_disconnected_overlay_effect(
    show_disconnected: UseStateHandle<bool>,
    reducer: UseReducerHandle<GameStateReducer>,
    is_joined: bool,
    is_queueing: bool,
    has_match_result: bool,
) {
    let show_disconnected = show_disconnected.clone();
    let should_show = reducer.disconnected
        && !reducer.fatal_error
        && (is_joined || is_queueing)
        && !has_match_result;
    use_effect_with(should_show, move |&should| {
        if should {
            show_disconnected.set(true);
            Box::new(|| ()) as Box<dyn FnOnce()>
        } else {
            let sd = show_disconnected.clone();
            let handle = Timeout::new(300, move || {
                sd.set(false);
            });
            Box::new(move || drop(handle)) as Box<dyn FnOnce()>
        }
    });
}

/// Keeps the rejoin flow in sync with match/queue state.
#[hook]
pub fn use_rejoin_flow_reset_effect(
    rejoin_flow: UseStateHandle<RejoinFlow>,
    reducer: UseReducerHandle<GameStateReducer>,
    has_match_result: bool,
) {
    let rejoin_flow = rejoin_flow.clone();
    use_effect_with(
        (
            reducer.phase,
            reducer.queue_status.clone(),
            has_match_result,
            *rejoin_flow,
        ),
        move |(phase, queue_status, has_match_result, flow)| {
            if flow.is_active()
                && !*has_match_result
                && *phase == ClientPhase::Alive
                && queue_status.is_none()
            {
                rejoin_flow.set(RejoinFlow::Inactive);
            }
        },
    );
}

/// Forces queue previews to the default board during kit select or queue screens.
#[hook]
pub fn use_preview_default_effect(
    tx: UseStateHandle<Option<MsgSender>>,
    join_step: UseStateHandle<JoinStep>,
    reducer: UseReducerHandle<GameStateReducer>,
    preview_default_ref: Rc<RefCell<Option<bool>>>,
    is_joined: bool,
) {
    let tx = tx.clone();
    let join_step = join_step.clone();
    let reducer = reducer.clone();
    let preview_default_ref = preview_default_ref.clone();
    let has_tx = (*tx).is_some();
    use_effect_with(
        (
            *join_step,
            reducer.queue_status.clone(),
            reducer.mode.clone(),
            has_tx,
            is_joined,
        ),
        move |(step, queue_status, mode, _has_tx, joined)| {
            if *joined {
                preview_default_ref.borrow_mut().take();
                return;
            }

            let should_force = mode
                .as_ref()
                .map(|mode| {
                    mode.queue_players.as_u32() >= 2
                        && (step.is_select_kit() || queue_status.is_some())
                })
                .unwrap_or(false);

            if let Some(sender) = tx.as_ref() {
                let mut last_sent = preview_default_ref.borrow_mut();
                if last_sent.as_ref() == Some(&should_force) {
                    return;
                }
                *last_sent = Some(should_force);
                if let Err(error) = sender.0.try_send(ClientMessage::SetPreviewDefault {
                    enabled: should_force,
                }) {
                    web_sys::console::error_1(
                        &format!("Failed to send SetPreviewDefault: {error}").into(),
                    );
                }
            }
        },
    );
}

/// Tracks cooldown when a match ends.
#[hook]
pub fn use_rejoin_cooldown_effect(
    rejoin_cooldown: UseStateHandle<CooldownSeconds>,
    rc_ref: Rc<RefCell<CooldownSeconds>>,
    current_mode_id: UseStateHandle<ModeId>,
    reducer: UseReducerHandle<GameStateReducer>,
    has_match_result: bool,
) {
    let rejoin_cooldown = rejoin_cooldown.clone();
    let rc_ref = rc_ref.clone();
    let current_mode_id = current_mode_id.clone();
    let reducer = reducer.clone();
    use_effect_with(has_match_result, move |has_match_result| {
        let mut interval = None;
        if *has_match_result {
            let cd_ms = reducer
                .mode
                .as_ref()
                .map(|m| m.respawn_cooldown_ms)
                .unwrap_or_else(|| common::types::DurationMs::from_millis(5000));
            set_death_timestamp(
                &current_mode_id,
                common::types::TimestampMs::from_millis(js_sys::Date::now() as i64),
                cd_ms,
            );
            let cooldown_sec = CooldownSeconds::from_seconds((cd_ms.as_u64() / 1000) as u32);
            rejoin_cooldown.set(cooldown_sec);
            *rc_ref.borrow_mut() = cooldown_sec;
            let rc = rejoin_cooldown.clone();
            let rr = rc_ref.clone();
            interval = Some(Interval::new(1000, move || {
                let mut val = *rr.borrow();
                if val.is_active() {
                    val = val.decrement();
                    *rr.borrow_mut() = val;
                    rc.set(val);
                }
            }));
        }
        || drop(interval)
    });
}
