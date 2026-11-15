//! Camera update logic for panning, zooming, and follow behavior.

mod flight;

use crate::math::Vec2;
use crate::reducer::ClientPhase;
use crate::utils::performance_now;
use common::models::JoinCameraCenterConfig;
use common::protocol::VictoryFocusTarget;
use common::*;
use flight::{CameraFlight, CameraPose};
use yew::prelude::*;

pub use flight::FlightTiming;

/// Length of the step the camera tuning values are expressed per, in milliseconds.
///
/// The camera advances once per displayed frame, scaling its smoothing by how many of these
/// steps elapsed, so it moves at the same speed on any refresh rate.
pub const CAMERA_STEP_MS: f64 = 16.0;

/// Fraction of the remaining distance covered after `steps` steps covering `factor` each.
fn lerp_factor(factor: f64, steps: f64) -> f64 {
    1.0 - (1.0 - factor).powf(steps)
}

/// Mutable camera state for smooth panning and zoom transitions.
pub struct CameraManager {
    pub camera: Vec2,
    pub target_camera: Vec2,
    pub zoom: f64,
    pub target_zoom: f64,
    pub mouse_pos: Vec2,
    /// Pan momentum, in pixels per camera step.
    pub velocity: Vec2,
    /// `performance.now()` of the last pan movement, used to drop stale momentum.
    pub last_pan_ms: f64,
    /// Scripted move in progress, which holds off player input until it lands.
    flight: Option<CameraFlight>,
    pub was_alive: bool,
    /// True while the menu or queue screen was showing on the last update.
    was_in_menu: bool,
    pub last_tracked_king_id: Option<PieceId>,
    pub last_tracked_join_time: Option<TimestampMs>,
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
            last_pan_ms: 0.0,
            flight: None,
            was_alive: false,
            was_in_menu: false,
            last_tracked_king_id: None,
            last_tracked_join_time: None,
            last_king_grid_pos: glam::IVec2::ZERO,
            last_touch_dist: None,
            last_touch_center: None,
        }
    }

    /// Returns true while a scripted move is in progress and player input is ignored.
    pub fn input_locked(&self) -> bool {
        self.flight.is_some()
    }

    /// Returns the current pose, for squares of `tile_size_px` pixels at zoom 1.
    fn pose(&self, tile_size_px: f64) -> CameraPose {
        CameraPose {
            center: self.camera / (tile_size_px * self.zoom),
            zoom: self.zoom,
        }
    }

    /// Jumps the camera to `pose`, leaving nothing to ease towards.
    fn set_pose(&mut self, pose: CameraPose, tile_size_px: f64) {
        self.camera = pose.center * tile_size_px * pose.zoom;
        self.target_camera = self.camera;
        self.zoom = pose.zoom;
        self.target_zoom = pose.zoom;
    }

    /// Starts a scripted move to `target`, whose zoom is clamped to the allowed range.
    fn fly_to(&mut self, target: CameraPose, params: &CameraUpdateParams<'_>) {
        let target = CameraPose {
            zoom: target.zoom.clamp(params.zoom_min, params.zoom_max),
            ..target
        };
        self.flight = Some(CameraFlight::new(
            self.pose(params.tile_size_px),
            target,
            params.flight_timing,
        ));
        self.velocity = Vec2::ZERO;
    }
}

/// Returns the center of `square`, in squares from the board origin.
fn square_center(square: glam::IVec2) -> Vec2 {
    square.as_dvec2() + 0.5
}

/// Input parameters for a camera update tick.
pub struct CameraUpdateParams<'a> {
    /// Camera steps elapsed since the last update.
    pub steps: f64,
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
    /// Milliseconds a pan can hold still before its release stops carrying momentum.
    pub pan_idle_ms: f64,
    pub pan_lerp_alive: f64,
    /// Timing of scripted moves: joining, the death or victory focus, and returning to the menu.
    pub flight_timing: FlightTiming,
    pub tile_size_px: f64,
    pub death_zoom: f64,
    pub board_rotated_180: bool,
}

