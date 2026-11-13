//! Effects related to mode selection, navigation, and websocket connectivity.

use super::super::callbacks::{LeaveInputs, SlotSession};
use crate::app::GlobalClientConfig;
use crate::app::config::order_modes;
use crate::app::focus::{FocusAction, ModeFocus};
use crate::app::ws::{ReducerRef, SocketSenderRef, connect_ws, report_connection_lost};
use crate::reducer::{GameAction, GameStateReducer, MsgSender};
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

/// Outgoing client messages buffered before the socket picks them up.
const CLIENT_BUFFER: usize = 256;

/// Lower bound of configurable polling intervals, in milliseconds.
const MIN_INTERVAL_MS: u32 = 500;

/// Keeps the mode list refreshed on an interval.
#[hook]
pub fn use_mode_refresh_effect(
    mode_options: UseStateHandle<Vec<ModeSummary>>,
    injected_mode_info: Option<ModeSummary>,
    global_cfg: UseStateHandle<GlobalClientConfig>,
) {
    use_effect_with((), move |_| {
        let order = global_cfg.game_order.clone();
        let fetch_modes = move || {
            let modes_state = mode_options.clone();
            let injected = injected_mode_info.clone();
            let order = order.clone();
            spawn_local(async move {
                if let Ok(resp) = gloo_net::http::Request::get("/api/modes").send().await
                    && let Ok(list) = resp.json::<Vec<ModeSummary>>().await
                {
                    modes_state.set(order_modes(list, &order));
                } else if let Some(info) = injected {
                    modes_state.set(order_modes(vec![info], &order));
                }
            });
        };

        fetch_modes();
        let interval = Interval::new(
            global_cfg.modes_refresh_ms.max(MIN_INTERVAL_MS),
            fetch_modes,
        );
        || drop(interval)
    });
}

/// Syncs browser URL navigation (back/forward/hash edits) with the selected mode and landing UI.
///
/// `open_sessions` always holds the sessions of every connected slot, all of which navigation
/// leaves.
#[hook]
pub fn use_mode_url_navigation_effect(
    focus: UseReducerHandle<ModeFocus>,
    fallback_mode_id: ModeId,
    open_sessions: Rc<RefCell<Vec<SlotSession>>>,
    leave_inputs: LeaveInputs,
) {
    use_effect_with((), move |_| {
        let apply_navigation = Rc::new(move || {
            leave_inputs.leave(&open_sessions.borrow());
            focus.dispatch(FocusAction::Navigate(current_mode_id_from_location(
                &fallback_mode_id,
            )));
        });

        let window = web_sys::window().expect("window available");
        let listeners = ["hashchange", "popstate"].map(|event| {
            let apply_navigation = apply_navigation.clone();
            EventListener::new(&window, event, move |_| apply_navigation())
        });
        move || drop(listeners)
    });
}

/// Connects a slot to the websocket of its mode, reconnecting when the mode changes.
///
/// A slot without a mode stays disconnected.
#[hook]
pub fn use_ws_connection_effect(
    mode_id: Option<ModeId>,
    reducer: UseReducerHandle<GameStateReducer>,
    tx_handle: UseStateHandle<Option<MsgSender>>,
    global_cfg: UseStateHandle<GlobalClientConfig>,
) {
    // The socket outlives renders, so it dispatches through the latest reducer handle.
    let reducer_ref = use_mut_ref(|| reducer.clone());
    *reducer_ref.borrow_mut() = reducer;
    use_effect_with(mode_id, move |mode_id| {
        reducer_ref.borrow().dispatch(GameAction::Reset);
        let connection = mode_id
            .as_ref()
            .map(|mode_id| open_connection(mode_id, reducer_ref, &tx_handle, &global_cfg));
        if connection.is_none() {
            tx_handle.set(None);
        }
        move || drop(connection)
    });
}

/// A live websocket connection of one slot, closed when dropped.
struct SlotConnection {
    /// Pings the server until dropped.
    _ping_interval: Interval,
    abort_handle: AbortHandle,
    socket_tx: SocketSenderRef,
}

impl Drop for SlotConnection {
    /// Stops reconnecting and closes the open socket.
    fn drop(&mut self) {
        self.abort_handle.abort();
        // Dropping the socket's sender ends its writer, which closes the socket.
        self.socket_tx.borrow_mut().take();
    }
}

/// Opens the websocket of `mode_id`, publishing its message sender to `tx_handle`.
fn open_connection(
    mode_id: &ModeId,
    reducer_ref: ReducerRef,
    tx_handle: &UseStateHandle<Option<MsgSender>>,
    global_cfg: &GlobalClientConfig,
) -> SlotConnection {
    let (client_tx, client_rx) = mpsc::channel::<ClientMessage>(CLIENT_BUFFER);
    let sender = MsgSender(client_tx);
    tx_handle.set(Some(sender.clone()));

    let ping = move || {
        let _ = sender
            .0
            .try_send(ClientMessage::Ping(js_sys::Date::now() as u64));
    };
    ping();
    let ping_interval = Interval::new(global_cfg.ping_interval_ms.max(MIN_INTERVAL_MS), ping);

    let socket_tx: SocketSenderRef = Rc::new(RefCell::new(None));
    spawn_ws_sender_loop(client_rx, socket_tx.clone(), reducer_ref.clone());

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let url = websocket_url(mode_id);
    let connection = Abortable::new(
        connect_ws(url, mode_id.clone(), reducer_ref, socket_tx.clone()),
        abort_registration,
    );
    spawn_local(async move {
        let _ = connection.await;
    });

    SlotConnection {
        _ping_interval: ping_interval,
        abort_handle,
        socket_tx,
    }
}

/// Reads the selected mode from the URL hash.
fn current_mode_id_from_location(fallback_mode_id: &ModeId) -> ModeId {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    match hash.trim_start_matches('#') {
        "" => fallback_mode_id.clone(),
        mode => ModeId::from(mode),
    }
}

/// Forwards client messages to whichever socket is currently open.
///
/// Messages other than pings sent while no socket is open mean the player acted while
/// disconnected, so the disconnected overlay is shown.
fn spawn_ws_sender_loop(
    mut client_rx: mpsc::Receiver<ClientMessage>,
    socket_tx: SocketSenderRef,
    reducer: ReducerRef,
) {
    spawn_local(async move {
        while let Some(message) = client_rx.recv().await {
            let is_ping = matches!(message, ClientMessage::Ping(_));
            let Ok(json) = serde_json::to_string(&message) else {
                continue;
            };
            let sent = socket_tx
                .borrow()
                .as_ref()
                .is_some_and(|tx| tx.try_send(Message::Text(json)).is_ok());
            if !sent && !is_ping {
                report_connection_lost(&reducer);
            }
        }
    });
}

/// Builds the websocket URL of a mode on the current host.
fn websocket_url(mode_id: &ModeId) -> String {
    let location = web_sys::window().expect("window available").location();
    let host = location.host().unwrap_or_default();
    let protocol = match location.protocol().as_deref() {
        Ok("https:") => "wss:",
        _ => "ws:",
    };
    format!("{protocol}//{host}/api/ws/{mode_id}")
}
