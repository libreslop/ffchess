use yew::prelude::*;
use common::*;
use crate::reducer::{GameStateReducer, GameAction, Pmove, MsgSender};
use crate::canvas::Renderer;
use glam::IVec2;
use uuid::Uuid;
use gloo_events::EventListener;
use gloo_timers::callback::Interval;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
}

#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let canvas_ref = use_node_ref();
    let selected_piece_id = use_state(|| None::<Uuid>);
    
    let camera_ref = use_mut_ref(|| (0.0, 0.0));
    let target_camera_ref = use_mut_ref(|| (0.0, 0.0));
    let zoom_ref = use_mut_ref(|| 1.0f64);
    let target_zoom_ref = use_mut_ref(|| 1.0f64);
    let mouse_ref = use_mut_ref(|| (0.0, 0.0));
    
    let state_ref = use_mut_ref(|| (props.reducer.state.clone(), props.reducer.player_id));
    *state_ref.borrow_mut() = (props.reducer.state.clone(), props.reducer.player_id);

    let zoom_state = use_state(|| 1.0f64);
    let cam_state = use_state(|| (0.0, 0.0));
    let frame_id = use_state(|| 0u64);
    let drag_start = use_state(|| None::<(f64, f64, bool)>);
    let velocity_ref = use_mut_ref(|| (0.0f64, 0.0f64));

    let was_alive_ref = use_mut_ref(|| false);
    let last_king_grid_pos_ref = use_mut_ref(|| IVec2::ZERO);

    {
        let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
        let player = props.reducer.state.players.get(&player_id);
        let is_alive = player.is_some() && player_id != Uuid::nil();
        let mut was_alive = was_alive_ref.borrow_mut();
        
        if is_alive {
            if let Some(p) = player
                && let Some(king) = props.reducer.state.pieces.get(&p.king_id) {
                *last_king_grid_pos_ref.borrow_mut() = king.position;
            }
            *was_alive = true;
        } else if *was_alive {
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
        } else if player_id == Uuid::nil() && !*was_alive
            && *target_camera_ref.borrow() != (0.0, 0.0) {
            *target_camera_ref.borrow_mut() = (0.0, 0.0);
            *target_zoom_ref.borrow_mut() = 1.0;
        }
    }
    
    let window_size = use_state(|| (
        web_sys::window().unwrap().inner_width().unwrap().as_f64().unwrap(),
        web_sys::window().unwrap().inner_height().unwrap().as_f64().unwrap()
    ));

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

                if (tz - z).abs() > 0.000001 {
                    let factor = 0.15;
                    let old_z = z;
                    z += (tz - z) * factor;
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

                let (state, player_id) = &*state_ref.borrow();
                let player_id_val = player_id.unwrap_or_else(Uuid::nil);
                let player = state.players.get(&player_id_val);
                let is_alive = player.is_some() && player_id_val != Uuid::nil();

                if is_alive {
                    if let Some(p) = player
                        && let Some(king) = state.pieces.get(&p.king_id)
                        && let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                        let rect = canvas.get_bounding_client_rect();
                        let tile_size = 40.0 * z;
                        let kpx = king.position.x as f64 * tile_size + tile_size / 2.0;
                        let kpy = king.position.y as f64 * tile_size + tile_size / 2.0;
                        let ksx = kpx - cam.0 + rect.width() / 2.0;
                        let ksy = kpy - cam.1 + rect.height() / 2.0;
                        
                        let pad = 150.0 * z.sqrt().min(1.0);
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
                } else if (tc.0 - cam.0).abs() > 0.1 || (tc.1 - cam.1).abs() > 0.1 {
                    cam.0 += (tc.0 - cam.0) * 0.1;
                    cam.1 += (tc.1 - cam.1) * 0.1;
                    changed = true;
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
        let last_fps_update_ref = use_mut_ref(js_sys::Date::now);

        use_effect_with((reducer_handle.clone(), sid, size, cam, zoom, fid), move |(reducer, sid, size, cam, zoom, _fid)| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                canvas.set_width(size.0 as u32);
                canvas.set_height(size.1 as u32);
                let renderer = Renderer::new(canvas, *zoom);
                let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
                renderer.draw_with_ghosts(&reducer.state, player_id, **sid, &reducer.pm_queue, &ghost_pieces_clone, *cam);
                
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
                    && let Some(piece) = ghosts.get(&sid)
                    && is_valid_chess_move(piece.piece_type, piece.position, target, ghosts.values().any(|p| p.position == target), board_size)
                    && (piece.piece_type == PieceType::Knight || !is_move_blocked(piece.position, target, &ghosts)) {
                    is_interactive = true;
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
                    if let Some(pid) = *player_id && pid != Uuid::nil()
                        && let Some(player) = state.players.get(&pid)
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
                        if is_valid_chess_move(p.piece_type, p.position, target, is_capture, reducer.state.board_size)
                            && (p.piece_type == PieceType::Knight || !is_move_blocked(p.position, target, &current_ghosts)) {
                            reducer.dispatch(GameAction::AddPmove(Pmove { piece_id: sid, target, pending: false, old_last_move_time: 0, old_cooldown_ms: 0 }));
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