/// Advances the camera state by `params.steps` steps using inputs and game state.
///
/// `manager` is the mutable camera state, `params` contains context and settings.
/// Returns `true` if the camera state changed and requires a re-render.
pub fn update_camera(manager: &mut CameraManager, params: CameraUpdateParams<'_>) -> bool {
    let mut changed = false;
    let player_id_val = params.player_id.unwrap_or_else(PlayerId::nil);
    let player = params.state.players.get(&player_id_val);
    let is_alive = params.phase == ClientPhase::Alive;
    let in_menu = matches!(params.phase, ClientPhase::Menu | ClientPhase::Queued);
    let entering_menu = in_menu && !manager.was_in_menu;
    manager.was_in_menu = in_menu;

    manager.target_zoom = manager.target_zoom.clamp(params.zoom_min, params.zoom_max);
    manager.zoom = manager.zoom.clamp(params.zoom_min, params.zoom_max);

    // 1. Zoom interpolation, which a flight does itself
    if manager.flight.is_none() && (manager.target_zoom - manager.zoom).abs() > 0.000001 {
        let factor = lerp_factor(params.zoom_lerp, params.steps);
        let old_z = manager.zoom;
        manager.zoom += (manager.target_zoom - manager.zoom) * factor;
        let ratio = manager.zoom / old_z;

        if let Some(canvas) = params.canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
            let rect = canvas.get_bounding_client_rect();

            // Mouse position relative to canvas center
            // When dead, anchor zoom to canvas center so the death focus stays accurate.
            let mut mouse_delta = if params.phase == ClientPhase::Dead {
                Vec2::ZERO
            } else {
                Vec2::new(
                    manager.mouse_pos.x - rect.left() - (canvas.width() as f64 / 2.0),
                    manager.mouse_pos.y - rect.top() - (canvas.height() as f64 / 2.0),
                )
            };
            if params.board_rotated_180 {
                mouse_delta = -mouse_delta;
            }

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
            // Distance a velocity decaying every step covers over the elapsed steps.
            let decay = params.inertia_decay.powf(params.steps);
            let distance = if params.inertia_decay < 1.0 {
                (1.0 - decay) / (1.0 - params.inertia_decay)
            } else {
                params.steps
            };
            manager.camera += manager.velocity * distance;
            manager.velocity *= decay;
            manager.target_camera = manager.camera;
            changed = true;
        } else {
            manager.velocity = Vec2::ZERO;
        }
    } else {
        // A pan held still gives no momentum on release.
        if performance_now() - manager.last_pan_ms > params.pan_idle_ms {
            manager.velocity = Vec2::ZERO;
        }
        manager.target_camera = manager.camera;
    }

    // 3. King Following / Death Focusing / Menu Focusing
    if is_alive {
        if let Some(p) = player
            && let Some(king) = params.state.pieces.get(&p.king_id)
        {
            let fresh_alive_entry = !manager.was_alive
                || manager.last_tracked_king_id != Some(p.king_id)
                || manager.last_tracked_join_time != Some(p.join_time);
            if fresh_alive_entry {
                let center = join_focus_center(&params, p.king_id, square_center(king.position.0));
                manager.fly_to(CameraPose { center, zoom: 1.0 }, &params);
                manager.last_king_grid_pos = king.position.into();
                manager.last_tracked_king_id = Some(p.king_id);
                manager.last_tracked_join_time = Some(p.join_time);
                manager.was_alive = true;
                changed = true;
            } else {
                manager.last_king_grid_pos = king.position.into();
                manager.last_tracked_king_id = Some(p.king_id);
                manager.last_tracked_join_time = Some(p.join_time);
                // We don't update target_camera here, allowing for free-panning
            }

            let tile_size = params.tile_size_px * manager.zoom;
            let kpx = king.position.0.x as f64 * tile_size + tile_size / 2.0;
            let kpy = king.position.0.y as f64 * tile_size + tile_size / 2.0;
            let king_pos = Vec2::new(kpx, kpy);

            // 4. Global Clamping (Respect camera_pan_limit)
            if let Some(m) = params.mode {
                let mut vars = ExprVars::new().with("player_piece_count", params.piece_count);
                let fog_of_war_radius = m
                    .fog_of_war_radius
                    .as_ref()
                    .map_or(0.0, |expr| expr.evaluate(&vars));
                vars.set("fog_of_war_radius", fog_of_war_radius);

                let limit_radius_tiles = m.camera_pan_limit.evaluate(&vars);
                let limit_radius_px = limit_radius_tiles * tile_size;

                let delta = manager.camera - king_pos;
                let dist = delta.length();

                if dist > limit_radius_px
                    && dist > 0.1
                    && !params.is_dragging
                    && manager.flight.is_none()
                {
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
                        manager.flight = None;
                        manager.target_camera = manager.camera;
                        manager.target_zoom = manager.zoom;
                    }
                    VictoryFocusTarget::BoardPosition(grid_pos) => {
                        let focus = CameraPose {
                            center: square_center(grid_pos.0),
                            zoom: params.death_zoom,
                        };
                        manager.fly_to(focus, &params);
                    }
                }
            } else {
                // Just died, focus on last king position.
                let focus = CameraPose {
                    center: square_center(manager.last_king_grid_pos),
                    zoom: params.death_zoom,
                };
                manager.fly_to(focus, &params);
            }
            manager.was_alive = false;
            manager.last_tracked_king_id = None;
            manager.last_tracked_join_time = None;
            manager.velocity = Vec2::ZERO;
            changed = true;
        }
    } else if params.phase == ClientPhase::Spectating {
        manager.was_alive = false;
        manager.last_tracked_king_id = None;
        manager.last_tracked_join_time = None;
        // Free panning, pulled back once the view centre leaves the board.
        if !params.is_dragging && manager.flight.is_none() {
            let tile_size = params.tile_size_px * manager.zoom;
            let bounded = board_bounds(params.state.board_size, tile_size).clamp(manager.camera);
            if bounded != manager.camera {
                manager.target_camera = bounded;
            }
        }
    } else if in_menu {
        // Menu / Choose Army screen, centred on the middle of the board.
        if entering_menu {
            let center = CameraPose {
                center: Vec2::ZERO,
                zoom: 1.0,
            };
            manager.fly_to(center, &params);
        }
        manager.was_alive = false;
        manager.last_tracked_king_id = None;
        manager.last_tracked_join_time = None;
        changed = true;
    } else {
        manager.was_alive = false;
        manager.last_tracked_king_id = None;
        manager.last_tracked_join_time = None;
    }

    // 5. Scripted flight, or easing towards target_camera
    if let Some(flight) = &mut manager.flight {
        let pose = flight.advance(params.steps * CAMERA_STEP_MS);
        if flight.is_done() {
            manager.flight = None;
        }
        manager.set_pose(pose, params.tile_size_px);
        changed = true;
    } else if !params.is_dragging
        && ((manager.target_camera.x - manager.camera.x).abs() > 0.1
            || (manager.target_camera.y - manager.camera.y).abs() > 0.1)
    {
        manager.camera += (manager.target_camera - manager.camera)
            * lerp_factor(params.pan_lerp_alive, params.steps);
        changed = true;
    }

    changed
}

