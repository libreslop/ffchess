//! Camera update logic for panning, zooming, and follow behavior.

use crate::math::Vec2;
use crate::reducer::ClientPhase;
use common::logic::evaluate_expression;
use common::protocol::VictoryFocusTarget;
use common::*;
use yew::prelude::*;

/// Mutable camera state for smooth panning and zoom transitions.
pub struct CameraManager {
    pub camera: Vec2,
    pub target_camera: Vec2,
    pub zoom: f64,
    pub target_zoom: f64,
    pub mouse_pos: Vec2,
    pub velocity: Vec2,
    pub input_locked: bool,
    pub was_alive: bool,
    pub last_king_grid_pos: glam::IVec2,
    pub last_touch_dist: Option<f64>,
    pub last_touch_center: Option<Vec2>,
}

impl CameraManager {
    /// Creates a camera manager with default pan/zoom state.
    pub fn new() -> Self {
        Self {
            camera: Vec2::ZERO,
            target_camera: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            mouse_pos: Vec2::ZERO,
            velocity: Vec2::ZERO,
            input_locked: false,
            was_alive: false,
            last_king_grid_pos: glam::IVec2::ZERO,
            last_touch_dist: None,
            last_touch_center: None,
        }
    }
}

/// Input parameters for a camera update tick.
pub struct CameraUpdateParams<'a> {
    pub state: &'a GameState,
    pub player_id: Option<PlayerId>,
    pub canvas_ref: &'a NodeRef,
    pub is_dragging: bool,
    pub mode: Option<&'a common::models::GameModeClientConfig>,
    pub piece_count: usize,
    pub phase: ClientPhase,
    pub is_victory: bool,
    pub victory_focus_target: VictoryFocusTarget,
    pub zoom_min: f64,
    pub zoom_max: f64,
    pub zoom_lerp: f64,
    pub inertia_decay: f64,
    pub velocity_cutoff: f64,
    pub pan_lerp_alive: f64,
    pub pan_lerp_dead: f64,
    pub tile_size_px: f64,
    pub death_zoom: f64,
}

