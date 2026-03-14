use crate::camera::{update_camera, CameraManager};
use crate::canvas::Renderer;
use crate::reducer::{GameAction, GameStateReducer, MsgSender, Pmove};
use common::logic::is_within_board;
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
    let renderer_state = use_state(|| None::<Renderer>);

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

    // Initialize renderer when canvas is bound
    {
        let renderer_state = renderer_state.clone();
        let piece_configs = props.reducer.piece_configs.clone();
        use_effect_with(canvas_ref.clone(), move |canvas_ref| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                renderer_state.set(Some(Renderer::new(canvas, piece_configs)));
            }
            || ()
        });
    }

    // We use a ref to track the latest state for the interval to avoid stale captures
    let latest_state = use_mut_ref(|| (props.reducer.clone(), (*drag_start).is_some()));
    {
        let mut s = latest_state.borrow_mut();
        s.0 = props.reducer.clone();
        s.1 = (*drag_start).is_some();
    }

    {
        let zoom_state = zoom_state.clone();
        let cam_state = cam_state.clone();
        let canvas_ref = canvas_ref.clone();
        let frame_id = frame_id.clone();
        let manager_ref = manager_ref.clone();
        let latest_state = latest_state.clone();

        use_effect(move || {
            let interval = Interval::new(16, move || {
                let (reducer, is_dragging) = {
                    let s = latest_state.borrow();
                    (s.0.clone(), s.1)
                };
                
                let mut manager = manager_ref.borrow_mut();
                
                let player_id_val = reducer.player_id.unwrap_or_else(Uuid::nil);
                let piece_count = reducer.state.pieces.values().filter(|p| p.owner_id == Some(player_id_val)).count();
                
                let changed = update_camera(
                    &mut manager,
                    &reducer.state,
                    reducer.player_id,
                    &canvas_ref,
                    is_dragging,
                    reducer.mode.as_ref(),
                    piece_count,
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
            if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                let manager_ref = manager_ref.clone();
                let listener = EventListener::new(&canvas, "wheel", move |e| {
                    let e = e.dyn_ref::<web_sys::WheelEvent>().unwrap();
                    e.prevent_default();
                    let delta = e.delta_y();
                    let factor = 1.2f64.powf(-delta / 100.0);
                    let mut manager = manager_ref.borrow_mut();
                    manager.target_zoom = (manager.target_zoom * factor).clamp(0.2, 2.0);
                });
                return Box::new(move || drop(listener)) as Box<dyn FnOnce()>;
            }
            Box::new(|| ()) as Box<dyn FnOnce()>
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

    let handle_input_start = {
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
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
                    && common::logic::is_valid_move(
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
                    manager.camera.0 -= dx;
                    manager.camera.1 -= dy;

                    let player_id_val = reducer.player_id.unwrap_or_else(Uuid::nil);
                    let is_alive = reducer.state.players.contains_key(&player_id_val)
                        && player_id_val != Uuid::nil();

                    if !is_alive {
                        manager.target_camera = manager.camera;
                    }
                    cam_state.set(manager.camera);
                    manager.velocity = (dx, dy);
                    drag_start.set(Some((cx, cy, true)));
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
                            if common::logic::is_valid_move(
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
                }
            } else {
                let piece = current_ghosts
                    .values()
                    .find(|p| p.position == target && p.owner_id == Some(player_id));
                if let Some(p) = piece {
                    selected_piece_id.set(Some(p.id));
                }
            }
        })
    };

    let player_id_val = props.reducer.player_id.unwrap_or_else(Uuid::nil);
    {
        let renderer_state = renderer_state.clone();
        let mode = props.reducer.mode.clone();
        use_effect_with(
            (
                frame_id,
                props.reducer.state.clone(),
                (*selected_piece_id).clone(),
                props.reducer.pm_queue.clone(),
                cam_state.clone(),
                zoom_state.clone(),
                window_size.clone(),
                mode,
            ),
            move |(_, state, sid, pm_queue, cam, zoom, window_size, mode)| {
                if let Some(renderer) = renderer_state.as_ref() {
                    let mut ghosts = state.pieces.clone();
                    for pm in pm_queue {
                        if let Some(p) = ghosts.get_mut(&pm.piece_id) {
                            p.position = pm.target;
                        }
                    }
                    renderer.draw_with_ghosts(
                        state,
                        player_id_val,
                        *sid,
                        pm_queue,
                        &ghosts,
                        **cam,
                        window_size.0,
                        window_size.1,
                        **zoom,
                        mode.as_ref(),
                    );
                }
                || ()
            },
        );
    }

    let (width, height) = *window_size;

    let player_id = props.reducer.player_id.unwrap_or_else(Uuid::nil);
    let player_score = props.reducer.state.players.get(&player_id).map(|p| p.score).unwrap_or(0);
    let player_pieces_count = props.reducer.state.pieces.values().filter(|p| p.owner_id == Some(player_id)).count();
    let player_pieces: Vec<_> = props.reducer.state.pieces.values().filter(|p| p.owner_id == Some(player_id)).collect();
    
    let shop_on_which_player_is = props.reducer.state.shops.iter().find(|s| {
        player_pieces.iter().any(|p| p.position == s.position)
    });

    let piece_on_shop = shop_on_which_player_is.and_then(|s| {
        player_pieces.iter().find(|p| p.position == s.position).cloned().cloned()
    });

    html! {
        <div class="fixed inset-0 bg-slate-100 overflow-hidden touch-none"
             onmousedown={
                 let handle_input_start = handle_input_start.clone();
                 Callback::from(move |e: MouseEvent| {
                     handle_input_start.emit((e.client_x() as f64, e.client_y() as f64, e.button() == 2));
                 })
             }
             onmousemove={
                 let handle_input_move = handle_input_move.clone();
                 Callback::from(move |e: MouseEvent| {
                     handle_input_move.emit((e.client_x() as f64, e.client_y() as f64));
                 })
             }
             onmouseup={
                 let handle_input_end = handle_input_end.clone();
                 Callback::from(move |e: MouseEvent| {
                     handle_input_end.emit((e.client_x() as f64, e.client_y() as f64, e.button() == 2));
                 })
             }
             oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}
             ontouchstart={
                 let handle_input_start = handle_input_start.clone();
                 Callback::from(move |e: TouchEvent| {
                     if let Some(touch) = e.touches().get(0) {
                         handle_input_start.emit((touch.client_x() as f64, touch.client_y() as f64, false));
                     }
                 })
             }
             ontouchmove={
                 let handle_input_move = handle_input_move.clone();
                 Callback::from(move |e: TouchEvent| {
                     if let Some(touch) = e.touches().get(0) {
                         handle_input_move.emit((touch.client_x() as f64, touch.client_y() as f64));
                     }
                 })
             }
             ontouchend={
                 let handle_input_end = handle_input_end.clone();
                 Callback::from(move |e: TouchEvent| {
                     if let Some(touch) = e.changed_touches().get(0) {
                         handle_input_end.emit((touch.client_x() as f64, touch.client_y() as f64, false));
                     }
                 })
             }
        >
            <canvas
                ref={canvas_ref}
                width={width.to_string()}
                height={height.to_string()}
                class="block w-full h-full"
            />

            <div class="absolute top-4 left-4 flex flex-col gap-2 pointer-events-none">
                <div class="bg-white/90 backdrop-blur px-3 py-1.5 rounded-lg shadow-sm border border-slate-200">
                    <span class="text-xs font-bold text-slate-500 uppercase tracking-wider block">{"Score"}</span>
                    <span class="text-xl font-black text-slate-800 tabular-nums">{player_score}</span>
                </div>
            </div>

            <crate::components::leaderboard::Leaderboard 
                players={props.reducer.state.players.values().cloned().collect::<Vec<_>>()} 
                self_id={player_id} 
            />

            if let Some(shop) = shop_on_which_player_is {
                if let Some(shop_config) = props.reducer.shop_configs.get(&shop.shop_id) {
                    <crate::components::shop_ui::ShopUI 
                        player_score={player_score}
                        player_pieces_count={player_pieces_count}
                        piece_on_shop={piece_on_shop}
                        shop_config={shop_config.clone()}
                        piece_configs={props.reducer.piece_configs.clone()}
                        tx={props.tx.clone()}
                        shop_pos={shop.position}
                    />
                }
            }

            if let Some(error) = props.reducer.error.clone() {
                <crate::components::error_toast::ErrorToast error={error} />
            }

            <crate::components::fatal_notification::FatalNotification 
                show={props.reducer.fatal_error} 
                title={props.reducer.disconnected_title.clone()}
                msg={props.reducer.disconnected_msg.clone()}
            />

            if props.reducer.disconnected {
                <crate::components::disconnected_screen::DisconnectedScreen 
                    show={true}
                    disconnected={props.reducer.disconnected}
                    title={props.reducer.disconnected_title.clone()}
                    msg={props.reducer.disconnected_msg.clone()}
                />
            }
        </div>
    }
}
