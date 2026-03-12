mod reducer;
mod canvas;

use wasm_bindgen::JsCast;
use yew::prelude::*;
pub use common::*;
use reducer::{GameStateReducer, GameAction, Pmove, MsgSender};
use canvas::Renderer;
use gloo_net::websocket::{futures::WebSocket, Message};
use futures_util::{StreamExt, SinkExt};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlCanvasElement;
use tokio::sync::mpsc;
use std::rc::Rc;
use glam::IVec2;
use uuid::Uuid;
use gloo_events::EventListener;
use gloo_timers::callback::{Interval, Timeout};
use rand::seq::SliceRandom;

fn get_stored_name() -> String {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage.get_item("ffchess_name").unwrap_or_default().unwrap_or_default();
    }
    String::new()
}

fn set_stored_name(name: &str) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_name", name);
    }
}

fn get_death_timestamp() -> i64 {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        return storage.get_item("ffchess_death_ts").unwrap_or_default()
            .and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    }
    0
}

fn set_death_timestamp(ts: i64) {
    let window = web_sys::window().unwrap();
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("ffchess_death_ts", &ts.to_string());
    }
}

fn generate_random_name() -> String {
    let adjectives = [
        "Swift", "Brave", "Silent", "Iron", "Gold", "Shadow", "Grand", "Quick", "Old", "New", "Wild", "Calm",
        "Crimson", "Azure", "Sly", "Mighty", "Ancient", "Fierce", "Noble", "Ethereal", "Frosty", "Fiery", 
        "Stormy", "Golden", "Silver", "Hidden", "Lone", "vibrant", "Dark", "Bright", "Steady", "Fallen"
    ];
    let nouns = [
        "Knight", "King", "Rook", "Bishop", "Pawn", "Queen", "Warrior", "Shadow", "Storm", "Frost", "Flame", "Blade",
        "Guard", "Seeker", "Warden", "Herald", "Slayer", "Spirit", "Ghost", "Titan", "Wolf", "Raven", "Dragon",
        "Phoenix", "Sentinel", "Oracle", "Monarch", "Paladin", "Ranger", "Saber", "Shield", "Fang"
    ];
    let mut rng = rand::thread_rng();
    let adj = adjectives.choose(&mut rng).unwrap();
    // Ensure the noun is different from the adjective
    let mut noun = nouns.choose(&mut rng).unwrap();
    while noun == adj {
        noun = nouns.choose(&mut rng).unwrap();
    }
    format!("{} {}", adj, noun)
}

