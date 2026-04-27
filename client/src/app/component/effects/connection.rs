//! Effects related to mode selection, navigation, and websocket connectivity.

use crate::app::GlobalClientConfig;
use crate::app::config::order_modes;
use crate::app::ws::connect_ws;
use crate::reducer::{ClientPhase, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{JoinStep, RejoinFlow};
use common::models::ModeSummary;
use common::protocol::ClientMessage;
use common::types::ModeId;
use futures_util::future::{AbortHandle, Abortable};
use gloo_events::EventListener;
use gloo_net::websocket::Message;
use gloo_timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use wasm_bindgen_futures::spawn_local;
use yew::hook;
use yew::prelude::*;

type ReducerHandleRef = Rc<RefCell<UseReducerHandle<GameStateReducer>>>;
type SenderHandleRef = Rc<RefCell<Option<MsgSender>>>;
type WsSenderRef = Rc<RefCell<Option<mpsc::Sender<Message>>>>;

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

/// Syncs browser URL navigation (back/forward/hash edits) with the selected mode and landing UI.
#[hook]
pub fn use_mode_url_navigation_effect(
    current_mode_id: UseStateHandle<ModeId>,
    fallback_mode_id: ModeId,
    reducer_ref: ReducerHandleRef,
    join_step: UseStateHandle<JoinStep>,
    rejoin_flow: UseStateHandle<RejoinFlow>,
    tx_ref: SenderHandleRef,
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
                if should_leave_current_session(&reducer)
                    && let Some(sender) = tx_ref.borrow().as_ref()
                {
                    if let Err(error) = sender.0.try_send(ClientMessage::Leave) {
                        web_sys::console::error_1(
                            &format!("Failed to send Leave while navigating modes: {error}").into(),
                        );
                    }
                }

                rejoin_flow.set(RejoinFlow::Inactive);
                reducer.dispatch(GameAction::Reset);
                join_step.set(JoinStep::EnterName);
                current_mode_id.set(current_mode_id_from_location(&fallback_mode_id));
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
    reducer_ref: ReducerHandleRef,
    tx_handle: UseStateHandle<Option<MsgSender>>,
    global_cfg: UseStateHandle<GlobalClientConfig>,
) {
    let reducer_ref = reducer_ref.clone();
    let tx_handle = tx_handle.clone();
    let global_cfg = global_cfg.clone();
    use_effect_with((*current_mode_id).clone(), move |mode_id| {
        reducer_ref.borrow().clone().dispatch(GameAction::Reset);
        let (client_tx, client_rx) = mpsc::channel::<ClientMessage>(100);
        let sender = MsgSender(client_tx);
        tx_handle.set(Some(sender.clone()));

        let ping_sender = sender.clone();
        if let Err(error) = ping_sender
            .0
            .try_send(ClientMessage::Ping(js_sys::Date::now() as u64))
        {
            web_sys::console::error_1(&format!("Initial ping send failed: {error}").into());
        }
        let ping_interval_ms = global_cfg.ping_interval_ms.max(500);
        let ping_interval = Interval::new(ping_interval_ms, move || {
            let now = js_sys::Date::now() as u64;
            if let Err(error) = ping_sender.0.try_send(ClientMessage::Ping(now)) {
                web_sys::console::error_1(&format!("Periodic ping send failed: {error}").into());
            }
        });

        let listener_reducer_ref = reducer_ref.clone();
        let current_ws_tx = Rc::new(RefCell::new(None::<mpsc::Sender<Message>>));

        spawn_ws_sender_loop(client_rx, current_ws_tx.clone(), reducer_ref.clone());

        let (abort_handle, abort_reg) = AbortHandle::new_pair();
        spawn_ws_connection(
            mode_id.clone(),
            listener_reducer_ref,
            current_ws_tx,
            abort_reg,
        );

        move || {
            drop(ping_interval);
            abort_handle.abort();
        }
    });
}

fn should_leave_current_session(reducer: &GameStateReducer) -> bool {
    reducer.phase == ClientPhase::Alive
        || reducer.queue_status.is_some()
        || reducer.active_player_id().is_some()
}

fn current_mode_id_from_location(fallback_mode_id: &ModeId) -> ModeId {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    let hash = hash.trim_start_matches('#');
    if hash.is_empty() {
        fallback_mode_id.clone()
    } else {
        ModeId::from(hash.to_string())
    }
}

fn spawn_ws_sender_loop(
    mut client_rx: mpsc::Receiver<ClientMessage>,
    current_ws_tx: WsSenderRef,
    reducer_ref: ReducerHandleRef,
) {
    spawn_local(async move {
        while let Some(msg) = client_rx.recv().await {
            let maybe_tx = current_ws_tx.borrow().clone();
            let current_reducer = reducer_ref.borrow().clone();
            if let Some(tx) = maybe_tx {
                let send_failed = tx
                    .try_send(Message::Text(serde_json::to_string(&msg).unwrap()))
                    .is_err();
                if send_failed {
                    dispatch_soft_disconnect_if_needed(&reducer_ref, &current_reducer);
                }
            } else if !matches!(msg, ClientMessage::Ping(_)) {
                dispatch_soft_disconnect_if_needed(&reducer_ref, &current_reducer);
            }
        }
    });
}

fn dispatch_soft_disconnect_if_needed(
    reducer_ref: &ReducerHandleRef,
    current_reducer: &UseReducerHandle<GameStateReducer>,
) {
    if current_reducer.disconnected || current_reducer.fatal_error {
        return;
    }
    reducer_ref
        .borrow()
        .clone()
        .dispatch(GameAction::SetDisconnected {
            disconnected: true,
            is_fatal: false,
            title: None,
            msg: None,
        });
}

fn spawn_ws_connection(
    mode_id: ModeId,
    reducer_ref: ReducerHandleRef,
    current_ws_tx: WsSenderRef,
    abort_reg: futures_util::future::AbortRegistration,
) {
    spawn_local(async move {
        let ws_url = websocket_url(&mode_id);
        let fut = Abortable::new(
            connect_ws(ws_url, mode_id, reducer_ref.clone(), current_ws_tx.clone()),
            abort_reg,
        );
        let _ = fut.await;
    });
}

fn websocket_url(mode_id: &ModeId) -> String {
    let window = web_sys::window().unwrap();
    let host = window.location().host().unwrap();
    let protocol = if window.location().protocol().unwrap() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    format!("{protocol}//{host}/api/ws/{}", mode_id.as_ref())
}
