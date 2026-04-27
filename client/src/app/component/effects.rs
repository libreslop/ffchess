//! Hook helpers for the root application component.

use crate::app::GlobalClientConfig;
use crate::app::config::order_modes;
use crate::app::favicon::set_team_favicon;
use crate::app::ws::connect_ws;
use crate::reducer::{ClientPhase, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{set_death_timestamp, set_stored_name};
use common::models::{KitSummary, ModeSummary};
use common::protocol::ClientMessage;
use common::types::{KitId, ModeId, PlayerId};
use futures_util::future::{AbortHandle, Abortable};
use gloo_events::EventListener;
use gloo_net::websocket::Message;
use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::hook;
use yew::prelude::*;

const DEFAULT_FAVICON_COLOR: &str = "#dc2626";

/// Inputs for keyboard shortcut handling on the landing and end screens.
pub struct KeyboardShortcutEffectInputs {
    pub is_joined: bool,
    pub is_dead: bool,
    pub is_victory: bool,
    pub join_step: UseStateHandle<JoinStep>,
    pub landing_cooldown: UseStateHandle<CooldownSeconds>,
    pub disconnected: bool,
    pub queueing: bool,
    pub kits: Vec<KitSummary>,
    pub single_kit: Option<KitId>,
    pub player_name: UseStateHandle<String>,
    pub has_interacted: UseStateHandle<bool>,
    pub on_join: Callback<KitId>,
    pub on_rejoin: Callback<MouseEvent>,
    pub rc_ref: Rc<RefCell<CooldownSeconds>>,
}

/// Keeps the browser tab favicon synced with the local player's team color.
#[hook]
pub fn use_team_favicon_effect(team_color: Option<String>) {
    use_effect_with(team_color, move |team_color| {
        set_team_favicon(team_color.as_deref().unwrap_or(DEFAULT_FAVICON_COLOR));
        || ()
    });
}

/// Keeps the mode list refreshed on an interval.
#[hook]
pub fn use_mode_refresh_effect(
    mode_options: UseStateHandle<Vec<ModeSummary>>,
    injected_mode_info: Option<ModeSummary>,
    global_cfg: UseStateHandle<GlobalClientConfig>,
) {
    let injected_mode_info = injected_mode_info.clone();
    use_effect_with((), move |_| {
        let modes_state = mode_options.clone();
        let injected = injected_mode_info.clone();
        let order = global_cfg.game_order.clone();
        let fetch_modes = Rc::new(move || {
            let modes_state = modes_state.clone();
            let injected = injected.clone();
            let order = order.clone();
            spawn_local(async move {
                if let Ok(resp) = gloo_net::http::Request::get("/api/modes").send().await
                    && let Ok(list) = resp.json::<Vec<ModeSummary>>().await
                {
                    modes_state.set(order_modes(list, &order));
                    return;
                }
                if let Some(info) = injected.clone() {
                    modes_state.set(order_modes(vec![info], &order));
                }
            });
        });

        fetch_modes.clone()();
        let refresh_ms = global_cfg.modes_refresh_ms.max(500);
        let interval = Interval::new(refresh_ms, move || {
            fetch_modes.clone()();
        });
        || drop(interval)
    });
}

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

/// Syncs browser URL navigation (back/forward/hash edits) with the selected mode and landing UI.
#[hook]
pub fn use_mode_url_navigation_effect(
    current_mode_id: UseStateHandle<ModeId>,
    fallback_mode_id: ModeId,
    reducer_ref: Rc<RefCell<UseReducerHandle<GameStateReducer>>>,
    join_step: UseStateHandle<JoinStep>,
    rejoin_flow: UseStateHandle<RejoinFlow>,
    tx_ref: Rc<RefCell<Option<MsgSender>>>,
) {
    let current_mode_id = current_mode_id.clone();
    let reducer_ref = reducer_ref.clone();
    let join_step = join_step.clone();
    let rejoin_flow = rejoin_flow.clone();
    let tx_ref = tx_ref.clone();

    use_effect_with((), move |_| {
        let apply_navigation = Rc::new({
            let current_mode_id = current_mode_id.clone();
            let reducer_ref = reducer_ref.clone();
            let join_step = join_step.clone();
            let rejoin_flow = rejoin_flow.clone();
            let tx_ref = tx_ref.clone();
            let fallback_mode_id = fallback_mode_id.clone();
            move || {
                let reducer = reducer_ref.borrow().clone();
                let should_leave = reducer.phase == ClientPhase::Alive
                    || reducer.queue_status.is_some()
                    || reducer.active_player_id().is_some();
                if should_leave && let Some(sender) = (*tx_ref.borrow()).as_ref() {
                    let _ = sender.0.try_send(ClientMessage::Leave);
                }

                let window = web_sys::window().unwrap();
                let hash = window
                    .location()
                    .hash()
                    .unwrap_or_default()
                    .trim_start_matches('#')
                    .to_string();
                let mode_id = if hash.is_empty() {
                    fallback_mode_id.clone()
                } else {
                    ModeId::from(hash)
                };

                rejoin_flow.set(RejoinFlow::Inactive);
                reducer.dispatch(GameAction::Reset);
                join_step.set(JoinStep::EnterName);
                current_mode_id.set(mode_id);
            }
        });

        let window = web_sys::window().unwrap();
        let on_hash = apply_navigation.clone();
        let hash_listener = EventListener::new(&window, "hashchange", move |_| {
            on_hash();
        });
        let on_pop = apply_navigation.clone();
        let pop_listener = EventListener::new(&window, "popstate", move |_| {
            on_pop();
        });

        move || {
            drop(hash_listener);
            drop(pop_listener);
        }
    });
}

/// Connects to the websocket when the current mode changes.
#[hook]
pub fn use_ws_connection_effect(
    current_mode_id: UseStateHandle<ModeId>,
    reducer_ref: Rc<RefCell<UseReducerHandle<GameStateReducer>>>,
    tx_handle: UseStateHandle<Option<MsgSender>>,
    global_cfg: UseStateHandle<GlobalClientConfig>,
) {
    let reducer_ref = reducer_ref.clone();
    let tx_handle = tx_handle.clone();
    let global_cfg = global_cfg.clone();
    use_effect_with((*current_mode_id).clone(), move |mode_id| {
        reducer_ref.borrow().clone().dispatch(GameAction::Reset);
        let (client_tx, mut client_rx) = mpsc::channel::<ClientMessage>(100);
        let sender = MsgSender(client_tx);
        tx_handle.set(Some(sender.clone()));

        let ping_sender = sender.clone();
        let _ = ping_sender
            .0
            .try_send(ClientMessage::Ping(js_sys::Date::now() as u64));
        let ping_interval_ms = global_cfg.ping_interval_ms.max(500);
        let ping_interval = Interval::new(ping_interval_ms, move || {
            let now = js_sys::Date::now() as u64;
            let _ = ping_sender.0.try_send(ClientMessage::Ping(now));
        });

        let listener_reducer_ref = reducer_ref.clone();
        let current_ws_tx = Rc::new(std::cell::RefCell::new(None::<mpsc::Sender<Message>>));

        let sender_ws_tx = current_ws_tx.clone();
        let sender_reducer_ref = reducer_ref.clone();
        spawn_local(async move {
            while let Some(msg) = client_rx.recv().await {
                let maybe_tx = sender_ws_tx.borrow().clone();
                let current_reducer = sender_reducer_ref.borrow().clone();
                if let Some(tx) = maybe_tx {
                    if tx
                        .try_send(Message::Text(serde_json::to_string(&msg).unwrap()))
                        .is_err()
                        && !current_reducer.disconnected
                        && !current_reducer.fatal_error
                    {
                        sender_reducer_ref
                            .borrow()
                            .clone()
                            .dispatch(GameAction::SetDisconnected {
                                disconnected: true,
                                is_fatal: false,
                                title: None,
                                msg: None,
                            });
                    }
                } else if !matches!(msg, ClientMessage::Ping(_))
                    && !current_reducer.disconnected
                    && !current_reducer.fatal_error
                {
                    sender_reducer_ref
                        .borrow()
                        .clone()
                        .dispatch(GameAction::SetDisconnected {
                            disconnected: true,
                            is_fatal: false,
                            title: None,
                            msg: None,
                        });
                }
            }
        });

        let (abort_handle, abort_reg) = AbortHandle::new_pair();
        {
            let listener_reducer_ref = listener_reducer_ref.clone();
            let current_ws_tx = current_ws_tx.clone();
            let mode_id = mode_id.clone();
            spawn_local(async move {
                let window = web_sys::window().unwrap();
                let host = window.location().host().unwrap();
                let protocol = if window.location().protocol().unwrap() == "https:" {
                    "wss:"
                } else {
                    "ws:"
                };

                let ws_url = format!("{}//{}/api/ws/{}", protocol, host, mode_id.as_ref());
                let fut = Abortable::new(
                    connect_ws(
                        ws_url,
                        mode_id.clone(),
                        listener_reducer_ref.clone(),
                        current_ws_tx.clone(),
                    ),
                    abort_reg,
                );
                let _ = fut.await;
            });
        }
        move || {
            drop(ping_interval);
            abort_handle.abort();
        }
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
            let is_queue_mode = mode
                .as_ref()
                .map(|m| m.queue_players.as_u32() >= 2)
                .unwrap_or(false);
            if *joined {
                preview_default_ref.borrow_mut().take();
                return;
            }

            let should_force = is_queue_mode && (step.is_select_kit() || queue_status.is_some());

            if let Some(sender) = (*tx).as_ref() {
                let mut last_sent = preview_default_ref.borrow_mut();
                if last_sent.as_ref() == Some(&should_force) {
                    return;
                }
                *last_sent = Some(should_force);
                let _ = sender.0.try_send(ClientMessage::SetPreviewDefault {
                    enabled: should_force,
                });
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

/// Handles keyboard shortcuts for joining and rejoining games.
#[hook]
pub fn use_keyboard_shortcuts_effect(inputs: KeyboardShortcutEffectInputs) {
    let KeyboardShortcutEffectInputs {
        is_joined,
        is_dead,
        is_victory,
        join_step,
        landing_cooldown,
        disconnected,
        queueing,
        kits,
        single_kit,
        player_name,
        has_interacted,
        on_join,
        on_rejoin,
        rc_ref,
    } = inputs;
    let join_step = join_step.clone();
    let player_name = player_name.clone();
    let landing_cooldown = landing_cooldown.clone();
    let has_interacted = has_interacted.clone();
    let on_join = on_join.clone();
    let on_rejoin = on_rejoin.clone();
    let rc_ref = rc_ref.clone();

    use_effect_with(
        (
            is_joined,
            is_dead,
            is_victory,
            *join_step,
            *landing_cooldown,
            disconnected,
            queueing,
            kits.clone(),
            single_kit.clone(),
        ),
        move |&(joined, dead, victory, step, lc, disc, queueing, ref kits, ref single_kit)| {
            let on_join = on_join.clone();
            let on_rejoin = on_rejoin.clone();
            let rc_ref = rc_ref.clone();
            let kits = kits.clone();
            let single_kit = single_kit.clone();

            let listener = EventListener::new(&web_sys::window().unwrap(), "keydown", move |e| {
                let e = e.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                let key = e.key();
                if key == "Enter" {
                    if !joined && !dead && !victory {
                        if step.is_enter_name() && lc.is_zero() && !disc {
                            let name = (*player_name).trim().to_string();
                            set_stored_name(&name);
                            if let Some(kit_id) = single_kit.clone() {
                                join_step.set(JoinStep::SelectKit);
                                on_join.emit(kit_id);
                            } else {
                                join_step.set(JoinStep::SelectKit);
                            }
                            has_interacted.set(true);
                        }
                    } else if (dead || victory) && rc_ref.borrow().is_zero() && !disc {
                        on_rejoin.emit(MouseEvent::new("click").unwrap());
                        has_interacted.set(true);
                    }
                } else if !joined
                    && !dead
                    && !victory
                    && step.is_select_kit()
                    && !queueing
                    && !disc
                    && let Ok(num) = key.parse::<usize>()
                    && num > 0
                    && num <= kits.len()
                    && let Some(kit) = kits.get(num - 1)
                {
                    on_join.emit(kit.name.clone());
                    has_interacted.set(true);
                }
            });
            move || drop(listener)
        },
    );
}
