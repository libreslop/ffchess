use crate::camera::{update_camera, CameraManager};
use crate::canvas::Renderer;
use crate::reducer::{GameAction, GameStateReducer, MsgSender, Pmove};
use crate::utils::is_mobile;
use common::models::{Piece, Shop};
use common::logic::{is_within_board, is_valid_move};
use common::protocol::ClientMessage;
use glam::IVec2;
use gloo_events::EventListener;
use gloo_timers::callback::Interval;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
}

#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let canvas_ref = use_node_ref();
    let selected_piece_id = use_state(|| None::<Uuid>);
    let manager_ref = use_mut_ref(CameraManager::new);

    let cam_state = use_state(|| (0.0, 0.0));
    let zoom_state = use_state(|| 1.0f64);
    let frame_id = use_state(|| 0u64);
    let drag_start = use_state(|| None::<(f64, f64, bool)>);

    let window_size = use_state(|| {
        (
            web_sys::window()
                .unwrap()
                .inner_width()
                .unwrap()
                .as_f64()
                .unwrap(),
            web_sys::window()
                .unwrap()
                .inner_height()
                .unwrap()
                .as_f64()
                .unwrap(),
        )
    });

    {
        let zoom_state = zoom_state.clone();
        let cam_state = cam_state.clone();
        let canvas_ref = canvas_ref.clone();
        let drag_start = drag_start.clone();
        let frame_id = frame_id.clone();
        let manager_ref = manager_ref.clone();
        let reducer = props.reducer.clone();

        use_effect(move || {
            let interval = Interval::new(16, move || {
                let mut manager = manager_ref.borrow_mut();
                let is_dragging = drag_start.is_some();
                let changed = update_camera(
                    &mut manager,
                    &reducer.state,
                    reducer.player_id,
                    &canvas_ref,
                    is_dragging,
                );

                if changed {
                    zoom_state.set(manager.zoom);
                    cam_state.set(manager.camera);
                }
                frame_id.set(*frame_id + 1);
            });
            move || drop(interval)
        });
    }

    {
        let manager_ref = manager_ref.clone();
        let canvas_ref = canvas_ref.clone();
        use_effect_with(canvas_ref.clone(), move |canvas_ref| {
            let canvas = canvas_ref.cast::<web_sys::HtmlElement>().unwrap();
            let manager_ref = manager_ref.clone();
            let listener = EventListener::new(&canvas, "wheel", move |e| {
                let e = e.dyn_ref::<web_sys::WheelEvent>().unwrap();
                e.prevent_default();
                let delta = e.delta_y();
                let factor = 1.2f64.powf(-delta / 100.0);
                let mut manager = manager_ref.borrow_mut();
                manager.target_zoom = (manager.target_zoom * factor).clamp(0.2, 2.0);
            });
            || drop(listener)
        });
    }

    {
        let window_size = window_size.clone();
        use_effect_with((), move |_| {
            let listener = EventListener::new(&web_sys::window().unwrap(), "resize", move |_| {
                window_size.set((
                    web_sys::window()
                        .unwrap()
                        .inner_width()
                        .unwrap()
                        .as_f64()
                        .unwrap(),
                    web_sys::window()
                        .unwrap()
                        .inner_height()
                        .unwrap()
                        .as_f64()
                        .unwrap(),
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

        use_effect_with(
            (reducer_handle.clone(), sid, size, cam, zoom, fid),
            move |(reducer, sid, size, cam, zoom, _fid)| {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    canvas.set_width(size.0 as u32);
                    canvas.set_height(size.1 as u32);
                    let renderer = Renderer {
                        ctx: canvas.get_context("2d").unwrap().unwrap().dyn_into().unwrap(),
                        width: size.0,
                        height: size.1,
                        tile_size: 40.0 * zoom,
                        zoom: *zoom,
                        piece_configs: reducer.piece_configs.clone(),
                    };
                    let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);
                    renderer.draw_with_ghosts(
                        &reducer.state,
                        player_id,
                        **sid,
                        &reducer.pm_queue,
                        &ghost_pieces_clone,
                        *cam,
                    );

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
            },
        );
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

    let handle_input_start = {
        let canvas_ref = canvas_ref.clone();
        let manager_ref = manager_ref.clone();
        let reducer = props.reducer.clone();
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
        Callback::from(move |(cx, cy, is_right_click): (f64, f64, bool)| {
            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let mut manager = manager_ref.borrow_mut();
            let zoom = manager.zoom;
            let tile_size = 40.0 * zoom;
            let x = cx - rect.left();
            let y = cy - rect.top();

            let world_x = x + manager.camera.0 - (canvas.width() as f64 / 2.0);
            let world_y = y + manager.camera.1 - (canvas.height() as f64 / 2.0);

            let grid_x = (world_x / tile_size).floor() as i32;
            let grid_y = (world_y / tile_size).floor() as i32;
            let target = IVec2::new(grid_x, grid_y);

            let board_size = reducer.state.board_size;
            let mut is_interactive = false;

            if !is_right_click && is_within_board(target, board_size) {
                let mut ghosts = reducer.state.pieces.clone();
                for pm in &reducer.pm_queue {
                    if let Some(p) = ghosts.get_mut(&pm.piece_id) {
                        p.position = pm.target;
                    }
                }

                if ghosts.values().any(|p| p.position == target) {
                    is_interactive = true;
                } else if let Some(sid) = *selected_piece_id
                    && let Some(piece) = ghosts.get(&sid)
                    && let Some(config) = reducer.piece_configs.get(&piece.piece_type)
                    && is_valid_move(
                        config,
                        piece.position,
                        target,
                        ghosts.values().any(|p| p.position == target),
                        board_size,
                        &ghosts,
                        piece.owner_id,
                    )
                {
                    is_interactive = true;
                }
            }
            drag_start.set(Some((cx, cy, !is_interactive)));
            manager.velocity = (0.0, 0.0);
        })
    };

    let handle_input_move = {
        let drag_start = drag_start.clone();
        let cam_state = cam_state.clone();
        let manager_ref = manager_ref.clone();
        let reducer = props.reducer.clone();
        let canvas_ref = canvas_ref.clone();
        Callback::from(move |(cx, cy): (f64, f64)| {
            let mut manager = manager_ref.borrow_mut();
            manager.mouse_pos = (cx, cy);
            if let Some((start_x, start_y, allow_panning)) = *drag_start {
                if !allow_panning {
                    return;
                }
                let dx = cx - start_x;
                let dy = cy - start_y;
                if dx.abs() > 0.1 || dy.abs() > 0.1 {
                    let mut cam = manager.camera;
                    cam.0 -= dx;
                    cam.1 -= dy;

                    let mut valid_pan = true;
                    let player_id_val = reducer.player_id.unwrap_or_else(Uuid::nil);
                    let is_alive = reducer.state.players.contains_key(&player_id_val)
                        && player_id_val != Uuid::nil();

                    if is_alive
                        && let Some(player) = reducer.state.players.get(&player_id_val)
                        && let Some(king) = reducer.state.pieces.get(&player.king_id)
                        && let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>()
                    {
                        let rect = canvas.get_bounding_client_rect();
                        let z = manager.zoom;
                        let tile_size = 40.0 * z;
                        let kpx = king.position.x as f64 * tile_size + tile_size / 2.0;
                        let kpy = king.position.y as f64 * tile_size + tile_size / 2.0;
                        let ksx = kpx - cam.0 + rect.width() / 2.0;
                        let ksy = kpy - cam.1 + rect.height() / 2.0;
                        if ksx < -50.0
                            || ksx > rect.width() + 50.0
                            || ksy < -50.0
                            || ksy > rect.height() + 50.0
                        {
                            valid_pan = false;
                        }
                    }

                    if valid_pan {
                        manager.camera = cam;
                        if !is_alive {
                            manager.target_camera = cam;
                        }
                        cam_state.set(cam);
                        manager.velocity = (dx, dy);
                        drag_start.set(Some((cx, cy, true)));
                    } else {
                        manager.velocity = (0.0, 0.0);
                        drag_start.set(Some((cx, cy, true)));
                    }
                }
            }
        })
    };

    let handle_input_end = {
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
        let selected_piece_id = selected_piece_id.clone();
        let manager_ref = manager_ref.clone();
        let drag_start = drag_start.clone();

        Callback::from(move |(cx, cy, is_right_click): (f64, f64, bool)| {
            let start = *drag_start;
            drag_start.set(None);
            
            let mut is_tap = true;
            if let Some((sx, sy, allow_panning)) = start {
                let dx = cx - sx;
                let dy = cy - sy;
                let dist = (dx * dx + dy * dy).sqrt();
                if allow_panning && dist > 10.0 {
                    is_tap = false;
                }
                if !allow_panning {
                    manager_ref.borrow_mut().velocity = (0.0, 0.0);
                }
            } else {
                manager_ref.borrow_mut().velocity = (0.0, 0.0);
            }

            if !is_tap {
                return;
            }

            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let manager = manager_ref.borrow_mut();
            let zoom = manager.zoom;
            let tile_size = 40.0 * zoom;
            let x = cx - rect.left();
            let y = cy - rect.top();

            let world_x = x + manager.camera.0 - (canvas.width() as f64 / 2.0);
            let world_y = y + manager.camera.1 - (canvas.height() as f64 / 2.0);

            let grid_x = (world_x / tile_size).floor() as i32;
            let grid_y = (world_y / tile_size).floor() as i32;
            let target = IVec2::new(grid_x, grid_y);
            let player_id = reducer.player_id.unwrap_or_else(Uuid::nil);

            if is_right_click {
                selected_piece_id.set(None);
                reducer.dispatch(GameAction::ClearPmQueue(Uuid::nil()));
                return;
            }

            let mut current_ghosts = reducer.state.pieces.clone();
            for pm in &reducer.pm_queue {
                if let Some(p) = current_ghosts.get_mut(&pm.piece_id) {
                    p.position = pm.target;
                }
            }

            if let Some(sid) = *selected_piece_id {
                let proj_p = current_ghosts.get(&sid);
                if let Some(p) = proj_p {
                    if target == p.position {
                        selected_piece_id.set(None);
                        reducer.dispatch(GameAction::ClearPmQueue(sid));
                    } else if let Some(other) = current_ghosts
                        .values()
                        .find(|p| p.position == target && p.owner_id == Some(player_id))
                    {
                        selected_piece_id.set(Some(other.id));
                    } else {
                        let target_occupied =
                            current_ghosts.values().find(|gp| gp.position == target);
                        let is_capture = target_occupied.is_some()
                            && target_occupied.unwrap().owner_id != Some(player_id);
                        
                        if let Some(config) = reducer.piece_configs.get(&p.piece_type) {
                            if is_valid_move(
                                config,
                                p.position,
                                target,
                                is_capture,
                                reducer.state.board_size,
                                &current_ghosts,
                                p.owner_id,
                            ) {
                                reducer.dispatch(GameAction::AddPmove(Pmove {
                                    piece_id: sid,
                                    target,
                                    pending: false,
                                    old_last_move_time: 0,
                                    old_cooldown_ms: 0,
                                }));
                            }
                        }
                    }
                } else {
                    selected_piece_id.set(None);
                }
            } else if let Some(piece) = current_ghosts
                .values()
                .find(|p| p.position == target && p.owner_id == Some(player_id))
            {
                selected_piece_id.set(Some(piece.id));
            }
        })
    };

    let on_mousedown = {
        let handle_input_start = handle_input_start.clone();
        Callback::from(move |e: MouseEvent| {
            handle_input_start.emit((e.client_x() as f64, e.client_y() as f64, e.button() == 2));
        })
    };

    let on_mousemove = {
        let handle_input_move = handle_input_move.clone();
        Callback::from(move |e: MouseEvent| {
            handle_input_move.emit((e.client_x() as f64, e.client_y() as f64));
        })
    };

    let on_mouseup = {
        let handle_input_end = handle_input_end.clone();
        Callback::from(move |e: MouseEvent| {
            handle_input_end.emit((e.client_x() as f64, e.client_y() as f64, e.button() == 2));
        })
    };

    let on_touchstart = {
        let handle_input_start = handle_input_start.clone();
        let manager_ref = manager_ref.clone();
        Callback::from(move |e: TouchEvent| {
            let e = e.dyn_ref::<web_sys::TouchEvent>().unwrap();
            let touches = e.touches();
            if touches.length() == 1 {
                let touch = touches.get(0).unwrap();
                handle_input_start.emit((touch.client_x() as f64, touch.client_y() as f64, false));
                e.prevent_default();
            } else if touches.length() == 2 {
                let t1 = touches.get(0).unwrap();
                let t2 = touches.get(1).unwrap();
                let dx = t1.client_x() - t2.client_x();
                let dy = t1.client_y() - t2.client_y();
                let dist = ((dx * dx + dy * dy) as f64).sqrt();
                
                let mut manager = manager_ref.borrow_mut();
                manager.last_touch_dist = Some(dist);
                
                // Centering zoom on midpoint of two fingers
                manager.mouse_pos = (
                    (t1.client_x() + t2.client_x()) as f64 / 2.0,
                    (t1.client_y() + t2.client_y()) as f64 / 2.0
                );
                
                e.prevent_default();
            }
        })
    };

    let on_touchmove = {
        let handle_input_move = handle_input_move.clone();
        let manager_ref = manager_ref.clone();
        Callback::from(move |e: TouchEvent| {
            let e = e.dyn_ref::<web_sys::TouchEvent>().unwrap();
            let touches = e.touches();
            if touches.length() == 1 {
                let touch = touches.get(0).unwrap();
                handle_input_move.emit((touch.client_x() as f64, touch.client_y() as f64));
                e.prevent_default();
            } else if touches.length() == 2 {
                let t1 = touches.get(0).unwrap();
                let t2 = touches.get(1).unwrap();
                let dx = t1.client_x() - t2.client_x();
                let dy = t1.client_y() - t2.client_y();
                let dist = ((dx * dx + dy * dy) as f64).sqrt();

                let mut manager = manager_ref.borrow_mut();
                if let Some(last_dist) = manager.last_touch_dist {
                    let ratio = dist / last_dist;
                    manager.target_zoom = (manager.target_zoom * ratio).clamp(0.2, 2.0);
                    manager.last_touch_dist = Some(dist);
                    
                    manager.mouse_pos = (
                        (t1.client_x() + t2.client_x()) as f64 / 2.0,
                        (t1.client_y() + t2.client_y()) as f64 / 2.0
                    );
                }
                e.prevent_default();
            }
        })
    };

    let on_touchend = {
        let handle_input_end = handle_input_end.clone();
        let manager_ref = manager_ref.clone();
        Callback::from(move |e: TouchEvent| {
            let e = e.dyn_ref::<web_sys::TouchEvent>().unwrap();
            {
                let mut manager = manager_ref.borrow_mut();
                manager.last_touch_dist = None;
            }
            let touches = e.changed_touches();
            if touches.length() > 0 {
                let touch = touches.get(0).unwrap();
                handle_input_end.emit((touch.client_x() as f64, touch.client_y() as f64, false));
                e.prevent_default();
            }
        })
    };

    let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
    let player = props.reducer.state.players.get(&player_id);
    let player_score = player.map(|p| p.score).unwrap_or(0);
    let player_pieces = props
        .reducer
        .state
        .pieces
        .values()
        .filter(|p| p.owner_id == Some(player_id))
        .collect::<Vec<_>>();
    let shop_nearby = props
        .reducer
        .state
        .shops
        .iter()
        .find(|s| player_pieces.iter().any(|p| p.position == s.position));

    let piece_on_shop =
        shop_nearby.and_then(|shop| player_pieces.iter().find(|p| p.position == shop.position)).copied();

    let is_alive = props.reducer.state.players.contains_key(&player_id) && player_id != Uuid::nil();

    let shop_ui = if let Some(shop) = shop_nearby {
        let piece_count = player_pieces.len();
        if let Some(shop_config) = props.reducer.shop_configs.get(&shop.shop_id) {
            html! {
                <crate::components::ShopUI
                    player_score={player_score}
                    player_pieces_count={piece_count}
                    piece_on_shop={piece_on_shop.cloned()}
                    shop_config={shop_config.clone()}
                    piece_configs={props.reducer.piece_configs.clone()}
                    shop_pos={shop.position}
                    tx={props.tx.clone()}
                />
            }
        } else {
            html! {}
        }
    } else {
        html! {}
    };

    let mobile = is_mobile();
    let hud_style = if mobile {
        "position: absolute; bottom: 2px; right: 2px; background: transparent; color: #000; font-family: monospace; font-size: 8px; padding: 0; pointer-events: none; z-index: 100; display: flex; flex-direction: column; align-items: flex-end; gap: 0;"
    } else {
        "position: absolute; bottom: 5px; right: 5px; background: transparent; color: #000; font-family: monospace; font-size: 10px; padding: 0; pointer-events: none; z-index: 100; display: flex; flex-direction: column; align-items: flex-end; gap: 0;"
    };

    html! {
        <div style="width: 100%; height: 100%; position: relative;" oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}>
            <canvas ref={canvas_ref}
                onmousedown={on_mousedown} onmousemove={on_mousemove} onmouseup={on_mouseup}
                ontouchstart={on_touchstart} ontouchmove={on_touchmove} ontouchend={on_touchend}
                style="display: block; background: #fafafa; cursor: grab; touch-action: none;"></canvas>

            if is_alive {
                <div style={hud_style}>
                    <span>{"FPS: "}{props.reducer.fps}</span>
                    <span>{"PING: "}{props.reducer.ping_ms}{"ms"}</span>
                    <span>{"BOARD: "}{props.reducer.state.board_size}{"x"}{props.reducer.state.board_size}</span>
                </div>
            }

            {shop_ui}
        </div>
    }
}
