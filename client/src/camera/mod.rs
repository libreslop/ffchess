//! Camera update logic for panning, zooming, and follow behavior.

mod flight;
mod framing;

pub use framing::ZoomRange;

use crate::math::Vec2;
use crate::reducer::ClientPhase;
use crate::utils::performance_now;
use common::models::{CameraCue, MaxFitZoom, StartingCameraPosition};
use common::*;
use flight::{CameraFlight, CameraPose};
use framing::max_fit_zoom;
use std::rc::Rc;
use yew::prelude::*;

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
    /// Zoom levels allowed on the last update: the configured range, widened to reach the
    /// zoom fitting the whole board.
    zoom_range: ZoomRange,
    /// Last hook camera move made, so each is made once.
    applied_cue: Option<Rc<CameraCue>>,
    /// Starting pose the camera keeps fitted to the screen: while flying there, and after
    /// landing too when the mode resets the camera on resize.
    start_framing: Option<StartFraming>,
    pub was_alive: bool,
    /// Phase on the last update.
    last_phase: Option<ClientPhase>,
    pub last_tracked_king_id: Option<PieceId>,
    pub last_tracked_join_time: Option<TimestampMs>,
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
            zoom_range: ZoomRange::new(1.0, 1.0),
            applied_cue: None,
            start_framing: None,
            was_alive: false,
            last_phase: None,
            last_tracked_king_id: None,
            last_tracked_join_time: None,
            last_touch_dist: None,
            last_touch_center: None,
        }
    }

    /// Returns true while a scripted move is in progress and player input is ignored.
    pub fn input_locked(&self) -> bool {
        self.flight.is_some()
    }

    /// Returns the zoom levels the camera may take.
    pub fn zoom_range(&self) -> ZoomRange {
        self.zoom_range
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

    /// Starts a scripted move to `target`, whose zoom is clamped to the allowed range, and
    /// returns the pose it lands on.
    ///
    /// The move replaces any starting framing.
    fn fly_to(&mut self, target: CameraPose, params: &CameraUpdateParams<'_>) -> CameraPose {
        let target = CameraPose {
            zoom: self.zoom_range.clamp(target.zoom),
            ..target
        };
        self.flight = Some(CameraFlight::new(
            self.pose(params.tile_size_px),
            target,
            params.flight_ms,
        ));
        self.velocity = Vec2::ZERO;
        self.start_framing = None;
        target
    }

    /// Flies to the starting pose centred on `center`, and keeps it fitted to the screen.
    fn frame_start(&mut self, center: Vec2, params: &CameraUpdateParams<'_>, fit: MaxFitZoom) {
        let pose = CameraPose {
            center,
            zoom: starting_zoom(params, fit),
        };
        let pose = self.fly_to(pose, params);
        self.start_framing = Some(StartFraming { pose, fit });
    }

    /// Refits the starting pose when the room for the board changed: while flying there, as
    /// the view may still be laying out, and after landing only when the mode resets the
    /// camera on resize, which puts it back in the starting pose wherever it was moved.
    ///
    /// Returns true when the camera moved.
    fn follow_start_framing(&mut self, params: &CameraUpdateParams<'_>, fit: MaxFitZoom) -> bool {
        let Some(framing) = self.start_framing else {
            return false;
        };
        let reset_on_resize = params
            .mode
            .is_some_and(|mode| mode.starting_camera.reset_on_resize);
        if self.flight.is_none() && !reset_on_resize {
            self.start_framing = None;
            return false;
        }
        if framing.fit == fit {
            return false;
        }
        let pose = CameraPose {
            center: framing.pose.center,
            zoom: self.zoom_range.clamp(starting_zoom(params, fit)),
        };
        if self.flight.is_some() {
            self.fly_to(pose, params);
        } else {
            self.set_pose(pose, params.tile_size_px);
        }
        self.start_framing = Some(StartFraming { pose, fit });
        true
    }

    /// Makes the camera move of `cue` unless it was already made.
    ///
    /// Returns true when the camera started moving.
    fn apply_cue(
        &mut self,
        cue: &Rc<CameraCue>,
        max_fit_zoom: MaxFitZoom,
        params: &CameraUpdateParams<'_>,
    ) -> bool {
        if self
            .applied_cue
            .as_ref()
            .is_some_and(|applied| Rc::ptr_eq(applied, cue))
        {
            return false;
        }
        self.applied_cue = Some(cue.clone());
        let target = cue.target(max_fit_zoom);
        if target.position.is_none() && target.zoom.is_none() {
            return false;
        }
        let current = self.pose(params.tile_size_px);
        let pose = CameraPose {
            center: target.position.unwrap_or(current.center),
            zoom: target.zoom.unwrap_or(current.zoom),
        };
        self.fly_to(pose, params);
        true
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
    /// Latest camera move a hook made for the local player.
    pub camera_cue: Option<Rc<CameraCue>>,
    pub zoom_min: f64,
    pub zoom_max: f64,
    pub zoom_lerp: f64,
    pub inertia_decay: f64,
    pub velocity_cutoff: f64,
    /// Milliseconds a pan can hold still before its release stops carrying momentum.
    pub pan_idle_ms: f64,
    pub pan_lerp_alive: f64,
    /// Duration of scripted moves: joining, hook camera moves, and returning to the menu.
    pub flight_ms: f64,
    pub tile_size_px: f64,
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
    let previous_phase = manager.last_phase.replace(params.phase);
    let was_in_menu = matches!(
        previous_phase,
        Some(ClientPhase::Menu | ClientPhase::Queued)
    );
    let entering_menu = in_menu && !was_in_menu;
    let entering_spectate =
        params.phase == ClientPhase::Spectating && previous_phase != Some(params.phase);

    let max_fit_zoom = max_fit_zoom(
        params.canvas_ref,
        params.state.board_size,
        params.tile_size_px,
    );
    manager.zoom_range = ZoomRange::new(params.zoom_min, params.zoom_max).including(max_fit_zoom.0);
    manager.target_zoom = manager.zoom_range.clamp(manager.target_zoom);
    manager.zoom = manager.zoom_range.clamp(manager.zoom);

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
                let center = starting_center(&params, square_center(king.position.0));
                manager.frame_start(center, &params, max_fit_zoom);
                manager.last_tracked_king_id = Some(p.king_id);
                manager.last_tracked_join_time = Some(p.join_time);
                manager.was_alive = true;
                changed = true;
            } else {
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
            // Hooks move the camera at the end of a life with camera cues, which may arrive
            // before this phase; without one the camera settles where it is.
            if manager.flight.is_none() {
                manager.target_camera = manager.camera;
                manager.target_zoom = manager.zoom;
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
        // Watching from the menu starts framed as playing would; watching the match just
        // left keeps the view of its end.
        if entering_spectate && previous_phase != Some(ClientPhase::Dead) {
            let center = starting_center(&params, board_center(params.state.board_size));
            manager.frame_start(center, &params, max_fit_zoom);
            changed = true;
        }
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

    if let Some(cue) = &params.camera_cue {
        changed |= manager.apply_cue(cue, max_fit_zoom, &params);
    }
    changed |= manager.follow_start_framing(&params, max_fit_zoom);

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

/// Returns the centre of a board of `board_size`, in squares from the board origin.
fn board_center(board_size: BoardSize) -> Vec2 {
    Vec2::splat((board_size.limit_pos() - board_size.half()) as f64 / 2.0)
}

/// Starting pose kept fitted to the screen.
#[derive(Clone, Copy, Debug)]
struct StartFraming {
    /// Pose the camera was sent to.
    pose: CameraPose,
    /// Fit zoom the pose was worked out for.
    fit: MaxFitZoom,
}

/// Returns the point the camera centres on when the player joins or starts watching: as the
/// mode configures, or `fallback` when it names no piece the viewer owns.
fn starting_center(params: &CameraUpdateParams<'_>, fallback: Vec2) -> Vec2 {
    let Some(mode) = params.mode else {
        return fallback;
    };
    match &mode.starting_camera.position {
        StartingCameraPosition::Point(point) => Vec2::new(point[0], point[1]),
        StartingCameraPosition::Piece { piece_id } => {
            let viewer = params.player_id.unwrap_or_else(PlayerId::nil);
            params
                .state
                .pieces
                .values()
                .find(|piece| piece.owner_id == Some(viewer) && piece.piece_type == *piece_id)
                .map_or(fallback, |piece| square_center(piece.position.0))
        }
    }
}

/// Returns the zoom the camera starts at when the player joins or starts watching.
fn starting_zoom(params: &CameraUpdateParams<'_>, fit: MaxFitZoom) -> f64 {
    params
        .mode
        .map_or(1.0, |mode| mode.starting_camera.zoom(fit))
}