/// Area of camera positions, in pixels, whose view centre lies on the board.
struct CameraBounds {
    min: Vec2,
    max: Vec2,
}

impl CameraBounds {
    /// Returns the point of the bounds closest to `camera`.
    fn clamp(&self, camera: Vec2) -> Vec2 {
        camera.clamp(self.min, self.max)
    }
}

/// Returns the camera bounds of a board of `board_size` drawn with `tile_size` pixel squares.
fn board_bounds(board_size: BoardSize, tile_size: f64) -> CameraBounds {
    CameraBounds {
        min: Vec2::splat(-board_size.half() as f64 * tile_size),
        max: Vec2::splat(board_size.limit_pos() as f64 * tile_size),
    }
}

/// Returns the point, in squares, the camera centres on when the player joins: as the mode
/// configures, or `fallback` when it names no piece the player owns.
fn join_focus_center(params: &CameraUpdateParams<'_>, king_id: PieceId, fallback: Vec2) -> Vec2 {
    let Some(mode) = params.mode else {
        return fallback;
    };
    let focus = &mode.join_camera_center;
    match focus {
        JoinCameraCenterConfig::Position { position } => Vec2::new(position[0], position[1]),
        JoinCameraCenterConfig::Piece { piece_id } => {
            let player_id = params.player_id.unwrap_or_else(PlayerId::nil);
            let target_piece = params.state.pieces.get(&king_id).and_then(|king| {
                if king.piece_type == *piece_id {
                    Some(king)
                } else {
                    params.state.pieces.values().find(|piece| {
                        piece.owner_id == Some(player_id) && piece.piece_type == *piece_id
                    })
                }
            });
            target_piece
                .map(|piece| square_center(piece.position.0))
                .unwrap_or(fallback)
        }
    }
}
