//! Effects related to local UI state, cooldowns, and overlays.

use crate::app::focus::Slot;
use crate::reducer::{ClientPhase, DisconnectNotice, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{DeathRecord, SessionStore, local_now, store_name};
use common::protocol::ClientMessage;
use common::types::ModeId;
use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use yew::hook;
use yew::prelude::*;

/// How long an error notice stays visible, matching the toast animation.
const ERROR_VISIBLE_MS: u32 = 3000;

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
        move |_| is_joining.set(false),
    );
}

/// Returns to name entry when kit selection is left showing for a mode with a single kit.
///
/// Single-kit modes skip kit selection by joining right away, but the step stays on
/// kit selection afterwards. Once the player is back on the menu without a join in flight
/// (a failed join, or a reconnect after the server dropped the session), that leftover step
/// would show a kit picker with one choice.
#[hook]
pub fn use_single_kit_step_reset_effect(
    join_step: UseStateHandle<JoinStep>,
    reducer: UseReducerHandle<GameStateReducer>,
    is_joining: bool,
    single_kit: bool,
    is_nested: bool,
) {
    let idle_on_menu = reducer.phase == ClientPhase::Menu && !is_joining;
    // A quick-play game has no name entry of its own to return to.
    let stale = single_kit && idle_on_menu && join_step.is_select_kit() && !is_nested;
    use_effect_with(stale, move |&stale| {
        if stale {
            join_step.set(JoinStep::EnterName);
        }
    });
}

/// Hides each error notice once its toast animation has finished.
#[hook]
pub fn use_error_auto_clear_effect(reducer: UseReducerHandle<GameStateReducer>) {
    let serial = reducer.error.as_ref().map(|notice| notice.serial);
    use_effect_with(serial, move |serial| {
        let timeout = serial.map(|serial| {
            Timeout::new(ERROR_VISIBLE_MS, move || {
                reducer.dispatch(GameAction::ClearError(serial));
            })
        });
        move || drop(timeout)
    });
}

/// Resets fatal error flags after a delay.
#[hook]
pub fn use_fatal_error_reset_effect(
    reducer: UseReducerHandle<GameStateReducer>,
    hide_after_ms: u32,
) {
    let fatal_error = reducer.fatal_error;
    use_effect_with(fatal_error, move |&fatal| {
        let timeout = fatal.then(|| {
            Timeout::new(hide_after_ms, move || {
                reducer.dispatch(GameAction::SetDisconnected(DisconnectNotice::none()));
            })
        });
        move || drop(timeout)
    });
}

/// Starts counting `cooldown` down from `start` once per second.
///
/// `mirror` holds the same value for callbacks that read it outside of renders. Returns the
/// interval driving the countdown, or `None` when there is nothing to count.
fn start_countdown(
    cooldown: UseStateHandle<CooldownSeconds>,
    mirror: Rc<RefCell<CooldownSeconds>>,
    start: CooldownSeconds,
) -> Option<Interval> {
    *mirror.borrow_mut() = start;
    cooldown.set(start);
    start.is_active().then(|| {
        Interval::new(1000, move || {
            let current = *mirror.borrow();
            if current.is_active() {
                let next = current.decrement();
                *mirror.borrow_mut() = next;
                cooldown.set(next);
            }
        })
    })
}

/// Counts the landing cooldown down from the last recorded death of the selected mode.
///
/// Restarts whenever the landing screen comes back, e.g. after leaving a match.
#[hook]
pub fn use_landing_cooldown_effect(
    landing_cooldown: UseStateHandle<CooldownSeconds>,
    lc_ref: Rc<RefCell<CooldownSeconds>>,
    current_mode_id: ModeId,
    is_joined: bool,
) {
    use_effect_with((current_mode_id, is_joined), move |(mode_id, is_joined)| {
        let interval = (!is_joined)
            .then(|| {
                let remaining = SessionStore::new(mode_id).death().remaining(local_now());
                start_countdown(
                    landing_cooldown,
                    lc_ref,
                    CooldownSeconds::from_duration(remaining),
                )
            })
            .flatten();
        move || drop(interval)
    });
}

