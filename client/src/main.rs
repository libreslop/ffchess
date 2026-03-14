mod camera;
mod canvas;
mod components;
mod reducer;
mod utils;

pub use common::*;
use components::{
    DefeatScreen, DisconnectedScreen, ErrorToast, FatalNotification, GameView, JoinScreen,
    Leaderboard,
};
use common::protocol::{ClientMessage, GameError, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use gloo_events::EventListener;
use gloo_net::websocket::{Message, futures::WebSocket};
use gloo_timers::callback::{Interval, Timeout};
use reducer::{GameAction, GameStateReducer, MsgSender};
use std::rc::Rc;
use tokio::sync::mpsc;
use utils::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    let reducer = use_reducer(GameStateReducer::default);
    let is_joining = use_state(|| false);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(get_stored_name);
    let join_step = use_state(|| 0);
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);

    {
        let is_joining = is_joining.clone();
        let reducer_state = reducer.clone();
        use_effect_with(
            (
                reducer_state.player_id,
                reducer_state.error.clone(),
                reducer_state.disconnected,
            ),
            move |_| {
                is_joining.set(false);
            },
        );
    }

    {
        let show_disconnected = show_disconnected.clone();
        let disconnected = reducer.disconnected && !reducer.fatal_error;
        use_effect_with(disconnected, move |&disconnected| {
            if disconnected {
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

    {
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

    let landing_cooldown = use_state(|| {
        let ts = get_death_timestamp();
        let now = js_sys::Date::now() as i64;
        let cooldown_ms = 5000; // Hardcoded 5s respawn cooldown for now
        let diff_ms = cooldown_ms - (now - ts);
        if diff_ms > 0 { (diff_ms / 1000) as i32 } else { 0 }
    });
    let lc_ref = use_mut_ref(|| *landing_cooldown);

    {
        let lc = landing_cooldown.clone();
        let lc_ref = lc_ref.clone();
        use_effect_with(*lc, move |&initial_lc| {
            let mut interval = None;
            if initial_lc > 0 {
                *lc_ref.borrow_mut() = initial_lc;
                let lc_inner = lc.clone();
                let lr = lc_ref.clone();
                interval = Some(Interval::new(1000, move || {
                    let mut cur = *lr.borrow();
                    if cur > 0 {
                        cur -= 1;
                        *lr.borrow_mut() = cur;
                        lc_inner.set(cur);
                    }
                }));
            }
            || drop(interval)
        });
    }

    let reducer_ref = use_mut_ref(|| reducer.clone());
    *reducer_ref.borrow_mut() = reducer.clone();

    {
        let reducer_ref = reducer_ref.clone();
        let tx_handle = tx.clone();
        use_effect_with((), move |_| {
            let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ClientMessage>();
            let sender = MsgSender(client_tx);
            tx_handle.set(Some(sender.clone()));

            let tick_sender = sender.clone();
            let tick_reducer_ref = reducer_ref.clone();
            let interval = Interval::new(50, move || {
                let handle = tick_reducer_ref.borrow().clone();
                handle.dispatch(GameAction::Tick(tick_sender.clone()));
            });

            let ping_sender = sender.clone();
            let ping_interval = Interval::new(2000, move || {
                let now = js_sys::Date::now() as u64;
                let _ = ping_sender.0.send(ClientMessage::Ping(now));
            });

            let listener_reducer_ref = reducer_ref.clone();
            let current_ws_tx = Rc::new(std::cell::RefCell::new(
                None::<mpsc::UnboundedSender<Message>>,
            ));

            let sender_ws_tx = current_ws_tx.clone();
            let sender_reducer_ref = reducer_ref.clone();
            spawn_local(async move {
                while let Some(msg) = client_rx.recv().await {
                    let maybe_tx = sender_ws_tx.borrow().clone();
                    let current_reducer = sender_reducer_ref.borrow().clone();
                    if let Some(tx) = maybe_tx {
                        if tx
                            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                            .is_err()
                            && !current_reducer.disconnected && !current_reducer.fatal_error
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
                        && !current_reducer.disconnected && !current_reducer.fatal_error
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

            spawn_local(async move {
                let window = web_sys::window().unwrap();
                let host = window.location().host().unwrap();
                let protocol = if window.location().protocol().unwrap() == "https:" {
                    "wss:"
                } else {
                    "ws:"
                };
                
                // Get mode from URL path or default to ffa
                let pathname = window.location().pathname().unwrap();
                let mode_id = pathname.trim_start_matches('/').split('/').next().unwrap_or("ffa");
                if mode_id.is_empty() {
                    // if it was just "/"
                    let mode_id = "ffa";
                    let ws_url = format!("{}//{}/api/ws/{}", protocol, host, mode_id);
                    connect_ws(ws_url, listener_reducer_ref.clone(), current_ws_tx.clone()).await;
                } else {
                    let ws_url = format!("{}//{}/api/ws/{}", protocol, host, mode_id);
                    connect_ws(ws_url, listener_reducer_ref.clone(), current_ws_tx.clone()).await;
                }
            });
            || {
                drop(interval);
                drop(ping_interval);
            }
        });
    }

    let on_join = {
        let tx = tx.clone();
        let player_name = player_name.clone();
        let reducer_ref = reducer_ref.clone();
        let is_joining = is_joining.clone();
        let has_interacted = has_interacted.clone();
        Callback::from(move |kit_name: String| {
            let current_reducer = reducer_ref.borrow().clone();
            if *is_joining || current_reducer.disconnected || current_reducer.fatal_error {
                return;
            }
            has_interacted.set(true);
            if is_mobile() {
                request_fullscreen();
            }
            if let Some(sender) = (*tx).as_ref() {
                is_joining.set(true);
                let stored_id = get_stored_id();
                let stored_secret = get_stored_secret();
                let _ = sender.0.send(ClientMessage::Join {
                    name: (*player_name).clone(),
                    kit_name,
                    player_id: stored_id,
                    session_secret: stored_secret,
                });
            }
        })
    };

    let on_name_input = {
        let player_name = player_name.clone();
        Callback::from(move |e: InputEvent| {
            player_name.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_name_submit = {
        let join_step = join_step.clone();
        let player_name = player_name.clone();
        let landing_cooldown = landing_cooldown.clone();
        let reducer = reducer.clone();
        let has_interacted = has_interacted.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            has_interacted.set(true);
            if reducer.disconnected || *landing_cooldown > 0 {
                return;
            }
            let mut name = (*player_name).trim().to_string();
            if name.is_empty() {
                name = generate_random_name();
                player_name.set(name.clone());
            }
            set_stored_name(&name);
            join_step.set(1);
        })
    };

    let is_joined = reducer.player_id.is_some() && reducer.player_id != Some(Uuid::nil());
    let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
    let player = reducer.state.players.get(&player_id);
    let is_dead = is_joined && player.is_none();

    let rejoin_cooldown = use_state(|| 5);
    let rc_ref = use_mut_ref(|| 5);
    {
        let rejoin_cooldown = rejoin_cooldown.clone();
        let rc_ref = rc_ref.clone();
        use_effect_with(is_dead, move |is_dead| {
            let mut interval = None;
            if *is_dead {
                set_death_timestamp(js_sys::Date::now() as i64);
                let cooldown_sec = 5;
                rejoin_cooldown.set(cooldown_sec);
                *rc_ref.borrow_mut() = cooldown_sec;
                let rc = rejoin_cooldown.clone();
                let rr = rc_ref.clone();
                interval = Some(Interval::new(1000, move || {
                    let mut val = *rr.borrow();
                    if val > 0 {
                        val -= 1;
                        *rr.borrow_mut() = val;
                        rc.set(val);
                    }
                }));
            }
            || drop(interval)
        });
    }

    let on_rejoin = {
        let rc_ref = rc_ref.clone();
        let reducer = reducer.clone();
        let join_step = join_step.clone();
        let has_interacted = has_interacted.clone();
        Callback::from(move |_| {
            if *rc_ref.borrow() == 0 {
                has_interacted.set(true);
                if reducer.disconnected {
                    return;
                }
                reducer.dispatch(GameAction::SetInit {
                    player_id: Uuid::nil(),
                    session_secret: Uuid::nil(),
                    state: reducer.state.clone(),
                    mode: reducer.mode.clone().unwrap_or_else(|| panic!("No mode")),
                    pieces: reducer.piece_configs.clone(),
                    shops: reducer.shop_configs.clone(),
                });
                join_step.set(1);
            }
        })
    };

    {
        let join_step = join_step.clone();
        let player_name = player_name.clone();
        let landing_cooldown = landing_cooldown.clone();
        let reducer = reducer.clone();
        let has_interacted = has_interacted.clone();
        let on_join = on_join.clone();
        let on_rejoin = on_rejoin.clone();
        let rc_ref = rc_ref.clone();
        let is_joined = is_joined;
        let is_dead = is_dead;

        use_effect_with((is_joined, is_dead, *join_step), move |&(joined, dead, step)| {
            let listener = EventListener::new(&web_sys::window().unwrap(), "keydown", move |e| {
                let e = e.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                if e.key() == "Enter" {
                    if !joined {
                        if step == 0 && *landing_cooldown == 0 && !reducer.disconnected {
                            let mut name = (*player_name).trim().to_string();
                            if name.is_empty() {
                                name = generate_random_name();
                                player_name.set(name.clone());
                            }
                            set_stored_name(&name);
                            join_step.set(1);
                            has_interacted.set(true);
                        } else if step == 1 && !reducer.disconnected {
                            on_join.emit("Standard".to_string());
                            has_interacted.set(true);
                        }
                    } else if dead && *rc_ref.borrow() == 0 && !reducer.disconnected {
                        on_rejoin.emit(MouseEvent::new("click").unwrap());
                        has_interacted.set(true);
                    }
                }
            });
            move || drop(listener)
        });
    }

    html! {
        <div style="margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            <style>{"
                @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes simpleFadeIn { from { opacity: 0; } to { opacity: 1; } }
                @keyframes fadeInOut { 0% { opacity: 0; transform: translate(-50%, 20px); } 15% { opacity: 1; transform: translate(-50%, 0); } 85% { opacity: 1; transform: translate(-50%, 0); } 100% { opacity: 0; transform: translate(-50%, -20px); } }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
            "}</style>

            if let Some(sender) = (*tx).clone() {
                <GameView key={player_id.to_string()} reducer={reducer.clone()} tx={sender} />
            } else if !*show_disconnected || !*has_interacted {
                <div style="position: absolute; inset: 0; background: #f0f2f5; display: flex; align-items: center; justify-content: center; z-index: 200;">
                    <div style="text-align: center;">
                        <h2 style="color: #64748b;">{"Connecting to server..."}</h2>
                        <div style="width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top: 4px solid #2563eb; border-radius: 50%; margin: 20px auto; animation: spin 1s linear infinite;"></div>
                    </div>
                </div>
            }

            if *show_disconnected && reducer.disconnected && !reducer.fatal_error && (is_joined || *has_interacted) {
                <DisconnectedScreen
                    show={true}
                    disconnected={true}
                    title={reducer.disconnected_title.clone()}
                    msg={reducer.disconnected_msg.clone()}
                />
            }

            <FatalNotification
                show={reducer.fatal_error}
                title={reducer.disconnected_title.clone()}
                msg={reducer.disconnected_msg.clone()}
            />

            if is_joined {
                if is_dead {
                    <DefeatScreen score={reducer.last_score} kills={reducer.last_kills} captured={reducer.last_captured} survival_secs={reducer.last_survival_secs} on_rejoin={on_rejoin} rejoin_cooldown={*rejoin_cooldown} />
                } else {
                    <Leaderboard players={reducer.state.players.values().cloned().collect::<Vec<_>>()} self_id={player_id} />
                }
            } else if tx.is_some() {
                <JoinScreen
                    player_name={(*player_name).clone()}
                    on_name_input={on_name_input}
                    on_name_submit={on_name_submit}
                    landing_cooldown={*landing_cooldown}
                    join_step={*join_step}
                    on_join={on_join}
                    error={reducer.error.clone()}
                    is_loading={*is_joining}
                    mode={reducer.mode.clone()}
                />
            }

            if let Some(error) = &reducer.error {
                if is_joined && !is_dead {
                    <ErrorToast error={error.clone()} />
                }
            }
        </div>
    }
}

async fn connect_ws(ws_url: String, listener_reducer_ref: Rc<std::cell::RefCell<yew::UseReducerHandle<GameStateReducer>>>, current_ws_tx: Rc<std::cell::RefCell<Option<mpsc::UnboundedSender<Message>>>>) {
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
                            ServerMessage::Init { player_id, session_secret, state, mode, pieces, shops } => {
                                if player_id != Uuid::nil() {
                                    set_stored_id(player_id);
                                    set_stored_secret(session_secret);
                                }
                                GameAction::SetInit { player_id, session_secret, state, mode, pieces, shops }
                            }
                            ServerMessage::UpdateState {
                                players,
                                pieces,
                                shops,
                                removed_pieces,
                                removed_players,
                                board_size,
                            } => GameAction::UpdateState {
                                players,
                                pieces,
                                shops,
                                removed_pieces,
                                removed_players,
                                board_size,
                            },
                            ServerMessage::Error(e) => {
                                if let GameError::Custom { title, message } = &e {
                                    GameAction::SetDisconnected {
                                        disconnected: true,
                                        is_fatal: true,
                                        title: Some(title.clone()),
                                        msg: Some(message.clone()),
                                    }
                                } else {
                                    GameAction::SetError(e)
                                }
                            }
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
        }

        let current_reducer = listener_reducer_ref.borrow().clone();
        if !current_reducer.disconnected && !current_reducer.fatal_error {
            current_reducer.dispatch(GameAction::SetDisconnected {
                    disconnected: true,
                    is_fatal: false,
                    title: None,
                    msg: None,
                });
        }
        gloo_timers::future::TimeoutFuture::new(2000).await;
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
