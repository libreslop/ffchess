use super::helpers::{MOVE_ANIM_MS, PieceAnim, apply_visible_ghosts, pm_visible};
use crate::camera::{CameraManager, update_camera};
use crate::canvas::Renderer;
use crate::reducer::{GameAction, GameStateReducer, MsgSender, Pmove};
use common::logic::is_within_board;
use common::types::{DurationMs, PieceId, PlayerId, Score, TimestampMs};
use glam::IVec2;
use gloo_events::EventListener;
use gloo_render::{AnimationFrame, request_animation_frame};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Properties for the main game viewport.
#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
    pub render_interval_ms: u32,
    pub globals: crate::app::GlobalClientConfig,
}

#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let canvas_ref = use_node_ref();
    let selected_piece_id = use_state(|| None::<PieceId>);
    let manager_ref = use_mut_ref(CameraManager::new);
    let piece_prev_positions = use_mut_ref(HashMap::<PieceId, IVec2>::new);
    let piece_anims = use_mut_ref(HashMap::<PieceId, PieceAnim>::new);

    let cam_state = use_state(|| (0.0, 0.0));
    let zoom_state = use_state(|| 1.0f64);
    let frame_id = use_state(|| 0u64);
    let drag_start = use_state(|| None::<(f64, f64, bool)>);
    let renderer_state = use_state(|| None::<Renderer>);
    let fps_counter = use_mut_ref(|| {
        (
            0u32,
            web_sys::window().unwrap().performance().unwrap().now(),
        )
    });

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

    // Drive a steady render heartbeat with requestAnimationFrame so visual elements (e.g., cooldown bars) update every frame
    {
        let frame_id = frame_id.clone();
        use_effect_with((), move |_| {
            let handle_cell: Rc<RefCell<Option<AnimationFrame>>> = Rc::new(RefCell::new(None));
            let start_cell = handle_cell.clone();
            let start_frame = frame_id.clone();

            fn schedule(cell: Rc<RefCell<Option<AnimationFrame>>>, frame: UseStateHandle<u64>) {
                let inner_cell = cell.clone();
                let inner_frame = frame.clone();
                let handle = request_animation_frame(move |ms| {
                    inner_frame.set(ms as u64);
                    schedule(inner_cell, inner_frame);
                });
                *cell.borrow_mut() = Some(handle);
            }

            schedule(start_cell, start_frame);

            move || {
                if let Some(h) = handle_cell.borrow_mut().take() {
                    drop(h);
                }
            }
        });
    }

    // Initialize renderer when canvas is bound
    {
        let renderer_state = renderer_state.clone();
        let piece_configs = props.reducer.piece_configs.clone();
        let shop_configs = props.reducer.shop_configs.clone();
        use_effect_with(
            (canvas_ref.clone(), piece_configs, shop_configs),
            move |(canvas_ref, piece_configs, shop_configs)| {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    renderer_state.set(Some(Renderer::new(
                        canvas,
                        piece_configs.clone(),
                        shop_configs.clone(),
                    )));
                }
                || ()
            },
        );
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
        let manager_ref = manager_ref.clone();
        let latest_state = latest_state.clone();
        let fps_counter = fps_counter.clone();
        let reducer = props.reducer.clone();
        let globals = props.globals.clone();

        use_effect_with(props.render_interval_ms, move |&render_ms| {
            let render_ms = render_ms.max(8);
            let interval = gloo_timers::callback::Interval::new(render_ms, move || {
                let (reducer_state, is_dragging) = {
                    let s = latest_state.borrow();
                    (s.0.clone(), s.1)
                };

                {
                    let mut fc = fps_counter.borrow_mut();
                    fc.0 += 1;
                    let now = web_sys::window().unwrap().performance().unwrap().now();
                    if now - fc.1 >= 1000.0 {
                        let fps = ((fc.0 as f64) * 1000.0 / (now - fc.1)).round() as u32;
                        reducer.dispatch(GameAction::SetFPS(fps));
                        fc.0 = 0;
                        fc.1 = now;
                    }
                }

                {
                    let mut manager = manager_ref.borrow_mut();

                    let player_id_val = reducer_state.player_id.unwrap_or_else(PlayerId::nil);
                    let piece_count = reducer_state
                        .state
                        .pieces
                        .values()
                        .filter(|p| p.owner_id == Some(player_id_val))
                        .count();

                    let changed = update_camera(
                        &mut manager,
                        crate::camera::CameraUpdateParams {
                            state: &reducer_state.state,
                            player_id: reducer_state.player_id,
                            canvas_ref: &canvas_ref,
                            is_dragging,
                            mode: reducer_state.mode.as_ref(),
                            piece_count,
                            is_dead: reducer_state.is_dead,
                            zoom_min: globals.camera_zoom_min,
                            zoom_max: globals.camera_zoom_max,
                            zoom_lerp: globals.zoom_lerp,
                            inertia_decay: globals.inertia_decay,
                            velocity_cutoff: globals.velocity_cutoff,
                            pan_lerp_alive: globals.pan_lerp_alive,
                            pan_lerp_dead: globals.pan_lerp_dead,
                            tile_size_px: globals.tile_size_px,
                            death_zoom: globals.death_zoom,
                        },
                    );

                    if changed {
                        zoom_state.set(manager.zoom);
                        cam_state.set(manager.camera);
                    }
                }
            });
            move || drop(interval)
        });
    }

    {
        let manager_ref = manager_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let is_dead = props.reducer.is_dead;
        let globals = props.globals.clone();
        use_effect_with(
            (canvas_ref.clone(), is_dead),
            move |(canvas_ref, is_dead)| {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                    let manager_ref = manager_ref.clone();
                    let is_dead = *is_dead;
                    let zoom_min = globals.camera_zoom_min;
                    let zoom_max = globals.camera_zoom_max;
                    let scroll_base = globals.scroll_zoom_base;
                    let listener = EventListener::new(&canvas, "wheel", move |e| {
                        if is_dead {
                            return;
                        }
                        let e = e.dyn_ref::<web_sys::WheelEvent>().unwrap();
                        e.prevent_default();
                        let delta = e.delta_y();
                        let factor = scroll_base.max(1.01).powf(-delta / 100.0);
                        let mut manager = manager_ref.borrow_mut();
                        if manager.input_locked {
                            return;
                        }
                        manager.mouse_pos = (e.client_x() as f64, e.client_y() as f64);
                        manager.target_zoom =
                            (manager.target_zoom * factor).clamp(zoom_min, zoom_max);
                    });
                    return Box::new(move || drop(listener)) as Box<dyn FnOnce()>;
                }
                Box::new(|| ()) as Box<dyn FnOnce()>
            },
        );
    }

    // Render canvas every frame_id tick
    {
        let renderer_state = renderer_state.clone();
        let reducer = props.reducer.clone();
        let shop_configs = props.reducer.shop_configs.clone();
        let globals = props.globals.clone();
        let piece_anims = piece_anims.clone();
        use_effect_with(
            (
                *frame_id,
                cam_state.clone(),
                zoom_state.clone(),
                window_size.clone(),
                reducer.state.clone(),
                reducer.pm_queue.clone(),
                reducer.mode.clone(),
                reducer.player_id,
                *selected_piece_id,
                shop_configs,
                globals.clone(),
            ),
            move |(
                _,
                cam,
                zoom,
                window_size,
                state,
                pm_queue,
                mode,
                player_id,
                sid,
                shop_configs,
                globals,
            )| {
                if let Some(renderer) = renderer_state.as_ref() {
                    let mut ghosts = state.pieces.clone();
                    apply_visible_ghosts(&mut ghosts, pm_queue, state);
                    let visible_pm: Vec<_> = pm_queue
                        .iter()
                        .filter(|pm| pm_visible(pm, state))
                        .cloned()
                        .collect();

                    let now = web_sys::window().unwrap().performance().unwrap().now();
                    let mut anims = piece_anims.borrow_mut();
                    let mut animated_positions = HashMap::new();
                    anims.retain(|id, anim| {
                        let Some(_) = state.pieces.get(id) else {
                            return false;
                        };

                        let progress = ((now - anim.started_at) / MOVE_ANIM_MS).clamp(0.0, 1.0);
                        if progress < 1.0 {
                            let x =
                                anim.start.x as f64 + (anim.end.x - anim.start.x) as f64 * progress;
                            let y =
                                anim.start.y as f64 + (anim.end.y - anim.start.y) as f64 * progress;
                            animated_positions.insert(*id, (x, y));
                            true
                        } else {
                            false
                        }
                    });
                    renderer.draw_with_ghosts(crate::canvas::RenderParams {
                        state,
                        player_id: player_id.unwrap_or_else(PlayerId::nil),
                        selected_piece_id: *sid,
                        pm_queue: &visible_pm,
                        ghost_pieces: &ghosts,
                        animated_positions: &animated_positions,
                        camera_pos: **cam,
                        width: window_size.0,
                        height: window_size.1,
                        zoom: **zoom,
                        tile_size_px: globals.tile_size_px,
                        mode: mode.as_ref(),
                        shop_configs,
                    });
                }
                || ()
            },
        );
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

    {
        let selected_piece_id = selected_piece_id.clone();
        let player_id = props.reducer.player_id;
        use_effect_with(player_id, move |_| {
            selected_piece_id.set(None);
            || ()
        });
    }

    {
        let selected_piece_id = selected_piece_id.clone();
        let player_id = props.reducer.player_id.unwrap_or_else(PlayerId::nil);
        let pieces = props.reducer.state.pieces.clone();
        use_effect_with((pieces, player_id), move |(pieces, player_id)| {
            if let Some(sel) = *selected_piece_id {
                match pieces.get(&sel) {
                    Some(p) if p.owner_id == Some(*player_id) => {}
                    _ => selected_piece_id.set(None),
                }
            }
            || ()
        });
    }

    {
        let piece_prev_positions = piece_prev_positions.clone();
        let piece_anims = piece_anims.clone();
        let pieces = props.reducer.state.pieces.clone();
        use_effect_with(pieces, move |pieces| {
            let now = web_sys::window().unwrap().performance().unwrap().now();
            let mut prev = piece_prev_positions.borrow_mut();
            let mut anims = piece_anims.borrow_mut();

            anims.retain(|id, _| pieces.contains_key(id));

            for (id, piece) in pieces.iter() {
                if let Some(old_pos) = prev.get(id)
                    && old_pos != &piece.position
                {
                    anims.insert(
                        *id,
                        PieceAnim {
                            start: *old_pos,
                            end: piece.position,
                            started_at: now,
                        },
                    );
                }
            }

            prev.clear();
            prev.extend(pieces.iter().map(|(id, p)| (*id, p.position)));
            || ()
        });
    }

    let handle_input_start = {
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
        let tile_size_px = props.globals.tile_size_px;
        Callback::from(move |(cx, cy, is_right_click): (f64, f64, bool)| {
            if reducer.is_dead {
                return;
            }
            let mut manager = manager_ref.borrow_mut();
            if manager.input_locked {
                return;
            }
            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let zoom = manager.zoom;
            let tile_size = tile_size_px * zoom;
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
                apply_visible_ghosts(&mut ghosts, &reducer.pm_queue, &reducer.state);

                if ghosts.values().any(|p| p.position == target) {
                    is_interactive = true;
                } else if let Some(sid) = *selected_piece_id
                    && let Some(piece) = ghosts.get(&sid)
                    && let Some(config) = reducer.piece_configs.get(&piece.piece_type)
                    && common::logic::is_valid_move(common::logic::MoveValidationParams {
                        piece_config: config,
                        start: piece.position,
                        end: target,
                        is_capture: ghosts.values().any(|p| p.position == target),
                        board_size,
                        pieces: &ghosts,
                        moving_owner: piece.owner_id,
                    })
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
            if reducer.is_dead {
                return;
            }
            let mut manager = manager_ref.borrow_mut();
            if manager.input_locked {
                return;
            }
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

                    let player_id_val = reducer.player_id.unwrap_or_else(PlayerId::nil);
                    let is_alive = reducer.state.players.contains_key(&player_id_val)
                        && player_id_val != PlayerId::nil();

                    if !is_alive {
                        manager.target_camera = manager.camera;
                    }
                    cam_state.set(manager.camera);
                    manager.velocity = (-dx, -dy);
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
        let tile_size_px = props.globals.tile_size_px;

        Callback::from(move |(cx, cy, is_right_click): (f64, f64, bool)| {
            if reducer.is_dead {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = (0.0, 0.0);
                return;
            }
            if manager_ref.borrow().input_locked {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = (0.0, 0.0);
                return;
            }
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
            let tile_size = tile_size_px * zoom;
            let x = cx - rect.left();
            let y = cy - rect.top();

            let world_x = x + manager.camera.0 - (canvas.width() as f64 / 2.0);
            let world_y = y + manager.camera.1 - (canvas.height() as f64 / 2.0);

            let grid_x = (world_x / tile_size).floor() as i32;
            let grid_y = (world_y / tile_size).floor() as i32;
            let target = IVec2::new(grid_x, grid_y);
            let player_id = reducer.player_id.unwrap_or_else(PlayerId::nil);

            if is_right_click {
                selected_piece_id.set(None);
                reducer.dispatch(GameAction::ClearPmQueue(PieceId::nil()));
                return;
            }

            let mut current_ghosts = reducer.state.pieces.clone();
            apply_visible_ghosts(&mut current_ghosts, &reducer.pm_queue, &reducer.state);

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

                        if let Some(config) = reducer.piece_configs.get(&p.piece_type)
                            && common::logic::is_valid_move(common::logic::MoveValidationParams {
                                piece_config: config,
                                start: p.position,
                                end: target,
                                is_capture,
                                board_size: reducer.state.board_size,
                                pieces: &current_ghosts,
                                moving_owner: p.owner_id,
                            })
                        {
                            reducer.dispatch(GameAction::AddPmove(Pmove {
                                piece_id: sid,
                                target,
                                pending: false,
                                old_last_move_time: TimestampMs::from_millis(0),
                                old_cooldown_ms: DurationMs::zero(),
                            }));
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

    // Cancel panning when cursor leaves the screen
    let handle_mouse_leave = {
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        Callback::from(move |_| {
            let prev = *drag_start;
            drag_start.set(None);
            if let Some((_, _, allow_panning)) = prev
                && allow_panning
            {
                // Treat like a mouse release: keep current velocity for inertia but lock target to current position
                let mut mgr = manager_ref.borrow_mut();
                mgr.target_camera = mgr.camera;
            }
        })
    };

    // Stop ongoing pan/zoom inputs immediately after death so death focus can take over
    {
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        use_effect_with(props.reducer.is_dead, move |is_dead| {
            if *is_dead {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = (0.0, 0.0);
            }
            || ()
        });
    }

    let (width, height) = *window_size;

    let player_id = props.reducer.player_id.unwrap_or_else(PlayerId::nil);
    let player_score = props
        .reducer
        .state
        .players
        .get(&player_id)
        .map(|p| p.score)
        .unwrap_or_else(Score::zero);
    let player_pieces_count = props
        .reducer
        .state
        .pieces
        .values()
        .filter(|p| p.owner_id == Some(player_id))
        .count();
    let player_pieces: Vec<_> = props
        .reducer
        .state
        .pieces
        .values()
        .filter(|p| p.owner_id == Some(player_id))
        .collect();

    let mut active_shops = Vec::new();
    for shop in &props.reducer.state.shops {
        if let Some(p) = player_pieces.iter().find(|p| p.position == shop.position) {
            let tile_size = props.globals.tile_size_px;
            let px_x = p.position.x as f64 * tile_size + tile_size / 2.0;
            let px_y = p.position.y as f64 * tile_size + tile_size / 2.0;
            let dx = px_x - cam_state.0;
            let dy = px_y - cam_state.1;
            let dist_sq = dx * dx + dy * dy;
            active_shops.push((shop, (*p).clone(), dist_sq));
        }
    }
    active_shops.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let shop_on_which_player_is = active_shops.first().map(|(s, _, _)| *s);
    let piece_on_shop = active_shops.first().map(|(_, p, _)| p.clone());

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
             onmouseleave={handle_mouse_leave}
             oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}
            ontouchstart={
                let handle_input_start = handle_input_start.clone();
                let manager_ref = manager_ref.clone();
                let drag_start = drag_start.clone();
                let latest_state = latest_state.clone();
                let is_dead = props.reducer.is_dead;
                Callback::from(move |e: TouchEvent| {
                    e.prevent_default();
                    if is_dead {
                        return;
                    }
                    if e.touches().length() == 2 {
                        // Begin pinch zoom
                        let t0 = e.touches().get(0).unwrap();
                        let t1 = e.touches().get(1).unwrap();
                        let dx = t1.client_x() as f64 - t0.client_x() as f64;
                        let dy = t1.client_y() as f64 - t0.client_y() as f64;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let cx = (t0.client_x() as f64 + t1.client_x() as f64) / 2.0;
                        let cy = (t0.client_y() as f64 + t1.client_y() as f64) / 2.0;
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = Some(dist);
                        mgr.last_touch_center = Some((cx, cy));
                        mgr.velocity = (0.0, 0.0);
                        drop(mgr);
                        drag_start.set(None);
                        if let Ok(mut s) = latest_state.try_borrow_mut() {
                            s.1 = false;
                        }
                    } else if let Some(touch) = e.touches().get(0) {
                        handle_input_start.emit((touch.client_x() as f64, touch.client_y() as f64, false));
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = None;
                        mgr.last_touch_center = None;
                    }
                })
            }
            ontouchmove={
                let handle_input_move = handle_input_move.clone();
               let manager_ref = manager_ref.clone();
                let is_dead = props.reducer.is_dead;
                let cam_state = cam_state.clone();
                let zoom_state = zoom_state.clone();
                let latest_state = latest_state.clone();
                let zoom_min = props.globals.camera_zoom_min;
                let zoom_max = props.globals.camera_zoom_max;
                Callback::from(move |e: TouchEvent| {
                    e.prevent_default();
                    if is_dead {
                        return;
                    }
                    if e.touches().length() == 2 {
                        let t0 = e.touches().get(0).unwrap();
                        let t1 = e.touches().get(1).unwrap();
                        let dx = t1.client_x() as f64 - t0.client_x() as f64;
                        let dy = t1.client_y() as f64 - t0.client_y() as f64;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let mut mgr = manager_ref.borrow_mut();
                        if let Some(prev) = mgr.last_touch_dist {
                            let factor = (dist / prev).powf(0.8); // dampen sensitivity
                            let cx = (t0.client_x() as f64 + t1.client_x() as f64) / 2.0;
                            let cy = (t0.client_y() as f64 + t1.client_y() as f64) / 2.0;
                            mgr.mouse_pos = (cx, cy);
                            if let Some((pcx, pcy)) = mgr.last_touch_center {
                                let pan_dx = cx - pcx;
                                let pan_dy = cy - pcy;
                                mgr.camera.0 -= pan_dx;
                                mgr.camera.1 -= pan_dy;
                                mgr.target_camera = mgr.camera;
                            }
                            mgr.last_touch_center = Some((cx, cy));
                            mgr.target_zoom = (mgr.target_zoom * factor).clamp(zoom_min, zoom_max);
                            mgr.zoom = mgr.target_zoom; // apply immediately for smooth pinch
                            mgr.velocity = (0.0, 0.0);
                            mgr.last_touch_dist = Some(dist);
                            let new_cam = mgr.camera;
                            let new_zoom = mgr.zoom;
                            drop(mgr);
                            cam_state.set(new_cam);
                            zoom_state.set(new_zoom);
                        } else {
                            mgr.last_touch_dist = Some(dist);
                            drop(mgr);
                        }
                        if let Ok(mut s) = latest_state.try_borrow_mut() {
                            s.1 = false;
                        }
                    } else if let Some(touch) = e.touches().get(0) {
                        {
                            let mut mgr = manager_ref.borrow_mut();
                            mgr.last_touch_dist = None;
                            mgr.last_touch_center = None;
                        }
                        handle_input_move.emit((touch.client_x() as f64, touch.client_y() as f64));
                    }
                })
            }
            ontouchend={
                let handle_input_end = handle_input_end.clone();
                let manager_ref = manager_ref.clone();
                let drag_start = drag_start.clone();
                let latest_state = latest_state.clone();
                Callback::from(move |e: TouchEvent| {
                    e.prevent_default();
                    {
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = None;
                        mgr.last_touch_center = None;
                    }
                    drag_start.set(None);
                    if let Ok(mut s) = latest_state.try_borrow_mut() {
                        s.1 = false;
                    }
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
                    <span class="text-xl font-black text-slate-800 tabular-nums">{player_score.to_string()}</span>
                </div>
            </div>

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

            <crate::components::fatal_notification::FatalNotification
                show={props.reducer.fatal_error}
                title={props.reducer.disconnected_title.clone()}
                msg={props.reducer.disconnected_msg.clone()}
            />
        </div>
    }
}