#[function_component(App)]
pub fn app() -> Html {
    let reducer = use_reducer(GameStateReducer::default);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(|| get_stored_name());
    let join_step = use_state(|| 0); // Always start at Name step on refresh
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);

    {
        let show_disconnected = show_disconnected.clone();
        let disconnected = reducer.disconnected;
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
    
    // Landing Cooldown (persists through refresh)
    let landing_cooldown = use_state(|| {
        let ts = get_death_timestamp();
        let now = js_sys::Date::now() as i64 / 1000;
        let diff = 5 - (now - ts);
        if diff > 0 { diff as i32 } else { 0 }
    });
    let lc_ref = use_mut_ref(|| *landing_cooldown);

    {
        let lc = landing_cooldown.clone();
        let lc_ref = lc_ref.clone();
        use_effect_with((*lc).clone(), move |&initial_lc| {
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

    // Stable reference to the latest reducer for the Tick loop
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

            let listener_reducer_ref = reducer_ref.clone();
            let current_ws_tx = Rc::new(std::cell::RefCell::new(None::<mpsc::UnboundedSender<Message>>));
            
            // Outgoing Message Loop
            let sender_ws_tx = current_ws_tx.clone();
            let sender_reducer_ref = reducer_ref.clone();
            spawn_local(async move {
                while let Some(msg) = client_rx.recv().await {
                    let maybe_tx = sender_ws_tx.borrow().clone();
                    if let Some(tx) = maybe_tx {
                        if let Err(_) = tx.send(Message::Text(serde_json::to_string(&msg).unwrap())) {
                            if !sender_reducer_ref.borrow().disconnected {
                                sender_reducer_ref.borrow().clone().dispatch(GameAction::SetDisconnected(true));
                            }
                        }
                    } else if !matches!(msg, ClientMessage::Ping(_)) {
                        // If we try to send a non-ping message and we are not connected, ensure the UI knows it
                        if !sender_reducer_ref.borrow().disconnected {
                            sender_reducer_ref.borrow().clone().dispatch(GameAction::SetDisconnected(true));
                        }
                    }
                }
            });

            // Reconnection & Listener Loop
            spawn_local(async move {
                let window = web_sys::window().unwrap();
                let host = window.location().host().unwrap();
                let protocol = if window.location().protocol().unwrap() == "https:" { "wss:" } else { "ws:" };
                let ws_url = format!("{}//{}/api/ws", protocol, host);
                
                loop {
                    if let Ok(ws) = WebSocket::open(&ws_url) {
                        let (mut write, mut read) = ws.split();
                        let (internal_tx, mut internal_rx) = mpsc::unbounded_channel::<Message>();
                        *current_ws_tx.borrow_mut() = Some(internal_tx);
                        
                        // Bridge outgoing
                        spawn_local(async move {
                            while let Some(m) = internal_rx.recv().await {
                                let _ = write.send(m).await;
                            }
                        });

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg
                                && let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                                listener_reducer_ref.borrow().clone().dispatch(match server_msg {
                                    ServerMessage::Init { player_id, state } => GameAction::SetInit { player_id, state },
                                    ServerMessage::UpdateState { players, pieces, shops, removed_pieces, removed_players, board_size } => GameAction::UpdateState { players, pieces, shops, removed_pieces, removed_players, board_size },
                                    ServerMessage::Error(e) => GameAction::SetError(e),
                                    ServerMessage::GameOver { final_score, kills, pieces_captured, time_survived_secs } => GameAction::GameOver { final_score, kills, pieces_captured, time_survived_secs },
                                    ServerMessage::Pong(t) => GameAction::Pong(t),
                                });
                            }
                        }
                        *current_ws_tx.borrow_mut() = None;
                    }
                    
                    if !listener_reducer_ref.borrow().disconnected {
                        listener_reducer_ref.borrow().clone().dispatch(GameAction::SetDisconnected(true));
                    }
                    gloo_timers::future::TimeoutFuture::new(2000).await;
                }
            });
            || drop(interval)
        });
    }

    let on_join = {
        let tx = tx.clone();
        let player_name = player_name.clone();
        let reducer = reducer.clone();
        let has_interacted = has_interacted.clone();
        Callback::from(move |kit: KitType| {
            has_interacted.set(true);
            if reducer.disconnected {
                return;
            }
            if let Some(sender) = (*tx).as_ref() {
                let _ = sender.0.send(ClientMessage::Join { name: (*player_name).clone(), kit, player_id: None });
            }
        })
    };

    let on_name_input = {
        let player_name = player_name.clone();
        Callback::from(move |e: InputEvent| {
            player_name.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
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
            if reducer.disconnected {
                return;
            }
            if *landing_cooldown > 0 { return; }
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
    
    // Game Over Cooldown
    let rejoin_cooldown = use_state(|| 5);
    let rc_ref = use_mut_ref(|| 5);
    {
        let rejoin_cooldown = rejoin_cooldown.clone();
        let rc_ref = rc_ref.clone();
        use_effect_with(is_dead, move |is_dead| {
            let mut interval = None;
            if *is_dead {
                set_death_timestamp(js_sys::Date::now() as i64 / 1000);
                rejoin_cooldown.set(5);
                *rc_ref.borrow_mut() = 5;
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
                reducer.dispatch(GameAction::SetInit { player_id: Uuid::nil(), state: reducer.state.clone() });
                join_step.set(1);
            }
        })
    };

    // Global keyboard listener for Enter (rejoin / step transition)
    {
        let is_dead = is_dead;
        let rc_ref = rc_ref.clone();
        let join_step = join_step.clone();
        let on_rejoin = on_rejoin.clone();
        let reducer = reducer.clone();
        use_effect_with((is_dead, rc_ref, join_step), move |(is_dead, rc_ref, join_step)| {
            let is_dead = *is_dead;
            let rc_ref = rc_ref.clone();
            let join_step = join_step.clone();
            let on_rejoin = on_rejoin.clone();
            let reducer = reducer.clone();
            let listener = EventListener::new(&web_sys::window().unwrap(), "keydown", move |e| {
                let e = e.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                if e.key() == "Enter" && !reducer.disconnected {
                    if is_dead && *rc_ref.borrow() == 0 {
                        on_rejoin.emit(MouseEvent::new("click").unwrap());
                    } else if !is_dead && *join_step == 0 {
                        // Let the form submit handle it to keep validation logic in one place
                    }
                }
            });
            || drop(listener)
        });
    }

    let leaderboard_items = if is_joined && !is_dead {
        let mut players: Vec<_> = reducer.state.players.values().collect();
        players.sort_by(|a, b| b.score.cmp(&a.score));
        players.into_iter().take(10).map(|p| {
            let is_self = player_id == p.id;
            let display_name = if p.name.trim().is_empty() { "An Unnamed Player" } else { &p.name };
            html! {
                <div style={format!("display: flex; justify-content: space-between; font-size: 0.9em; {}", if is_self { "font-weight: bold; color: #2563eb;" } else { "" })}>
                    <span style="max-width: 130px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{display_name}</span>
                    <span>{p.score}</span>
                </div>
            }
        }).collect::<Html>()
    } else { html! {} };

    html! {
        <div style="margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            <style>{"
                @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes simpleFadeIn { from { opacity: 0; } to { opacity: 1; } }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
            "}</style>
            {
                if let Some(sender) = (*tx).clone() {
                    html! { <GameView key={player_id.to_string()} reducer={reducer.clone()} tx={sender} /> }
                } else if *show_disconnected && *has_interacted {
                    html! {} // Let the disconnected screen handle it
                } else {
                    html! {
                        <div style="position: absolute; inset: 0; background: #f0f2f5; display: flex; align-items: center; justify-content: center; z-index: 200;">
                            <div style="text-align: center;">
                                <h2 style="color: #64748b;">{"Connecting to server..."}</h2>
                                <div style="width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top: 4px solid #2563eb; border-radius: 50%; margin: 20px auto; animation: spin 1s linear infinite;"></div>
                            </div>
                        </div>
                    }
                }
            }

            if *show_disconnected && (is_joined || *has_interacted) {
                <div style={format!("position: absolute; inset: 0; background: #ef4444; z-index: 300; display: flex; align-items: center; justify-content: center; transition: opacity 0.3s ease-out; animation: simpleFadeIn 0.3s ease-out; opacity: {}; pointer-events: {};", 
                    if reducer.disconnected { "1" } else { "0" },
                    if reducer.disconnected { "all" } else { "none" }
                )}>
                    <div style="text-align: center; color: #fff; padding: 20px;">
                        <h1 style="color: #fff; margin: 0; font-size: 4em; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);">{"DISCONNECTED"}</h1>
                        <p style="margin: 20px 0 0; font-size: 1.2em; color: #fff; letter-spacing: 1px;">{"The connection to the server was lost."}</p>
                    </div>
                </div>
            }

            if is_joined {
                if is_dead {
                    <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90; animation: simpleFadeIn 0.3s ease-out;"></div>
                    <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 400px; color: #fff; animation: fadeIn 0.3s ease-out;">
                        <h1 style="color: #ef4444; margin-top: 0; font-size: 4em; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);">{"DEFEAT"}</h1>
                        
                        <div style="margin: 30px 0; display: flex; flex-direction: column; gap: 15px;">
                            <div style="padding: 15px;">
                                <span style="display: block; font-size: 0.9em; text-transform: uppercase; color: #cbd5e1; margin-bottom: 5px; letter-spacing: 1px;">{"Final Score"}</span>
                                <span style="font-size: 3em; font-weight: 900; color: #fff;">{reducer.last_score}</span>
                            </div>
                            
                            <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px;">
                                <div style="padding: 10px;">
                                    <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Kills"}</span>
                                    <span style="font-size: 1.8em; font-weight: bold;">{reducer.last_kills}</span>
                                </div>
                                <div style="padding: 10px;">
                                    <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Taken"}</span>
                                    <span style="font-size: 1.8em; font-weight: bold;">{reducer.last_captured}</span>
                                </div>
                                <div style="padding: 10px;">
                                    <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Survived"}</span>
                                    <span style="font-size: 1.8em; font-weight: bold;">{format!("{}m {}s", reducer.last_survival_secs / 60, reducer.last_survival_secs % 60)}</span>
                                </div>
                            </div>
                        </div>

                        <button onclick={on_rejoin} disabled={*rejoin_cooldown > 0}
                            style={format!("padding: 15px 40px; font-size: 1.2em; cursor: {}; background: {}; color: white; border: 3px solid {}; border-radius: 0; font-weight: 900; width: auto; transition: all 0.2s; text-transform: uppercase; letter-spacing: 2px;", 
                                if *rejoin_cooldown > 0 { "not-allowed" } else { "pointer" },
                                if *rejoin_cooldown > 0 { "rgba(148, 163, 184, 0.2)" } else { "rgba(30, 41, 59, 0.4)" },
                                if *rejoin_cooldown > 0 { "#94a3b8" } else { "#fff" })}>
                            if *rejoin_cooldown > 0 {
                                {format!("Wait ({}s)", *rejoin_cooldown)}
                            } else {
                                {"PLAY AGAIN"}
                            }
                        </button>
                        <p style="margin-top: 25px; font-size: 0.9em; color: #cbd5e1; letter-spacing: 1px;">{"Tip: Press ENTER to play again"}</p>
                    </div>
                } else {
                    <div style="position: absolute; top: 20px; right: 20px; background: transparent; padding: 15px; width: 200px; z-index: 60; pointer-events: none;">
                        <div style="display: flex; flex-direction: column; gap: 5px;">
                            {leaderboard_items}
                        </div>
                    </div>
                }
            } else if tx.is_some() {
                <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
                <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 400px; padding: 30px;">
                    <h1 style="margin-top: 0; color: #fff; font-size: 4em; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);">{"FFCHESS"}</h1>
                    
                    if *join_step == 0 {
                        <form onsubmit={on_name_submit}>
                            <div style="display: flex; flex-direction: column; gap: 15px; align-items: center;">
                                <input type="text" name="player_name" value={(*player_name).clone()} oninput={on_name_input} placeholder="This is a tale of..." autofocus=true
                                    style="padding: 12px 20px; border-radius: 0; border: 2px solid #cbd5e1; width: 100%; box-sizing: border-box; font-size: 1.2em; outline: none; background: #fff; text-align: center;"/>
                                <button type="submit" disabled={*landing_cooldown > 0}
                                    style={format!("padding: 10px 40px; background: {}; color: #fff; border: 3px solid {}; border-radius: 0; font-weight: 900; cursor: {}; font-size: 1.2em; width: auto; text-transform: uppercase; letter-spacing: 1px;", 
                                        if *landing_cooldown > 0 { "#94a3b8" } else { "#3b82f6" },
                                        if *landing_cooldown > 0 { "#64748b" } else { "#1e3a8a" },
                                        if *landing_cooldown > 0 { "not-allowed" } else { "pointer" })}>
                                    if *landing_cooldown > 0 {
                                        {format!("Wait ({}s)", *landing_cooldown)}
                                    } else {
                                        {"Play!"}
                                    }
                                </button>
                            </div>
                        </form>
                    } else {
                        <div style="animation: fadeIn 0.3s ease-out; display: flex; flex-direction: column; align-items: center;">
                            <h3 style="color: #fff; margin-bottom: 25px; text-transform: uppercase; letter-spacing: 2px; text-shadow: 0 2px 4px rgba(0,0,0,0.3);">{"CHOOSE YOUR ARMY"}</h3>
                            <div style="display: grid; grid-template-columns: 1fr; gap: 12px; width: 100%;">
                                <button onclick={on_join.reform(|_| KitType::Standard)} style="padding: 15px; cursor: pointer; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s;">
                                    {"STANDARD"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"2 Pawns, 2 Knights"}</span>
                                </button>
                                <button onclick={on_join.reform(|_| KitType::Shield)} style="padding: 15px; cursor: pointer; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s;">
                                    {"SHIELD"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"6 Pawns"}</span>
                                </button>
                                <button onclick={on_join.reform(|_| KitType::Scout)} style="padding: 15px; cursor: pointer; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s;">
                                    {"SCOUT"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"1 Pawn, 2 Bishops"}</span>
                                </button>
                                <button onclick={on_join.reform(|_| KitType::Tank)} style="padding: 15px; cursor: pointer; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s;">
                                    {"TANK"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"1 Rook"}</span>
                                </button>
                            </div>
                        </div>
                    }
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
}

#[function_component(GameView)]
fn game_view(props: &GameViewProps) -> Html {
    let canvas_ref = use_node_ref();
    let selected_piece_id = use_state(|| None::<Uuid>);
    
    // Using Refs for interpolation to avoid stale closures and redundant renders
    let camera_ref = use_mut_ref(|| (0.0, 0.0));
    let target_camera_ref = use_mut_ref(|| (0.0, 0.0));
    let zoom_ref = use_mut_ref(|| 1.0f64);
    let target_zoom_ref = use_mut_ref(|| 1.0f64);
    let mouse_ref = use_mut_ref(|| (0.0, 0.0));
    
    // Stable ref to state for the loop
    let state_ref = use_mut_ref(|| (props.reducer.state.clone(), props.reducer.player_id));
    *state_ref.borrow_mut() = (props.reducer.state.clone(), props.reducer.player_id);

    // States for rendering triggering
    let zoom_state = use_state(|| 1.0f64);
    let cam_state = use_state(|| (0.0, 0.0));
    let frame_id = use_state(|| 0u64);
    let drag_start = use_state(|| None::<(f64, f64, bool)>);
    let velocity_ref = use_mut_ref(|| (0.0f64, 0.0f64));

    // Track death state transition
    let was_alive_ref = use_mut_ref(|| false);
    let last_king_grid_pos_ref = use_mut_ref(|| IVec2::ZERO);

    {
        let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
        let player = props.reducer.state.players.get(&player_id);
        let is_alive = player.is_some() && player_id != Uuid::nil();
        let mut was_alive = was_alive_ref.borrow_mut();
        
        if is_alive {
            if let Some(p) = player {
                if let Some(king) = props.reducer.state.pieces.get(&p.king_id) {
                    *last_king_grid_pos_ref.borrow_mut() = king.position;
                }
            }
            *was_alive = true;
        } else if *was_alive {
            // Just died
            let grid_pos = *last_king_grid_pos_ref.borrow();
            let target_zoom = 1.3;
            let tile_size = 40.0 * target_zoom;
            let pixel_pos = (
                grid_pos.x as f64 * tile_size + tile_size / 2.0,
                grid_pos.y as f64 * tile_size + tile_size / 2.0
            );
            
            *target_camera_ref.borrow_mut() = pixel_pos;
            *target_zoom_ref.borrow_mut() = target_zoom;
            *was_alive = false;
        } else if player_id == Uuid::nil() && !*was_alive {
            // On kit selection screen (reset camera)
            if *target_camera_ref.borrow() != (0.0, 0.0) {
                *target_camera_ref.borrow_mut() = (0.0, 0.0);
                *target_zoom_ref.borrow_mut() = 1.0;
            }
        }
    }
    
    let window_size = use_state(|| (
        web_sys::window().unwrap().inner_width().unwrap().as_f64().unwrap(),
        web_sys::window().unwrap().inner_height().unwrap().as_f64().unwrap()
    ));

    // Smooth Interpolation Loop
    {
        let zoom_state = zoom_state.clone();
        let cam_state = cam_state.clone();
        let zoom_ref = zoom_ref.clone();
        let target_zoom_ref = target_zoom_ref.clone();
        let camera_ref = camera_ref.clone();
        let mouse_ref = mouse_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let state_ref = state_ref.clone();
        let drag_start = drag_start.clone();
        let velocity_ref = velocity_ref.clone();
        
        let target_camera_ref = target_camera_ref.clone();
        let frame_id = frame_id.clone();
        
        use_effect(move || {
            let interval = Interval::new(16, move || {
                let tz = *target_zoom_ref.borrow();
                let tc = *target_camera_ref.borrow();
                let mut z = *zoom_ref.borrow();
                let mut cam = *camera_ref.borrow();
                let mut changed = false;

                // 1. Zoom Interpolation
                if (tz - z).abs() > 0.000001 {
                    let factor = 0.15;
                    let old_z = z;
                    z = z + (tz - z) * factor;
                    *zoom_ref.borrow_mut() = z;
                    
                    if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                        let rect = canvas.get_bounding_client_rect();
                        let mpos = *mouse_ref.borrow();
                        let px = mpos.0 - rect.left();
                        let py = mpos.1 - rect.top();
                        
                        let dx = px - rect.width() / 2.0;
                        let dy = py - rect.height() / 2.0;
                        
                        let ratio = z / old_z;
                        cam.0 = cam.0 * ratio + dx * (ratio - 1.0);
                        cam.1 = cam.1 * ratio + dy * (ratio - 1.0);
                        changed = true;
                    }
                }

                // 2. Momentum
                if drag_start.is_none() {
                    let mut vel = *velocity_ref.borrow();
                    if vel.0.abs() > 0.1 || vel.1.abs() > 0.1 {
                        cam.0 -= vel.0;
                        cam.1 -= vel.1;
                        vel.0 *= 0.94;
                        vel.1 *= 0.94;
                        *velocity_ref.borrow_mut() = vel;
                        changed = true;
                    } else {
                        *velocity_ref.borrow_mut() = (0.0, 0.0);
                    }
                }

                // 3. King Constraints / Smooth Pan
                let (state, player_id) = &*state_ref.borrow();
                let player_id_val = player_id.unwrap_or_else(Uuid::nil);
                let player = state.players.get(&player_id_val);
                let is_alive = player.is_some() && player_id_val != Uuid::nil();

                if is_alive {
                    if let Some(p) = player
                        && let Some(king) = state.pieces.get(&p.king_id) {
                        
                        if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                            let rect = canvas.get_bounding_client_rect();
                            let tile_size = 40.0 * z;
                            
                            // King world pos in pixels
                            let kpx = king.position.x as f64 * tile_size + tile_size / 2.0;
                            let kpy = king.position.y as f64 * tile_size + tile_size / 2.0;
                            
                            // King screen pos
                            let ksx = kpx - cam.0 + rect.width() / 2.0;
                            let ksy = kpy - cam.1 + rect.height() / 2.0;
                            
                            let pad = 150.0 * z.sqrt().min(1.0); // More padding
                            let mut target_cam = cam;
                            let mut force_speed = false;

                            if ksx < pad { target_cam.0 -= pad - ksx; if ksx < 0.0 { force_speed = true; } }
                            if ksx > rect.width() - pad { target_cam.0 += ksx - (rect.width() - pad); if ksx > rect.width() { force_speed = true; } }
                            if ksy < pad { target_cam.1 -= pad - ksy; if ksy < 0.0 { force_speed = true; } }
                            if ksy > rect.height() - pad { target_cam.1 += ksy - (rect.height() - pad); if ksy > rect.height() { force_speed = true; } }

                            if (target_cam.0 - cam.0).abs() > 0.1 || (target_cam.1 - cam.1).abs() > 0.1 {
                                let move_factor = if force_speed { 0.3 } else { 0.1 };
                                cam.0 += (target_cam.0 - cam.0) * move_factor;
                                cam.1 += (target_cam.1 - cam.1) * move_factor;
                                changed = true;
                            }
                        }
                    }
                } else {
                    // Smooth pan to death/reset target when dead
                    if (tc.0 - cam.0).abs() > 0.1 || (tc.1 - cam.1).abs() > 0.1 {
                        cam.0 += (tc.0 - cam.0) * 0.1;
                        cam.1 += (tc.1 - cam.1) * 0.1;
                        changed = true;
                    }
                }

                if changed {
                    *camera_ref.borrow_mut() = cam;
                    zoom_state.set(z);
                    cam_state.set(cam);
                }
                frame_id.set(*frame_id + 1);
            });
            move || drop(interval)
        });
    }

    // Initial Centering on Board (if no King yet)
    {
        let camera_ref = camera_ref.clone();
        let cam_state = cam_state.clone();
        let reducer = props.reducer.clone();
        use_effect_with(reducer.state.board_size, move |board_size| {
            if *board_size > 0 && (reducer.player_id.is_none() || reducer.player_id == Some(Uuid::nil())) {
                *camera_ref.borrow_mut() = (0.0, 0.0);
                cam_state.set((0.0, 0.0));
            }
            || ()
        });
    }

    // Wheel Listener
    {
        let target_zoom_ref = target_zoom_ref.clone();
        let canvas_ref = canvas_ref.clone();
        use_effect_with(canvas_ref.clone(), move |canvas_ref| {
            let canvas = canvas_ref.cast::<web_sys::HtmlElement>().unwrap();
            let target_zoom_ref = target_zoom_ref.clone();
            let listener = EventListener::new(&canvas, "wheel", move |e| {
                let e = e.dyn_ref::<web_sys::WheelEvent>().unwrap();
                e.prevent_default();
                let delta = e.delta_y();
                let factor = 1.2f64.powf(-delta / 100.0);
                let mut tz = *target_zoom_ref.borrow();
                tz = (tz * factor).clamp(0.2, 2.0);
                *target_zoom_ref.borrow_mut() = tz;
            });
            || drop(listener)
        });
    }

    // Window Resize
    {
        let window_size = window_size.clone();
        use_effect_with((), move |_| {
            let listener = EventListener::new(&web_sys::window().unwrap(), "resize", move |_| {
                window_size.set((
                    web_sys::window().unwrap().inner_width().unwrap().as_f64().unwrap(),
                    web_sys::window().unwrap().inner_height().unwrap().as_f64().unwrap()
                ));
            });
            || drop(listener)
        });
    }

    let mut ghost_pieces = props.reducer.state.pieces.clone();
    for pm in &props.reducer.pm_queue {
        if let Some(p) = ghost_pieces.get_mut(&pm.piece_id) {
            p.position = pm.target;
        }
    }

    // Draw Effect
    {
        let canvas_ref = canvas_ref.clone();
        let reducer_handle = props.reducer.clone();
        let sid = selected_piece_id.clone();
        let size = *window_size;
        let ghost_pieces_clone = ghost_pieces.clone();
        let cam = *cam_state;
        let zoom = *zoom_state;
        let fid = *frame_id;
        
        let frame_count_ref = use_mut_ref(|| 0);
        let last_fps_update_ref = use_mut_ref(|| js_sys::Date::now());

        use_effect_with((reducer_handle.clone(), sid, size, cam, zoom, fid), move |(reducer, sid, size, cam, zoom, _fid)| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                canvas.set_width(size.0 as u32);
                canvas.set_height(size.1 as u32);
                let renderer = Renderer::new(canvas, *zoom);
                let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
                renderer.draw_with_ghosts(&reducer.state, player_id, **sid, &reducer.pm_queue, &ghost_pieces_clone, *cam);
                
                // FPS Calculation
                let now = js_sys::Date::now();
                *frame_count_ref.borrow_mut() += 1;
                let elapsed = now - *last_fps_update_ref.borrow();
                if elapsed >= 1000.0 {
                    let fps = (*frame_count_ref.borrow() as f64 / (elapsed / 1000.0)) as u32;
                    reducer_handle.dispatch(GameAction::SetFPS(fps));
                    *frame_count_ref.borrow_mut() = 0;
                    *last_fps_update_ref.borrow_mut() = now;
                }
            }
            || ()
        });
    }

    // Ping Loop
    {
        let tx = props.tx.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(2000, move || {
                let _ = tx.0.send(ClientMessage::Ping(js_sys::Date::now() as u64));
            });
            || drop(interval)
        });
    }

    let on_mousedown = {
        let canvas_ref = canvas_ref.clone();
        let camera_ref = camera_ref.clone();
        let zoom_ref = zoom_ref.clone();
        let reducer = props.reducer.clone();
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
        let velocity_ref = velocity_ref.clone();
        Callback::from(move |e: MouseEvent| {
            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let zoom = *zoom_ref.borrow();
            let tile_size = 40.0 * zoom;
            let x = e.client_x() as f64 - rect.left();
            let y = e.client_y() as f64 - rect.top();
            
            let cam = *camera_ref.borrow();
            let world_x = x + cam.0 - (canvas.width() as f64 / 2.0);
            let world_y = y + cam.1 - (canvas.height() as f64 / 2.0);
            
            let grid_x = (world_x / tile_size).floor() as i32;
            let grid_y = (world_y / tile_size).floor() as i32;
            let target = IVec2::new(grid_x, grid_y);

            let board_size = reducer.state.board_size;
            let mut is_interactive = false;

            if is_within_board(target, board_size) {
                let mut ghosts = reducer.state.pieces.clone();
                for pm in &reducer.pm_queue {
                    if let Some(p) = ghosts.get_mut(&pm.piece_id) { p.position = pm.target; }
                }

                if ghosts.values().any(|p| p.position == target) {
                    is_interactive = true;
                } else if let Some(sid) = *selected_piece_id
                    && let Some(piece) = ghosts.get(&sid) {
                    let is_capture = ghosts.values().any(|p| p.position == target);
                    if is_valid_chess_move(piece.piece_type, piece.position, target, is_capture, board_size) {
                        if piece.piece_type == PieceType::Knight || !is_move_blocked(piece.position, target, &ghosts) {
                            is_interactive = true;
                        }
                    }
                }
            }
            drag_start.set(Some((e.client_x() as f64, e.client_y() as f64, !is_interactive)));
            *velocity_ref.borrow_mut() = (0.0, 0.0);
        })
    };

    let on_mousemove = {
        let drag_start = drag_start.clone();
        let camera_ref = camera_ref.clone();
        let cam_state = cam_state.clone();
        let mouse_ref = mouse_ref.clone();
        let state_ref = state_ref.clone();
        let zoom_ref = zoom_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let velocity_ref = velocity_ref.clone();
        Callback::from(move |e: MouseEvent| {
            *mouse_ref.borrow_mut() = (e.client_x() as f64, e.client_y() as f64);
            if let Some((start_x, start_y, allow_panning)) = *drag_start {
                if !allow_panning { return; }
                let dx = e.client_x() as f64 - start_x;
                let dy = e.client_y() as f64 - start_y;
                if dx.abs() > 1.0 || dy.abs() > 1.0 {
                    let mut cam = *camera_ref.borrow();
                    cam.0 -= dx;
                    cam.1 -= dy;
                    
                    let (state, player_id) = &*state_ref.borrow();
                    let mut valid_pan = true;
                    if let Some(pid) = *player_id && pid != Uuid::nil() {
                        if let Some(player) = state.players.get(&pid)
                            && let Some(king) = state.pieces.get(&player.king_id)
                            && let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                            let rect = canvas.get_bounding_client_rect();
                            let z = *zoom_ref.borrow();
                            let tile_size = 40.0 * z;
                            let kpx = king.position.x as f64 * tile_size + tile_size / 2.0;
                            let kpy = king.position.y as f64 * tile_size + tile_size / 2.0;
                            let ksx = kpx - cam.0 + rect.width() / 2.0;
                            let ksy = kpy - cam.1 + rect.height() / 2.0;
                            if ksx < -50.0 || ksx > rect.width() + 50.0 || ksy < -50.0 || ksy > rect.height() + 50.0 {
                                valid_pan = false;
                            }
                        }
                    }

                    if valid_pan {
                        *camera_ref.borrow_mut() = cam;
                        cam_state.set(cam);
                        *velocity_ref.borrow_mut() = (dx, dy);
                        drag_start.set(Some((e.client_x() as f64, e.client_y() as f64, true)));
                    } else {
                        *velocity_ref.borrow_mut() = (0.0, 0.0);
                        drag_start.set(Some((e.client_x() as f64, e.client_y() as f64, true)));
                    }
                }
            }
        })
    };

    let on_mouseup = {
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
        let selected_piece_id = selected_piece_id.clone();
        let camera_ref = camera_ref.clone();
        let zoom_ref = zoom_ref.clone();
        let drag_start = drag_start.clone();
        let velocity_ref = velocity_ref.clone();
        
        Callback::from(move |e: MouseEvent| {
            let start = *drag_start;
            drag_start.set(None);
            if let Some((sx, sy, allow_panning)) = start {
                let dx = e.client_x() as f64 - sx;
                let dy = e.client_y() as f64 - sy;
                if allow_panning && (dx*dx + dy*dy).sqrt() > 5.0 { 
                    return; 
                }
                if !allow_panning {
                    *velocity_ref.borrow_mut() = (0.0, 0.0);
                }
            } else {
                *velocity_ref.borrow_mut() = (0.0, 0.0);
            }

            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let zoom = *zoom_ref.borrow();
            let tile_size = 40.0 * zoom;
            let x = e.client_x() as f64 - rect.left();
            let y = e.client_y() as f64 - rect.top();
            
            let cam = *camera_ref.borrow();
            let world_x = x + cam.0 - (canvas.width() as f64 / 2.0);
            let world_y = y + cam.1 - (canvas.height() as f64 / 2.0);
            
            let grid_x = (world_x / tile_size).floor() as i32;
            let grid_y = (world_y / tile_size).floor() as i32;
            let target = IVec2::new(grid_x, grid_y);
            let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);

            if e.button() == 2 {
                selected_piece_id.set(None);
                reducer.dispatch(GameAction::ClearPmQueue(Uuid::nil())); 
                return;
            }

            let mut current_ghosts = reducer.state.pieces.clone();
            for pm in &reducer.pm_queue {
                if let Some(p) = current_ghosts.get_mut(&pm.piece_id) { p.position = pm.target; }
            }

            if let Some(sid) = *selected_piece_id {
                let proj_p = current_ghosts.get(&sid);
                if let Some(p) = proj_p {
                    if target == p.position {
                        selected_piece_id.set(None);
                        reducer.dispatch(GameAction::ClearPmQueue(sid));
                    } else if let Some(other) = current_ghosts.values().find(|p| p.position == target && p.owner_id == Some(player_id)) {
                        selected_piece_id.set(Some(other.id));
                    } else {
                        let target_occupied = current_ghosts.values().find(|gp| gp.position == target);
                        let is_capture = target_occupied.is_some() && target_occupied.unwrap().owner_id != Some(player_id);
                        if is_valid_chess_move(p.piece_type, p.position, target, is_capture, reducer.state.board_size) {
                            if p.piece_type == PieceType::Knight || !is_move_blocked(p.position, target, &current_ghosts) {
                                reducer.dispatch(GameAction::AddPmove(Pmove { piece_id: sid, target, pending: false, old_last_move_time: 0, old_cooldown_ms: 0 }));
                            }
                        }
                    }
                } else {
                    selected_piece_id.set(None);
                }
            } else if let Some(piece) = current_ghosts.values().find(|p| p.position == target && p.owner_id == Some(player_id)) {
                selected_piece_id.set(Some(piece.id));
            }
        })
    };

    let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
    let player = props.reducer.state.players.get(&player_id);
    let player_score = player.map(|p| p.score).unwrap_or(0);
    let player_pieces = props.reducer.state.pieces.values().filter(|p| p.owner_id == Some(player_id)).collect::<Vec<_>>();
    let shop_nearby = props.reducer.state.shops.iter().find(|s| player_pieces.iter().any(|p| p.position == s.position));
    
    let piece_on_shop = shop_nearby.and_then(|shop| {
        player_pieces.iter().find(|p| p.position == shop.position)
    });

    let can_shop = shop_nearby.is_some();

    let on_buy = {
        let tx = props.tx.clone();
        let shop_pos = shop_nearby.map(|s| s.position).unwrap_or(IVec2::ZERO);
        Callback::from(move |pt: PieceType| {
            let _ = tx.0.send(ClientMessage::BuyPiece { shop_pos, piece_type: pt });
        })
    };

    let is_alive = props.reducer.state.players.contains_key(&player_id) && player_id != Uuid::nil();

    let shop_ui = if can_shop {
        let piece_count = player_pieces.len();
        let current_piece_type = piece_on_shop.map(|p| p.piece_type).unwrap_or(PieceType::Pawn);
        let current_value = get_piece_value(current_piece_type);
        let is_king_on_shop = current_piece_type == PieceType::King;

        html! {
            <div style="position: absolute; bottom: 40px; left: 50%; transform: translateX(-50%); background: rgba(255, 255, 255, 0.9); padding: 15px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.2); display: flex; flex-direction: column; align-items: center; gap: 10px; z-index: 50;">
                <span style="font-weight: bold; color: #1e3a8a;">{"RECRUITMENT & UPGRADES"}</span>
                <div style="display: flex; gap: 10px;">
                    {
                        [PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen].into_iter().map(|pt| {
                            let cost = get_upgrade_cost(pt, piece_count);
                            let can_afford = player_score >= cost;
                            
                            // Pawn is always shown (as a spawn)
                            // Others are only shown if they are an upgrade (strictly higher value)
                            // And not if current is King (King only sees Pawn)
                            let should_show = if pt == PieceType::Pawn {
                                true
                            } else if is_king_on_shop {
                                false
                            } else {
                                get_piece_value(pt) > current_value
                            };

                            if should_show {
                                let label = match pt {
                                    PieceType::Pawn => "Recruit Pawn",
                                    PieceType::Knight => "Knight",
                                    PieceType::Bishop => "Bishop",
                                    PieceType::Rook => "Rook",
                                    PieceType::Queen => "Queen",
                                    _ => "Unknown",
                                };
                                html! {
                                    <button 
                                        onclick={on_buy.reform(move |_| pt)} 
                                        disabled={!can_afford} 
                                        style={format!(
                                            "padding: 8px 15px; cursor: {}; border-radius: 6px; border: 1px solid #ddd; background: {}; color: {};", 
                                            if can_afford { "pointer" } else { "not-allowed" },
                                            if can_afford { "white" } else { "#f1f5f9" },
                                            if can_afford { "black" } else { "#94a3b8" }
                                        )}
                                    >
                                        {format!("{} ({})", label, cost)}
                                    </button>
                                }
                            } else {
                                html! {}
                            }
                        }).collect::<Html>()
                    }
                </div>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <div style="width: 100%; height: 100%; position: relative;" oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}>
            <canvas ref={canvas_ref} onmousedown={on_mousedown} onmousemove={on_mousemove} onmouseup={on_mouseup} style="display: block; background: #fafafa; cursor: grab;"></canvas>
            
            // Bottom Right Debug Stats
            if is_alive {
                <div style="position: absolute; bottom: 10px; right: 10px; background: rgba(0, 0, 0, 0.4); color: #fff; font-family: monospace; font-size: 10px; padding: 5px 10px; pointer-events: none; z-index: 100; border-radius: 4px; display: flex; flex-direction: column; align-items: flex-end; gap: 2px;">
                    <span>{"FPS: "}{props.reducer.fps}</span>
                    <span>{"PING: "}{props.reducer.ping_ms}{"ms"}</span>
                    <span>{"BOARD: "}{props.reducer.state.board_size}{"x"}{props.reducer.state.board_size}</span>
                </div>
            }

            {shop_ui}
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
