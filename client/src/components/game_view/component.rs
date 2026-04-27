//! Main in-game canvas view and input handling.

use super::geometry::{
    is_ui_exempt_target, local_board_rotated_180, read_window_size, screen_to_grid,
};
use super::helpers::{MOVE_ANIM_MS, apply_visible_ghosts, pm_visible};
use super::types::{
    DragStart, FpsCounter, InputEnd, InputMove, InputStart, LastTap, LatestStateSnapshot, PieceAnim,
};
use crate::camera::{CameraManager, update_camera};
use crate::canvas::Renderer;
use crate::math::{Vec2, vec2};
use crate::reducer::{GameAction, GameStateReducer, MsgSender, Pmove};
use crate::utils::request_fullscreen;
use common::logic::is_within_board;
use common::protocol::ClientMessage;
use common::types::{BoardCoord, PieceId, PlayerId, Score};
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
/// Renders the main game view, canvas, and interaction handlers.
///
/// `props` provides reducer state, message sender, and render config. Returns rendered HTML.
pub fn game_view(props: &GameViewProps) -> Html {
    let canvas_ref = use_node_ref();
    let selected_piece_id = use_state(|| None::<PieceId>);
    let next_pm_id = use_mut_ref(|| 1u64);
    let manager_ref = use_mut_ref(CameraManager::new);
    let piece_prev_positions = use_mut_ref(HashMap::<PieceId, BoardCoord>::new);
    let piece_anims = use_mut_ref(HashMap::<PieceId, PieceAnim>::new);
    let last_tap = use_mut_ref(|| None::<LastTap>);
    let touch_gesture_active = use_mut_ref(|| false);

    let cam_state = use_state(|| Vec2::ZERO);
    let zoom_state = use_state(|| 1.0f64);
    let frame_id = use_state(|| 0u64);
    let drag_start = use_state(|| None::<DragStart>);
    let did_pan = use_mut_ref(|| false);
    let renderer_state = use_state(|| None::<Renderer>);
    let fps_counter = use_mut_ref(FpsCounter::new);

    let window_size = use_state(read_window_size);
    let has_match_result = props.reducer.is_dead || props.reducer.is_victory;

    // Drive a steady render heartbeat with requestAnimationFrame so visual elements (e.g., cooldown bars) update every frame
    {
        let frame_id = frame_id.clone();
        use_effect_with((), move |_| {
            let handle_cell: Rc<RefCell<Option<AnimationFrame>>> = Rc::new(RefCell::new(None));
            let start_cell = handle_cell.clone();
            let start_frame = frame_id.clone();

            /// Schedules the next animation frame and updates the frame counter.
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
        use_effect_with(
            (canvas_ref.clone(), piece_configs),
            move |(canvas_ref, piece_configs)| {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    renderer_state.set(Some(Renderer::new(canvas, piece_configs.clone())));
                }
                || ()
            },
        );
    }

    // We use a ref to track the latest state for the interval to avoid stale captures
    let latest_state =
        use_mut_ref(|| LatestStateSnapshot::new(props.reducer.clone(), (*drag_start).is_some()));
    {
        let mut s = latest_state.borrow_mut();
        s.update(props.reducer.clone(), (*drag_start).is_some());
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
                    (s.reducer.clone(), s.is_dragging)
                };

                {
                    let mut fc = fps_counter.borrow_mut();
                    fc.frames += 1;
                    let now = web_sys::window().unwrap().performance().unwrap().now();
                    if now - fc.last_ms >= 1000.0 {
                        let fps = ((fc.frames as f64) * 1000.0 / (now - fc.last_ms)).round() as u32;
                        reducer.dispatch(GameAction::SetFPS(fps));
                        fc.frames = 0;
                        fc.last_ms = now;
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
                            phase: reducer_state.phase,
                            is_victory: reducer_state.is_victory,
                            victory_focus_target: reducer_state.victory_focus_target,
                            zoom_min: globals.camera_zoom_min,
                            zoom_max: globals.camera_zoom_max,
                            zoom_lerp: globals.zoom_lerp,
                            inertia_decay: globals.inertia_decay,
                            velocity_cutoff: globals.velocity_cutoff,
                            pan_lerp_alive: globals.pan_lerp_alive,
                            pan_lerp_dead: globals.pan_lerp_dead,
                            tile_size_px: globals.tile_size_px,
                            death_zoom: globals.death_zoom,
                            board_rotated_180: local_board_rotated_180(
                                &reducer_state.state,
                                reducer_state.player_id,
                            ),
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
        let input_blocked = has_match_result;
        let globals = props.globals.clone();
        use_effect_with(
            (canvas_ref.clone(), input_blocked),
            move |(canvas_ref, input_blocked)| {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                    let manager_ref = manager_ref.clone();
                    let input_blocked = *input_blocked;
                    let zoom_min = globals.camera_zoom_min;
                    let zoom_max = globals.camera_zoom_max;
                    let scroll_base = globals.scroll_zoom_base;
                    let listener = EventListener::new(&canvas, "wheel", move |e| {
                        if input_blocked {
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
                        manager.mouse_pos = vec2(e.client_x() as f64, e.client_y() as f64);
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
                has_match_result,
                shop_configs,
                reducer.clock_offset_ms,
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
                has_match_result,
                shop_configs,
                clock_offset_ms,
            )| {
                if let Some(renderer) = renderer_state.as_ref() {
                    let selected_piece_id = if *has_match_result { None } else { *sid };
                    let mut ghosts = state.pieces.clone();
                    apply_visible_ghosts(&mut ghosts, pm_queue, state, shop_configs);
                    let visible_pm: Vec<_> = pm_queue
                        .iter()
                        .filter(|pm| pm_visible(pm, state))
                        .cloned()
                        .collect();
                    let active_shop_highlight_pos = {
                        let player_id = player_id.unwrap_or_else(PlayerId::nil);
                        let tile_size = globals.tile_size_px;
                        state
                            .shops
                            .iter()
                            .filter_map(|shop| {
                                ghosts
                                    .values()
                                    .find(|p| {
                                        p.position == shop.position && p.owner_id == Some(player_id)
                                    })
                                    .map(|p| {
                                        let piece_pos = vec2(
                                            p.position.0.x as f64 * tile_size + tile_size / 2.0,
                                            p.position.0.y as f64 * tile_size + tile_size / 2.0,
                                        );
                                        let dist_sq = (piece_pos - **cam).length_squared();
                                        (shop.position, dist_sq)
                                    })
                            })
                            .min_by(|a, b| {
                                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(pos, _)| pos)
                    };

                    let now = web_sys::window().unwrap().performance().unwrap().now();
                    let mut anims = piece_anims.borrow_mut();
                    let mut animated_positions = HashMap::new();
                    anims.retain(|id, anim| {
                        let Some(_) = state.pieces.get(id) else {
                            return false;
                        };

                        let progress = ((now - anim.started_at) / MOVE_ANIM_MS).clamp(0.0, 1.0);
                        if progress < 1.0 {
                            let x = anim.start.0.x as f64
                                + (anim.end.0.x - anim.start.0.x) as f64 * progress;
                            let y = anim.start.0.y as f64
                                + (anim.end.0.y - anim.start.0.y) as f64 * progress;
                            animated_positions.insert(*id, vec2(x, y));
                            true
                        } else {
                            false
                        }
                    });
                    renderer.draw_with_ghosts(crate::canvas::RenderParams {
                        state,
                        player_id: player_id.unwrap_or_else(PlayerId::nil),
                        selected_piece_id,
                        pm_queue: &visible_pm,
                        ghost_pieces: &ghosts,
                        animated_positions: &animated_positions,
                        camera_pos: **cam,
                        canvas_size: **window_size,
                        zoom: **zoom,
                        tile_size_px: globals.tile_size_px,
                        mode: mode.as_ref(),
                        board_rotated_180: local_board_rotated_180(state, *player_id),
                        shop_configs,
                        active_shop_highlight_pos,
                        disable_fog_of_war: *has_match_result,
                        clock_offset_ms: *clock_offset_ms,
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
                window_size.set(read_window_size());
            });
            || drop(listener)
        });
    }

    {
        let selected_piece_id = selected_piece_id.clone();
        let reset_selection = (props.reducer.player_id, has_match_result);
        use_effect_with(reset_selection, move |_| {
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

    {
        let reducer = props.reducer.clone();
        let tx = props.tx.clone();
        use_effect_with(
            (
                reducer.pm_queue.clone(),
                reducer.state.pieces.clone(),
                reducer.state.shops.clone(),
                reducer.shop_configs.clone(),
                reducer.player_id,
            ),
            move |(pm_queue, pieces, shops, shop_configs, player_id)| {
                let player_id = player_id.unwrap_or_else(PlayerId::nil);
                if let Some(pm) = pm_queue
                    .iter()
                    .find(|pm| pm.shop_item_index.is_some())
                    .cloned()
                    && let Some(piece) = pieces.get(&pm.piece_id)
                    && piece.owner_id == Some(player_id)
                    && piece.position == BoardCoord(pm.target)
                    && let Some(shop) = shops.iter().find(|s| s.position == piece.position)
                    && let Some(shop_config) = shop_configs.get(&shop.shop_id)
                    && let Some(group) = common::logic::select_shop_group(shop_config, Some(piece))
                {
                    let item_index = pm.shop_item_index.unwrap_or_default();
                    if item_index >= group.items.len() {
                        reducer.dispatch(GameAction::RemovePm(pm.id));
                    } else {
                        let _ = tx.0.try_send(ClientMessage::BuyPiece {
                            shop_pos: piece.position,
                            item_index,
                        });
                        reducer.dispatch(GameAction::RemovePm(pm.id));
                    }
                }
                || ()
            },
        );
    }

    let handle_input_start = {
        let selected_piece_id = selected_piece_id.clone();
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        let did_pan = did_pan.clone();
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
        let tile_size_px = props.globals.tile_size_px;
        Callback::from(move |input: InputStart| {
            if reducer.is_dead || reducer.is_victory {
                return;
            }
            let mut manager = manager_ref.borrow_mut();
            if manager.input_locked {
                return;
            }
            *did_pan.borrow_mut() = false;
            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let zoom = manager.zoom;
            let tile_size = tile_size_px * zoom;
            let target = screen_to_grid(
                input.pos,
                &rect,
                &canvas,
                manager.camera,
                tile_size,
                local_board_rotated_180(&reducer.state, reducer.player_id),
            );

            let board_size = reducer.state.board_size;
            let mut is_interactive = false;

            if !input.is_right_click && is_within_board(BoardCoord(target), board_size) {
                let mut ghosts = reducer.state.pieces.clone();
                apply_visible_ghosts(
                    &mut ghosts,
                    &reducer.pm_queue,
                    &reducer.state,
                    &reducer.shop_configs,
                );

                if ghosts.values().any(|p| p.position == target) {
                    is_interactive = true;
                } else if let Some(sid) = *selected_piece_id
                    && let Some(piece) = ghosts.get(&sid)
                    && let Some(config) = reducer.piece_configs.get(&piece.piece_type)
                    && common::logic::is_valid_move(common::logic::MoveValidationParams {
                        piece_config: config,
                        start: piece.position,
                        end: BoardCoord(target),
                        is_capture: ghosts.values().any(|p| p.position == target),
                        board_size,
                        pieces: &ghosts,
                        moving_owner: piece.owner_id,
                    })
                {
                    is_interactive = true;
                }
            }
            drag_start.set(Some(DragStart {
                pos: input.pos,
                allow_panning: !is_interactive,
            }));
            manager.velocity = Vec2::ZERO;
        })
    };

    let handle_input_move = {
        let drag_start = drag_start.clone();
        let cam_state = cam_state.clone();
        let manager_ref = manager_ref.clone();
        let did_pan = did_pan.clone();
        let reducer = props.reducer.clone();
        Callback::from(move |input: InputMove| {
            if reducer.is_dead || reducer.is_victory {
                return;
            }
            let mut manager = manager_ref.borrow_mut();
            if manager.input_locked {
                return;
            }
            manager.mouse_pos = input.pos;
            if let Some(start) = *drag_start {
                if !start.allow_panning {
                    return;
                }
                let delta = input.pos - start.pos;
                if delta.x.abs().max(delta.y.abs()) > 0.1 {
                    *did_pan.borrow_mut() = true;
                    let board_rotated_180 =
                        local_board_rotated_180(&reducer.state, reducer.player_id);
                    if board_rotated_180 {
                        manager.camera += delta;
                        manager.velocity = delta;
                    } else {
                        manager.camera -= delta;
                        manager.velocity = -delta;
                    }

                    let player_id_val = reducer.player_id.unwrap_or_else(PlayerId::nil);
                    let is_alive = reducer.state.players.contains_key(&player_id_val)
                        && player_id_val != PlayerId::nil();

                    if !is_alive {
                        manager.target_camera = manager.camera;
                    }
                    cam_state.set(manager.camera);
                    drag_start.set(Some(DragStart {
                        pos: input.pos,
                        allow_panning: true,
                    }));
                }
            }
        })
    };

    let handle_input_end = {
        let canvas_ref = canvas_ref.clone();
        let reducer = props.reducer.clone();
        let tx = props.tx.clone();
        let selected_piece_id = selected_piece_id.clone();
        let next_pm_id = next_pm_id.clone();
        let manager_ref = manager_ref.clone();
        let drag_start = drag_start.clone();
        let did_pan = did_pan.clone();
        let tile_size_px = props.globals.tile_size_px;

        Callback::from(move |input: InputEnd| {
            if reducer.is_dead || reducer.is_victory {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = Vec2::ZERO;
                *did_pan.borrow_mut() = false;
                return;
            }
            if manager_ref.borrow().input_locked {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = Vec2::ZERO;
                *did_pan.borrow_mut() = false;
                return;
            }
            let start = *drag_start;
            drag_start.set(None);

            let mut is_tap = true;
            if let Some(start) = start {
                let delta = input.pos - start.pos;
                let dist = delta.length();
                if *did_pan.borrow() {
                    is_tap = false;
                }
                if start.allow_panning && dist > 10.0 {
                    is_tap = false;
                }
                if !start.allow_panning {
                    manager_ref.borrow_mut().velocity = Vec2::ZERO;
                }
            } else {
                manager_ref.borrow_mut().velocity = Vec2::ZERO;
            }
            *did_pan.borrow_mut() = false;

            if !is_tap {
                return;
            }

            let canvas = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let rect = canvas.get_bounding_client_rect();
            let manager = manager_ref.borrow_mut();
            let zoom = manager.zoom;
            let tile_size = tile_size_px * zoom;
            let target = screen_to_grid(
                input.pos,
                &rect,
                &canvas,
                manager.camera,
                tile_size,
                local_board_rotated_180(&reducer.state, reducer.player_id),
            );
            let player_id = reducer.player_id.unwrap_or_else(PlayerId::nil);

            if input.is_right_click {
                selected_piece_id.set(None);
                return;
            }

            let mut current_ghosts = reducer.state.pieces.clone();
            apply_visible_ghosts(
                &mut current_ghosts,
                &reducer.pm_queue,
                &reducer.state,
                &reducer.shop_configs,
            );
            let selected_id = *selected_piece_id;

            // Check if clicking on an ACTUAL piece that has premoves (to clear them)
            let actual_piece_at_target = reducer.state.pieces.values().find(|p| {
                p.owner_id == Some(player_id)
                    && p.position == target
                    && reducer.pm_queue.iter().any(|pm| pm.piece_id == p.id)
            });

            if let Some(p) = actual_piece_at_target {
                let _ =
                    tx.0.try_send(ClientMessage::ClearPremoves { piece_id: p.id });
                reducer.dispatch(GameAction::ClearPm(p.id));
                if selected_id == Some(p.id) {
                    selected_piece_id.set(None);
                } else {
                    selected_piece_id.set(Some(p.id));
                }
                return;
            }

            let target_has_piece = current_ghosts.values().any(|p| p.position == target);
            let mut handled_action = false;

            if let Some(sid) = selected_id {
                let proj_p = current_ghosts.get(&sid);
                if let Some(p) = proj_p {
                    if target == p.position {
                        selected_piece_id.set(None);
                        handled_action = true;
                    } else if let Some(other) = current_ghosts
                        .values()
                        .find(|p| p.position == target && p.owner_id == Some(player_id))
                    {
                        selected_piece_id.set(Some(other.id));
                        handled_action = true;
                    } else {
                        let target_occupied =
                            current_ghosts.values().find(|gp| gp.position == target);
                        let is_capture = target_occupied.is_some()
                            && target_occupied.unwrap().owner_id != Some(player_id);

                        if let Some(config) = reducer.piece_configs.get(&p.piece_type)
                            && common::logic::is_valid_move(common::logic::MoveValidationParams {
                                piece_config: config,
                                start: p.position,
                                end: BoardCoord(target),
                                is_capture,
                                board_size: reducer.state.board_size,
                                pieces: &current_ghosts,
                                moving_owner: p.owner_id,
                            })
                        {
                            let _ = tx.0.try_send(ClientMessage::MovePiece {
                                piece_id: sid,
                                target: BoardCoord(target),
                            });
                            let pm_id = {
                                let mut next_id = next_pm_id.borrow_mut();
                                let id = *next_id;
                                *next_id += 1;
                                id
                            };
                            reducer.dispatch(GameAction::AddPmove(Pmove {
                                id: pm_id,
                                piece_id: sid,
                                target,
                                shop_item_index: None,
                            }));
                            handled_action = true;
                        }
                    }
                }
            } else {
                let piece = current_ghosts
                    .values()
                    .find(|p| p.position == target && p.owner_id == Some(player_id));
                if let Some(p) = piece {
                    selected_piece_id.set(Some(p.id));
                    handled_action = true;
                }
            }

            if !handled_action && !target_has_piece && selected_id.is_some() {
                selected_piece_id.set(None);
            }
        })
    };

    // Cancel panning when cursor leaves the screen
    let handle_mouse_leave = {
        let drag_start = drag_start.clone();
        let manager_ref = manager_ref.clone();
        let did_pan = did_pan.clone();
        Callback::from(move |_| {
            let prev = *drag_start;
            drag_start.set(None);
            *did_pan.borrow_mut() = false;
            if let Some(prev) = prev
                && prev.allow_panning
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
        let did_pan = did_pan.clone();
        use_effect_with(has_match_result, move |has_match_result| {
            if *has_match_result {
                drag_start.set(None);
                manager_ref.borrow_mut().velocity = Vec2::ZERO;
                *did_pan.borrow_mut() = false;
            }
            || ()
        });
    }

    let window_size = *window_size;
    let width = window_size.x;
    let height = window_size.y;

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
    let mut ui_ghosts = props.reducer.state.pieces.clone();
    apply_visible_ghosts(
        &mut ui_ghosts,
        &props.reducer.pm_queue,
        &props.reducer.state,
        &props.reducer.shop_configs,
    );

    let cam = *cam_state;
    let mut active_shops = Vec::new();
    for shop in &props.reducer.state.shops {
        if let Some(p) = ui_ghosts
            .values()
            .find(|p| p.position == shop.position && p.owner_id == Some(player_id))
        {
            let tile_size = props.globals.tile_size_px;
            let piece_pos = vec2(
                p.position.0.x as f64 * tile_size + tile_size / 2.0,
                p.position.0.y as f64 * tile_size + tile_size / 2.0,
            );
            let dist_sq = (piece_pos - cam).length_squared();
            active_shops.push((shop, p.clone(), dist_sq));
        }
    }
    active_shops.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let shop_menu_context = active_shops
        .first()
        .map(|(s, p, _)| (s.position, s.shop_id.clone(), p.clone()));
    let shop_on_which_player_is = shop_menu_context
        .as_ref()
        .map(|(pos, shop_id, _)| (*pos, shop_id.clone()));
    let piece_on_shop = shop_menu_context.as_ref().map(|(_, _, p)| p.clone());

    let on_shop_buy = {
        let reducer = props.reducer.clone();
        let next_pm_id = next_pm_id.clone();
        let piece_on_shop = piece_on_shop.clone();
        let shop_menu_context = shop_menu_context.clone();
        Callback::from(move |item_index: usize| {
            let Some(piece) = piece_on_shop.as_ref() else {
                return;
            };
            let Some((shop_pos, shop_id, _)) = shop_menu_context.as_ref() else {
                return;
            };
            let Some(shop_config) = reducer.shop_configs.get(shop_id) else {
                return;
            };
            if shop_config.auto_upgrade_single_item
                && let Some(group) = common::logic::select_shop_group(shop_config, Some(piece))
                && group.items.len() == 1
            {
                return;
            }
            let pm_id = {
                let mut next_id = next_pm_id.borrow_mut();
                let id = *next_id;
                *next_id += 1;
                id
            };
            reducer.dispatch(GameAction::AddPmove(Pmove {
                id: pm_id,
                piece_id: piece.id,
                target: shop_pos.0,
                shop_item_index: Some(item_index),
            }));
        })
    };

    html! {
        <div class="fixed inset-0 bg-slate-100 overflow-hidden touch-none"
             onmousedown={
                 let handle_input_start = handle_input_start.clone();
                 Callback::from(move |e: MouseEvent| {
                     if is_ui_exempt_target(e.target()) {
                         return;
                     }
                     handle_input_start.emit(InputStart {
                         pos: vec2(e.client_x() as f64, e.client_y() as f64),
                         is_right_click: e.button() == 2,
                     });
                 })
             }
             onmousemove={
                 let handle_input_move = handle_input_move.clone();
                 Callback::from(move |e: MouseEvent| {
                     if is_ui_exempt_target(e.target()) {
                         return;
                     }
                     handle_input_move.emit(InputMove {
                         pos: vec2(e.client_x() as f64, e.client_y() as f64),
                     });
                 })
             }
             onmouseup={
                 let handle_input_end = handle_input_end.clone();
                 Callback::from(move |e: MouseEvent| {
                     if is_ui_exempt_target(e.target()) {
                         return;
                     }
                     handle_input_end.emit(InputEnd {
                         pos: vec2(e.client_x() as f64, e.client_y() as f64),
                         is_right_click: e.button() == 2,
                     });
                 })
             }
             onmouseleave={handle_mouse_leave}
             oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}
            ontouchstart={
                let handle_input_start = handle_input_start.clone();
                let manager_ref = manager_ref.clone();
                let drag_start = drag_start.clone();
                let latest_state = latest_state.clone();
                let input_blocked = has_match_result;
                let touch_gesture_active = touch_gesture_active.clone();
                Callback::from(move |e: TouchEvent| {
                    if is_ui_exempt_target(e.target()) {
                        return;
                    }
                    e.prevent_default();
                    if input_blocked {
                        return;
                    }
                    if e.touches().length() == 2 {
                        *touch_gesture_active.borrow_mut() = true;
                        // Begin pinch zoom
                        let t0 = e.touches().get(0).unwrap();
                        let t1 = e.touches().get(1).unwrap();
                        let p0 = vec2(t0.client_x() as f64, t0.client_y() as f64);
                        let p1 = vec2(t1.client_x() as f64, t1.client_y() as f64);
                        let dist = (p1 - p0).length();
                        let center = (p0 + p1) * 0.5;
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = Some(dist);
                        mgr.last_touch_center = Some(center);
                        mgr.velocity = Vec2::ZERO;
                        drop(mgr);
                        drag_start.set(None);
                        if let Ok(mut s) = latest_state.try_borrow_mut() {
                            s.is_dragging = false;
                        }
                    } else if let Some(touch) = e.touches().get(0) {
                        *touch_gesture_active.borrow_mut() = false;
                        handle_input_start.emit(InputStart {
                            pos: vec2(touch.client_x() as f64, touch.client_y() as f64),
                            is_right_click: false,
                        });
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = None;
                        mgr.last_touch_center = None;
                    }
                })
            }
            ontouchmove={
                let handle_input_move = handle_input_move.clone();
                let manager_ref = manager_ref.clone();
                let canvas_ref = canvas_ref.clone();
                let input_blocked = has_match_result;
                let cam_state = cam_state.clone();
                let zoom_state = zoom_state.clone();
                let latest_state = latest_state.clone();
                let reducer = props.reducer.clone();
                let zoom_min = props.globals.camera_zoom_min;
                let zoom_max = props.globals.camera_zoom_max;
                let drag_start = drag_start.clone();
                let touch_gesture_active = touch_gesture_active.clone();
                Callback::from(move |e: TouchEvent| {
                    if is_ui_exempt_target(e.target()) {
                        return;
                    }
                    e.prevent_default();
                    if input_blocked {
                        return;
                    }
                    if e.touches().length() == 2 {
                        *touch_gesture_active.borrow_mut() = true;
                        let t0 = e.touches().get(0).unwrap();
                        let t1 = e.touches().get(1).unwrap();
                        let p0 = vec2(t0.client_x() as f64, t0.client_y() as f64);
                        let p1 = vec2(t1.client_x() as f64, t1.client_y() as f64);
                        let dist = (p1 - p0).length();
                        let center = (p0 + p1) * 0.5;
                        let mut mgr = manager_ref.borrow_mut();
                        if let Some(prev) = mgr.last_touch_dist {
                            let factor = (dist / prev).powf(0.8); // dampen sensitivity
                            mgr.mouse_pos = center;
                            if let Some(prev_center) = mgr.last_touch_center {
                                let pan = center - prev_center;
                                let board_rotated_180 = local_board_rotated_180(
                                    &reducer.state,
                                    reducer.player_id,
                                );
                                if board_rotated_180 {
                                    mgr.camera += pan;
                                } else {
                                    mgr.camera -= pan;
                                }
                                mgr.target_camera = mgr.camera;
                            }
                            mgr.last_touch_center = Some(center);
                            let old_zoom = mgr.zoom;
                            let new_zoom = (old_zoom * factor).clamp(zoom_min, zoom_max);
                            if (new_zoom - old_zoom).abs() > 0.000001
                                && let Some(canvas) =
                                    canvas_ref.cast::<web_sys::HtmlCanvasElement>()
                            {
                                let rect = canvas.get_bounding_client_rect();
                                let screen_pos = center - vec2(rect.left(), rect.top());
                                let canvas_center =
                                    vec2(canvas.width() as f64 / 2.0, canvas.height() as f64 / 2.0);
                                let mut mouse_delta = screen_pos - canvas_center;
                                if local_board_rotated_180(&reducer.state, reducer.player_id) {
                                    mouse_delta = -mouse_delta;
                                }
                                let ratio = new_zoom / old_zoom;
                                mgr.camera = (mgr.camera + mouse_delta) * ratio - mouse_delta;
                                mgr.target_camera = mgr.camera;
                            }
                            mgr.target_zoom = new_zoom;
                            mgr.zoom = new_zoom; // apply immediately for smooth pinch
                            mgr.velocity = Vec2::ZERO;
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
                            s.is_dragging = false;
                        }
                    } else if let Some(touch) = e.touches().get(0) {
                        let pos = vec2(touch.client_x() as f64, touch.client_y() as f64);
                        if let Some(start) = *drag_start {
                            let dist = (pos - start.pos).length();
                            if start.allow_panning && dist > 10.0 {
                                *touch_gesture_active.borrow_mut() = true;
                            }
                        }
                        {
                            let mut mgr = manager_ref.borrow_mut();
                            mgr.last_touch_dist = None;
                            mgr.last_touch_center = None;
                        }
                        handle_input_move.emit(InputMove { pos });
                    }
                })
            }
            ontouchend={
                let handle_input_end = handle_input_end.clone();
                let manager_ref = manager_ref.clone();
                let drag_start = drag_start.clone();
                let latest_state = latest_state.clone();
                let last_tap = last_tap.clone();
                let touch_gesture_active = touch_gesture_active.clone();
                Callback::from(move |e: TouchEvent| {
                    if is_ui_exempt_target(e.target()) {
                        return;
                    }
                    e.prevent_default();
                    {
                        let mut mgr = manager_ref.borrow_mut();
                        mgr.last_touch_dist = None;
                        mgr.last_touch_center = None;
                    }
                    drag_start.set(None);
                    if let Ok(mut s) = latest_state.try_borrow_mut() {
                        s.is_dragging = false;
                    }
                    {
                        let mut gesture = touch_gesture_active.borrow_mut();
                        if *gesture {
                            if e.touches().length() == 0 {
                                *gesture = false;
                            }
                            return;
                        }
                    }
                    if let Some(touch) = e.changed_touches().get(0) {
                        if e.touches().length() == 0 {
                            let now = web_sys::window()
                                .and_then(|w| w.performance())
                                .map(|p| p.now())
                                .unwrap_or(0.0);
                            let pos = vec2(touch.client_x() as f64, touch.client_y() as f64);
                            let mut last = last_tap.borrow_mut();
                            if let Some(prev) = *last {
                                let dt = now - prev.time_ms;
                                let delta = pos - prev.pos;
                                if dt <= 300.0 && delta.length_squared() <= 900.0 {
                                    *last = None;
                                    request_fullscreen();
                                    return;
                                }
                            }
                            *last = Some(LastTap {
                                time_ms: now,
                                pos,
                            });
                        }
                        handle_input_end.emit(InputEnd {
                            pos: vec2(touch.client_x() as f64, touch.client_y() as f64),
                            is_right_click: false,
                        });
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
                <div class="bg-white/90 backdrop-blur px-3 py-1.5 rounded-lg border border-slate-200"
                     style="box-shadow: 4px 4px 0 rgba(15, 23, 42, 0.2);">
                    <span class="text-xs font-bold text-slate-500 uppercase tracking-wider block">{"Score"}</span>
                    <span class="text-xl font-black text-slate-800 tabular-nums">{player_score.to_string()}</span>
                </div>
            </div>

            if let Some((_shop_pos, shop_id)) = shop_on_which_player_is.as_ref() {
                if let Some(shop_config) = props.reducer.shop_configs.get(shop_id) {
                    <crate::components::shop_ui::ShopUI
                        player_score={player_score}
                        player_pieces_count={player_pieces_count}
                        piece_on_shop={piece_on_shop}
                        shop_config={shop_config.clone()}
                        piece_configs={props.reducer.piece_configs.clone()}
                        on_buy={on_shop_buy.clone()}
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