/// Syncs the player name to the server-chosen name when none was entered.
#[hook]
pub fn use_player_name_sync_effect(
    player_name: UseStateHandle<String>,
    reducer: UseReducerHandle<GameStateReducer>,
) {
    let server_name = reducer
        .active_player_id()
        .and_then(|player_id| reducer.state.players.get(&player_id))
        .map(|player| player.name.trim().to_string());
    use_effect_with(
        (server_name, (*player_name).clone()),
        move |(server_name, current_name)| {
            if current_name.trim().is_empty()
                && let Some(server_name) = server_name.as_ref().filter(|name| !name.is_empty())
            {
                player_name.set(server_name.clone());
                store_name(server_name);
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
    hide_after_ms: u32,
) {
    let should_show = reducer.disconnected
        && !reducer.fatal_error
        && (is_joined || is_queueing)
        && !reducer.has_match_result();
    use_effect_with(should_show, move |&should_show| {
        let timeout = if should_show {
            show_disconnected.set(true);
            None
        } else {
            Some(Timeout::new(hide_after_ms, move || {
                show_disconnected.set(false)
            }))
        };
        move || drop(timeout)
    });
}

/// Ends the rejoin flow once the player is back in a match.
#[hook]
pub fn use_rejoin_flow_reset_effect(
    rejoin_flow: UseStateHandle<RejoinFlow>,
    reducer: UseReducerHandle<GameStateReducer>,
) {
    use_effect_with(
        (
            reducer.phase,
            reducer.queue_status.clone(),
            reducer.has_match_result(),
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

/// Last preview pinning sent, and the connection slot it was sent on.
pub type PreviewDefaultRef = Rc<RefCell<Option<(Slot, bool)>>>;

/// Forces queue previews to the default board during kit select or queue screens.
///
/// `slot` is the connection of the focused session, which `tx` and `reducer` belong to.
#[hook]
pub fn use_preview_default_effect(
    slot: Slot,
    tx: UseStateHandle<Option<MsgSender>>,
    join_step: UseStateHandle<JoinStep>,
    reducer: UseReducerHandle<GameStateReducer>,
    preview_default_ref: PreviewDefaultRef,
    is_joined: bool,
) {
    let has_tx = tx.is_some();
    use_effect_with(
        (
            slot,
            *join_step,
            reducer.queue_status.clone(),
            reducer.mode.clone(),
            has_tx,
            is_joined,
        ),
        move |(slot, step, queue_status, mode, _, joined)| {
            if *joined {
                preview_default_ref.borrow_mut().take();
                return;
            }
            let should_force = mode.as_ref().is_some_and(|mode| {
                mode.queue_players.as_u32() >= 2 && (step.is_select_kit() || queue_status.is_some())
            });
            let Some(sender) = tx.as_ref() else {
                return;
            };
            let mut last_sent = preview_default_ref.borrow_mut();
            let next = Some((*slot, should_force));
            if *last_sent != next
                && sender.send(ClientMessage::SetPreviewDefault {
                    enabled: should_force,
                })
            {
                *last_sent = next;
            }
        },
    );
}

/// Starts the rejoin cooldown when a match ends and remembers the death for page reloads.
#[hook]
pub fn use_rejoin_cooldown_effect(
    rejoin_cooldown: UseStateHandle<CooldownSeconds>,
    rc_ref: Rc<RefCell<CooldownSeconds>>,
    current_mode_id: ModeId,
    reducer: UseReducerHandle<GameStateReducer>,
) {
    use_effect_with(reducer.has_match_result(), move |&has_match_result| {
        let interval = has_match_result
            .then(|| {
                let cooldown = reducer.respawn_cooldown();
                SessionStore::new(&current_mode_id).record_death(DeathRecord {
                    died_at: local_now(),
                    cooldown,
                });
                start_countdown(
                    rejoin_cooldown,
                    rc_ref,
                    CooldownSeconds::from_duration(cooldown),
                )
            })
            .flatten();
        move || drop(interval)
    });
}