/// Advances the camera state by one tick using inputs and game state.
///
/// `manager` is the mutable camera state, `params` contains context and settings.
/// Returns `true` if the camera state changed and requires a re-render.
pub fn update_camera(manager: &mut CameraManager, params: CameraUpdateParams<'_>) -> bool {
    let mut changed = false;
    let player_id_val = params.player_id.unwrap_or_else(PlayerId::nil);
    let player = params.state.players.get(&player_id_val);
    let is_alive = params.phase == ClientPhase::Alive;

    manager.target_zoom = manager.target_zoom.clamp(params.zoom_min, params.zoom_max);
    manager.zoom = manager.zoom.clamp(params.zoom_min, params.zoom_max);

    // 1. Zoom interpolation
    if (manager.target_zoom - manager.zoom).abs() > 0.000001 {
        let factor = params.zoom_lerp;
        let old_z = manager.zoom;
        manager.zoom += (manager.target_zoom - manager.zoom) * factor;
        let ratio = manager.zoom / old_z;

        if let Some(canvas) = params.canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
            let rect = canvas.get_bounding_client_rect();

            // Mouse position relative to canvas center
            // When dead, anchor zoom to canvas center so the death focus stays accurate.
            let mouse_delta = if params.phase == ClientPhase::Dead {
                Vec2::ZERO
            } else {
                Vec2::new(
                    manager.mouse_pos.x - rect.left() - (canvas.width() as f64 / 2.0),
                    manager.mouse_pos.y - rect.top() - (canvas.height() as f64 / 2.0),
                )
            };

            // To keep the point under the mouse fixed in world space:
            // The world position under the mouse is: W = (M - Offset) / Zoom
            // Where Offset = (CanvasWidth/2 - CameraPos)
            // So M - (CanvasWidth/2 - CameraPos) = Zoom * W
            // M - CanvasWidth/2 + CameraPos = Zoom * W
            // CameraPos = Zoom * W - (M - CanvasWidth/2)
            // When Zoom changes to Zoom', we want W to stay the same.
            // CameraPos' = Zoom' * W - (M - CanvasWidth/2)
            // CameraPos' = (Zoom' / Zoom) * (Zoom * W) - (M - CanvasWidth/2)
            // CameraPos' = Ratio * (CameraPos + M - CanvasWidth/2) - (M - CanvasWidth/2)

            if params.phase != ClientPhase::Dead {
                manager.camera = (manager.camera + mouse_delta) * ratio - mouse_delta;
                manager.target_camera = (manager.target_camera + mouse_delta) * ratio - mouse_delta;
            }
            changed = true;
        }
    }

    // 2. Velocity (inertia)
    if !params.is_dragging {
        if manager.velocity.x.abs() > params.velocity_cutoff
            || manager.velocity.y.abs() > params.velocity_cutoff
        {
            manager.camera += manager.velocity;
            manager.velocity *= params.inertia_decay;
            manager.target_camera = manager.camera;
            changed = true;
        } else {
            manager.velocity = Vec2::ZERO;
        }
    } else {
        manager.target_camera = manager.camera;
    }

    // 3. King Following / Death Focusing / Menu Focusing
    if is_alive {
        if let Some(p) = player
            && let Some(king) = params.state.pieces.get(&p.king_id)
        {
            let tile_size = params.tile_size_px * manager.zoom;
            let kpx = king.position.0.x as f64 * tile_size + tile_size / 2.0;
            let kpy = king.position.0.y as f64 * tile_size + tile_size / 2.0;
            let king_pos = Vec2::new(kpx, kpy);

            if !manager.was_alive {
                // Respawn or First Join: Set target to king and start panning
                manager.target_camera = king_pos;
                manager.target_zoom = 1.0;
                manager.last_king_grid_pos = king.position.into();
                manager.input_locked = true;
                manager.was_alive = true;
                changed = true;
            } else {
                manager.last_king_grid_pos = king.position.into();
                // We don't update target_camera here, allowing for free-panning
            }

            // 4. Global Clamping (Respect camera_pan_limit)
            if let Some(m) = params.mode {
                let mut vars = std::collections::HashMap::new();
                vars.insert("player_piece_count".to_string(), params.piece_count as f64);

                let fog_of_war_radius = m
                    .fog_of_war_radius
                    .as_ref()
                    .map(|expr| evaluate_expression(expr, &vars))
                    .unwrap_or(0.0);
                vars.insert("fog_of_war_radius".to_string(), fog_of_war_radius);

                let limit_radius_tiles = evaluate_expression(&m.camera_pan_limit, &vars);
                let limit_radius_px = limit_radius_tiles * tile_size;

                let delta = manager.camera - king_pos;
                let dist = delta.length();

                if dist > limit_radius_px && dist > 0.1 && !params.is_dragging {
                    // Not dragging: Smoothly interpolate back to the allowed region
                    let target_dist = limit_radius_px;
                    let ratio = target_dist / dist;
                    let target = king_pos + delta * ratio;

                    manager.target_camera = target;
                }
            }
        }
    } else if params.phase == ClientPhase::Dead {
        if manager.was_alive {
            if params.is_victory {
                match params.victory_focus_target {
                    VictoryFocusTarget::KeepCurrent => {
                        manager.target_camera = manager.camera;
                        manager.target_zoom = manager.zoom;
                    }
                    VictoryFocusTarget::BoardPosition(grid_pos) => {
                        let target_zoom = params.death_zoom;
                        let tile_size = params.tile_size_px * target_zoom;
                        let desired_focus_x = grid_pos.0.x as f64 * tile_size + tile_size / 2.0;
                        let desired_focus_y = grid_pos.0.y as f64 * tile_size + tile_size / 2.0;
                        manager.target_camera = Vec2::new(desired_focus_x, desired_focus_y);
                        manager.target_zoom = target_zoom.clamp(params.zoom_min, params.zoom_max);
                    }
                }
            } else {
                // Just died, focus on last king position.
                let grid_pos = manager.last_king_grid_pos;
                let target_zoom = params.death_zoom;
                let tile_size = params.tile_size_px * target_zoom;
                let desired_focus_x = grid_pos.x as f64 * tile_size + tile_size / 2.0;
                let desired_focus_y = grid_pos.y as f64 * tile_size + tile_size / 2.0;
                manager.target_camera = Vec2::new(desired_focus_x, desired_focus_y);
                manager.target_zoom = target_zoom.clamp(params.zoom_min, params.zoom_max);
            }
            manager.was_alive = false;
            manager.velocity = Vec2::ZERO;
            manager.input_locked = false;
            changed = true;
        }
    } else if matches!(params.phase, ClientPhase::Menu | ClientPhase::Queued) {
        // Menu / Choose Army screen
        // In this coordinate system, (0,0) is the center of the board
        manager.target_camera = Vec2::ZERO;
        manager.target_zoom = 1.0;
        manager.was_alive = false;
        manager.input_locked = false;
        changed = true;
    } else {
        manager.was_alive = false;
        manager.input_locked = false;
    }

    // 5. Final interpolation for target_camera
    if !params.is_dragging
        && ((manager.target_camera.x - manager.camera.x).abs() > 0.1
            || (manager.target_camera.y - manager.camera.y).abs() > 0.1)
    {
        let factor = if is_alive {
            params.pan_lerp_alive
        } else {
            params.pan_lerp_dead
        }; // Slightly slower pan in menus
        manager.camera += (manager.target_camera - manager.camera) * factor;
        changed = true;
    }

    if manager.input_locked {
        let delta = manager.target_camera - manager.camera;
        let close_enough = delta.x.abs() < 1.0 && delta.y.abs() < 1.0;
        let zoom_synced = (manager.target_zoom - manager.zoom).abs() < 0.01;
        if close_enough && zoom_synced {
            manager.input_locked = false;
        }
    }

    changed
}
