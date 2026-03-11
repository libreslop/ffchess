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
use glam::IVec2;
use uuid::Uuid;
use gloo_events::EventListener;
use gloo_timers::callback::Interval;

#[function_component(App)]
pub fn app() -> Html {
    let reducer = use_reducer(GameStateReducer::default);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(String::new);
    
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
            spawn_local(async move {
                let window = web_sys::window().unwrap();
                let host = window.location().host().unwrap();
                let protocol = if window.location().protocol().unwrap() == "https:" { "wss:" } else { "ws:" };
                let ws_url = format!("{}//{}/api/ws", protocol, host);
                
                if let Ok(ws) = WebSocket::open(&ws_url) {
                    let (mut write, mut read) = ws.split();
                    spawn_local(async move {
                        while let Some(msg) = client_rx.recv().await {
                            let _ = write.send(Message::Text(serde_json::to_string(&msg).unwrap())).await;
                        }
                    });

                    while let Some(msg) = read.next().await {
                        if let Ok(Message::Text(text)) = msg
                            && let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                            listener_reducer_ref.borrow().clone().dispatch(match server_msg {
                                ServerMessage::Init { player_id, state } => GameAction::SetInit { player_id, state },
                                ServerMessage::UpdateState { players, pieces, shops, removed_pieces, removed_players } => GameAction::UpdateState { players, pieces, shops, removed_pieces, removed_players },
                                ServerMessage::Error(e) => GameAction::SetError(e),
                                ServerMessage::GameOver { final_score } => GameAction::GameOver { final_score },
                            });
                        }
                    }
                }
            });
            || drop(interval)
        });
    }

    let on_join = {
        let tx = tx.clone();
        let player_name = player_name.clone();
        Callback::from(move |kit: KitType| {
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

    let is_joined = reducer.player_id.is_some() && reducer.player_id != Some(Uuid::nil());
    let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
    let player = reducer.state.players.get(&player_id);
    let is_dead = is_joined && player.is_none();

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
        <div style="font-family: 'Segoe UI', Arial, sans-serif; margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            if let Some(sender) = (*tx).clone() {
                <GameView reducer={reducer.clone()} tx={sender} />
            } else {
                <div style="position: absolute; inset: 0; background: #f0f2f5; display: flex; align-items: center; justify-content: center; z-index: 200;">
                    <div style="text-align: center;">
                        <h2 style="color: #64748b;">{"Connecting to server..."}</h2>
                        <div style="width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top: 4px solid #2563eb; border-radius: 50%; margin: 20px auto; animation: spin 1s linear infinite;"></div>
                        <style>{"@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }"}</style>
                    </div>
                </div>
            }

            if is_joined {
                if is_dead {
                    <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
                    <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); background: white; padding: 40px; border-radius: 12px; box-shadow: 0 15px 35px rgba(0,0,0,0.3); z-index: 100; text-align: center; width: 350px;">
                        <h1 style="color: #dc2626; margin-top: 0;">{"GAME OVER"}</h1>
                        <div style="margin: 25px 0; font-size: 1.2em;">
                            <p style="margin: 5px 0; color: #666;">{"Your King was captured!"}</p>
                            <div style="background: #f8fafc; padding: 15px; border-radius: 8px; margin-top: 20px;">
                                <span style="display: block; font-size: 0.8em; text-transform: uppercase; color: #94a3b8; letter-spacing: 1px;">{"Final Score"}</span>
                                <span style="font-size: 2.5em; font-weight: bold; color: #1e293b;">{reducer.last_score}</span>
                            </div>
                        </div>
                        <button onclick={|_| web_sys::window().unwrap().location().reload().unwrap()} 
                            style="padding: 12px 30px; font-size: 1.1em; cursor: pointer; background: #2563eb; color: white; border: none; border-radius: 8px; font-weight: bold; width: 100%; box-shadow: 0 4px 6px rgba(37, 99, 235, 0.2);">
                            {"Play Again"}
                        </button>
                    </div>
                } else {
                    <div style="position: absolute; top: 20px; right: 20px; background: rgba(255, 255, 255, 0.9); padding: 15px; border-radius: 10px; box-shadow: 0 4px 15px rgba(0,0,0,0.1); width: 200px; z-index: 60; pointer-events: none;">
                        <h3 style="margin: 0 0 10px 0; border-bottom: 1px solid #eee; padding-bottom: 5px;">{"Leaderboard"}</h3>
                        <div style="display: flex; flex-direction: column; gap: 5px;">
                            {leaderboard_items}
                        </div>
                    </div>
                }
            } else if tx.is_some() {
                <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.4); z-index: 90;"></div>
                <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); background: white; padding: 30px; border-radius: 12px; box-shadow: 0 10px 25px rgba(0,0,0,0.2); z-index: 100; text-align: center; width: 400px;">
                    <h1>{"FFChess"}</h1>
                    <div style="margin-bottom: 20px; text-align: left;">
                        <label style="display: block; margin-bottom: 5px; font-weight: bold;">{"Your Name:"}</label>
                        <input type="text" value={(*player_name).clone()} oninput={on_name_input} placeholder="Enter name..."
                            style="padding: 10px; border-radius: 6px; border: 1px solid #ddd; width: 100%; box-sizing: border-box; font-size: 1.1em;"/>
                    </div>
                    <h3>{"Choose your starting Kit:"}</h3>
                    <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 15px;">
                        <button onclick={on_join.reform(|_| KitType::Standard)} style="padding: 15px; cursor: pointer; border-radius: 8px; border: 1px solid #ddd; background: white; font-weight: bold;">
                            {"Standard"}<br/><span style="font-weight: normal; font-size: 0.8em;">{"2 Pawns, 2 Knights"}</span>
                        </button>
                        <button onclick={on_join.reform(|_| KitType::Shield)} style="padding: 15px; cursor: pointer; border-radius: 8px; border: 1px solid #ddd; background: white; font-weight: bold;">
                            {"Shield"}<br/><span style="font-weight: normal; font-size: 0.8em;">{"6 Pawns"}</span>
                        </button>
                        <button onclick={on_join.reform(|_| KitType::Scout)} style="padding: 15px; cursor: pointer; border-radius: 8px; border: 1px solid #ddd; background: white; font-weight: bold;">
                            {"Scout"}<br/><span style="font-weight: normal; font-size: 0.8em;">{"1 Pawn, 2 Bishops"}</span>
                        </button>
                        <button onclick={on_join.reform(|_| KitType::Tank)} style="padding: 15px; cursor: pointer; border-radius: 8px; border: 1px solid #ddd; background: white; font-weight: bold;">
                            {"Tank"}<br/><span style="font-weight: normal; font-size: 0.8em;">{"1 Rook"}</span>
                        </button>
                    </div>
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
    let zoom_ref = use_mut_ref(|| 1.0f64);
    let target_zoom_ref = use_mut_ref(|| 1.0f64);
    let mouse_ref = use_mut_ref(|| (0.0, 0.0));
    
    // Stable ref to state for the loop
    let state_ref = use_mut_ref(|| (props.reducer.state.clone(), props.reducer.player_id));
    *state_ref.borrow_mut() = (props.reducer.state.clone(), props.reducer.player_id);

    // States for rendering triggering
    let zoom_state = use_state(|| 1.0f64);
    let cam_state = use_state(|| (0.0, 0.0));
    let drag_start = use_state(|| None::<(f64, f64, bool)>);
    
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
        
        use_effect(move || {
            let interval = Interval::new(16, move || {
                let tz = *target_zoom_ref.borrow();
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

                // 2. King Constraints
                let (state, player_id) = &*state_ref.borrow();
                if let Some(pid) = *player_id && pid != Uuid::nil() {
                    if let Some(player) = state.players.get(&pid)
                        && let Some(king) = state.pieces.get(&player.king_id) {
                        
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
                }

                if changed {
                    *camera_ref.borrow_mut() = cam;
                    zoom_state.set(z);
                    cam_state.set(cam);
                }
            });
            move || drop(interval)
        });
    }

    // Centering on King (one-time or on board changes)
    {
        let camera_ref = camera_ref.clone();
        let cam_state = cam_state.clone();
        let reducer = props.reducer.clone();
        let zoom_ref = zoom_ref.clone();
        use_effect_with((reducer.player_id, reducer.state.board_size), move |(pid, board_size)| {
            let zoom = *zoom_ref.borrow();
            let tile_size = 40.0 * zoom;
            if let Some(player_id) = *pid {
                if player_id != Uuid::nil() {
                    if let Some(player) = reducer.state.players.get(&player_id)
                        && let Some(king) = reducer.state.pieces.get(&player.king_id) {
                        let nx = king.position.x as f64 * tile_size + tile_size / 2.0;
                        let ny = king.position.y as f64 * tile_size + tile_size / 2.0;
                        *camera_ref.borrow_mut() = (nx, ny);
                        cam_state.set((nx, ny));
                    }
                } else if *board_size > 0 {
                    let nx = *board_size as f64 * tile_size / 2.0;
                    let ny = *board_size as f64 * tile_size / 2.0;
                    *camera_ref.borrow_mut() = (nx, ny);
                    cam_state.set((nx, ny));
                }
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
        let reducer = props.reducer.clone();
        let sid = selected_piece_id.clone();
        let size = *window_size;
        let ghost_pieces_clone = ghost_pieces.clone();
        let cam = *cam_state;
        let zoom = *zoom_state;
        use_effect_with((reducer.clone(), sid, size, cam, zoom), move |(reducer, sid, size, cam, zoom)| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                canvas.set_width(size.0 as u32);
                canvas.set_height(size.1 as u32);
                let renderer = Renderer::new(canvas, *zoom);
                let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
                renderer.draw_with_ghosts(&reducer.state, player_id, **sid, &reducer.pm_queue, &ghost_pieces_clone, *cam);
            }
            || ()
        });
    }

    let on_mousedown = {
        let canvas_ref = canvas_ref.clone();
        let camera_ref = camera_ref.clone();
        let zoom_ref = zoom_ref.clone();
        let reducer = props.reducer.clone();
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
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
                        drag_start.set(Some((e.client_x() as f64, e.client_y() as f64, true)));
                    } else {
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
        
        Callback::from(move |e: MouseEvent| {
            let start = *drag_start;
            drag_start.set(None);
            if let Some((sx, sy, _)) = start {
                let dx = e.client_x() as f64 - sx;
                let dy = e.client_y() as f64 - sy;
                if (dx*dx + dy*dy).sqrt() > 5.0 { return; }
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
                                reducer.dispatch(GameAction::AddPmove(Pmove { piece_id: sid, target, pending: false }));
                            }
                        }
                    }
                }
            } else if let Some(piece) = current_ghosts.values().find(|p| p.position == target && p.owner_id == Some(player_id)) {
                selected_piece_id.set(Some(piece.id));
            }
        })
    };

    let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
    let player_pieces = props.reducer.state.pieces.values().filter(|p| p.owner_id == Some(player_id)).collect::<Vec<_>>();
    let shop_nearby = props.reducer.state.shops.iter().find(|s| player_pieces.iter().any(|p| p.position == s.position));

    let on_buy = {
        let tx = props.tx.clone();
        let shop_pos = shop_nearby.map(|s| s.position).unwrap_or(IVec2::ZERO);
        Callback::from(move |pt: PieceType| {
            let _ = tx.0.send(ClientMessage::BuyPiece { shop_pos, piece_type: pt });
        })
    };

    html! {
        <div style="width: 100%; height: 100%; position: relative;" oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}>
            <canvas ref={canvas_ref} onmousedown={on_mousedown} onmousemove={on_mousemove} onmouseup={on_mouseup} style="display: block; background: #fafafa; cursor: grab;"></canvas>
            <div style="position: absolute; top: 20px; left: 20px; pointer-events: none; display: flex; flex-direction: column; gap: 10px;">
                if player_id != Uuid::nil() {
                    <div style="background: rgba(255, 255, 255, 0.9); padding: 10px 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); font-weight: bold; font-size: 1.5em; pointer-events: auto;">{"Score: "}{props.reducer.last_score}</div>
                }
                if let Some(error) = &props.reducer.error {
                    <div style="background: rgba(254, 226, 226, 0.9); color: #dc2626; padding: 10px 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); pointer-events: auto;">{error}</div>
                }
            </div>
            if let Some(shop) = shop_nearby {
                <div style="position: absolute; bottom: 40px; left: 50%; transform: translateX(-50%); background: rgba(255, 255, 255, 0.9); padding: 15px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.2); display: flex; flex-direction: column; align-items: center; gap: 10px; z-index: 50;">
                    <span style="font-weight: bold; color: #856404;">{format!("Shop Area ({:?})", shop.shop_type)}</span>
                    <div style="display: flex; gap: 10px;">
                        <button onclick={on_buy.reform(|_| PieceType::Pawn)} style="padding: 8px 15px; cursor: pointer; border-radius: 6px; border: 1px solid #ddd; background: white;">{"Pawn (10)"}</button>
                        <button onclick={on_buy.reform(|_| PieceType::Knight)} style="padding: 8px 15px; cursor: pointer; border-radius: 6px; border: 1px solid #ddd; background: white;">{"Knight (50+)"}</button>
                        <button onclick={on_buy.reform(|_| PieceType::Rook)} style="padding: 8px 15px; cursor: pointer; border-radius: 6px; border: 1px solid #ddd; background: white;">{"Rook (100+)"}</button>
                        <button onclick={on_buy.reform(|_| PieceType::Queen)} style="padding: 8px 15px; cursor: pointer; border-radius: 6px; border: 1px solid #ddd; background: white;">{"Queen (250+)"}</button>
                    </div>
                </div>
            }
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
