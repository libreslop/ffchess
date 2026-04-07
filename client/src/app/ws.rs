//! WebSocket connection loop for the client app.

use crate::reducer::{GameAction, GameStateReducer, InitPayload, UpdateStatePayload};
use crate::utils::{clear_stored_session, set_stored_id, set_stored_secret};
use common::protocol::{GameError, ServerMessage};
use common::types::{ModeId, PlayerId};
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::{Message, futures::WebSocket};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::UseReducerHandle;

/// Connects to the server WebSocket and dispatches incoming messages.
///
/// `ws_url` is the endpoint URL, `mode_id` is the selected mode,
/// `listener_reducer_ref` is the state reducer handle, and `current_ws_tx` stores a sender.
/// Returns nothing; reconnects in a loop until the app exits.
pub async fn connect_ws(
    ws_url: String,
    mode_id: ModeId,
    listener_reducer_ref: Rc<RefCell<UseReducerHandle<GameStateReducer>>>,
    current_ws_tx: Rc<RefCell<Option<mpsc::UnboundedSender<Message>>>>,
) {
    loop {
        if let Ok(ws) = WebSocket::open(&ws_url) {
            let (mut write, mut read) = ws.split();
            let (internal_tx, mut internal_rx) = mpsc::unbounded_channel::<Message>();
            *current_ws_tx.borrow_mut() = Some(internal_tx);

            spawn_local(async move {
                while let Some(m) = internal_rx.recv().await {
                    let _ = write.send(m).await;
                }
            });

            while let Some(msg) = read.next().await {
                if let Ok(Message::Text(text)) = msg
                    && let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text)
                {
                    let current_reducer = listener_reducer_ref.borrow().clone();
                    current_reducer.dispatch(match server_msg {
                        ServerMessage::Init {
                            player_id,
                            session_secret,
                            state,
                            mode,
                            pieces,
                            shops,
                        } => {
                            if player_id != PlayerId::nil() {
                                set_stored_id(&mode_id, player_id);
                                set_stored_secret(&mode_id, session_secret);
                            }
                            let state = *state;
                            GameAction::SetInit(Box::new(InitPayload {
                                player_id,
                                session_secret,
                                state,
                                mode,
                                pieces,
                                shops,
                            }))
                        }
                        ServerMessage::UpdateState {
                            players,
                            pieces,
                            shops,
                            removed_pieces,
                            removed_players,
                            board_size,
                        } => GameAction::UpdateState(Box::new(UpdateStatePayload {
                            players,
                            pieces,
                            shops,
                            removed_pieces,
                            removed_players,
                            board_size,
                        })),
                        ServerMessage::QueueState {
                            position_in_queue,
                            queued_players,
                            required_players,
                        } => GameAction::SetQueueStatus(crate::reducer::QueueStatus {
                            position_in_queue,
                            queued_players,
                            required_players,
                        }),
                        ServerMessage::Error(e) => match &e {
                            GameError::Custom { title, message: _ }
                                if title.to_lowercase().contains("invalid session secret") =>
                            {
                                clear_stored_session(&mode_id);
                                GameAction::Reset
                            }
                            GameError::Custom { title, message } => GameAction::SetDisconnected {
                                disconnected: true,
                                is_fatal: true,
                                title: Some(title.clone()),
                                msg: Some(message.clone()),
                            },
                            _ => GameAction::SetError(e),
                        },
                        ServerMessage::GameOver {
                            final_score,
                            kills,
                            pieces_captured,
                            time_survived_secs,
                        } => GameAction::GameOver {
                            final_score,
                            kills,
                            pieces_captured,
                            time_survived_secs,
                        },
                        ServerMessage::Pong(t) => GameAction::Pong(t),
                    });
                }
            }
            *current_ws_tx.borrow_mut() = None;
            let current_reducer = listener_reducer_ref.borrow().clone();
            if !current_reducer.disconnected && !current_reducer.fatal_error {
                current_reducer.dispatch(GameAction::SetDisconnected {
                    disconnected: true,
                    is_fatal: false,
                    title: None,
                    msg: None,
                });
            }
            gloo_timers::future::TimeoutFuture::new(1500).await;
        } else {
            // Avoid tight spin if socket creation fails
            let current_reducer = listener_reducer_ref.borrow().clone();
            if !current_reducer.disconnected && !current_reducer.fatal_error {
                current_reducer.dispatch(GameAction::SetDisconnected {
                    disconnected: true,
                    is_fatal: false,
                    title: None,
                    msg: None,
                });
            }
            gloo_timers::future::TimeoutFuture::new(1500).await;
        }
    }
}
