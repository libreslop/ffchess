//! The per-frame camera step: easing, momentum, and the moves each phase of the session makes.

use super::flight::{CameraFlight, CameraPose};
use super::framing::{
    CameraBounds, StartFraming, StartingCamera, ZoomRange, board_center, max_fit_zoom,
    square_center, view_offset,
};
use super::{CAMERA_STEP_MS, CameraManager, TrackedLife};
use crate::app::GlobalClientConfig;
use crate::math::Vec2;
use crate::reducer::{ClientPhase, GameStateReducer};
use crate::utils::performance_now;
use common::models::{CameraCue, MaxFitZoom};
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::NodeRef;

/// Zoom difference below which the zoom counts as arrived at its target.
const ZOOM_EPSILON: f64 = 1e-6;

/// Distance, in pixels, below which the camera counts as arrived at its target.
const PAN_EPSILON_PX: f64 = 0.1;

/// How the camera feels, from the client config.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTuning {
    /// Zoom levels the player may pick, widened at runtime to reach the whole-board fit.
    zoom_range: ZoomRange,
    /// Fraction of the way to the target zoom covered per camera step.
    zoom_lerp: f64,
    /// Fraction of the way to the target position covered per camera step.
    pan_lerp: f64,
    /// Factor pan momentum keeps per camera step.
    inertia_decay: f64,
    /// Momentum, in pixels per camera step, below which the camera stops coasting.
    velocity_cutoff: f64,
    /// Milliseconds a pan can hold still before its release stops carrying momentum.
    pan_idle_ms: f64,
    /// Duration of scripted moves: joining, hook camera moves, and returning to the menu.
    flight_ms: f64,
    /// Size of a square at zoom 1, in pixels.
    tile_size_px: f64,
}

impl From<&GlobalClientConfig> for CameraTuning {
    fn from(globals: &GlobalClientConfig) -> Self {
        Self {
            zoom_range: ZoomRange::new(globals.camera_zoom_min, globals.camera_zoom_max),
            zoom_lerp: globals.zoom_lerp,
            pan_lerp: globals.pan_lerp_alive,
            inertia_decay: globals.inertia_decay,
            velocity_cutoff: globals.velocity_cutoff,
            pan_idle_ms: globals.pan_idle_ms,
            flight_ms: globals.focus_flight_ms,
            tile_size_px: globals.tile_size_px,
        }
    }
}

/// Inputs of one camera step.
pub struct CameraUpdateParams<'a> {
    /// Camera steps elapsed since the last update.
    pub steps: f64,
    pub tuning: CameraTuning,
    /// Session and board the view shows.
    pub reducer: &'a GameStateReducer,
    pub canvas_ref: &'a NodeRef,
    /// True while the pointer is pressed on the board.
    pub is_dragging: bool,
}

impl CameraUpdateParams<'_> {
    /// Returns how the mode starts the camera for the viewer.
    fn starting_camera(&self) -> StartingCamera<'_> {
        StartingCamera {
            mode: self.reducer.mode.as_ref(),
            state: &self.reducer.state,
            viewer: self.reducer.viewer_id(),
        }
    }

    /// Returns the size of a square at zoom `zoom`, in pixels.
    fn tile_size(&self, zoom: f64) -> f64 {
        self.tuning.tile_size_px * zoom
    }
}

/// Returns the fraction of the remaining distance covered after `steps` steps covering
/// `factor` each.
fn lerp_factor(factor: f64, steps: f64) -> f64 {
    1.0 - (1.0 - factor).powf(steps)
}

impl CameraManager {
    /// Advances the camera by `params.steps` camera steps.
    ///
    /// Returns true when the camera moved and the view needs a re-render.
    pub fn update(&mut self, params: &CameraUpdateParams<'_>) -> bool {
        let fit = max_fit_zoom(
            params.canvas_ref,
            params.reducer.state.board_size,
            params.tuning.tile_size_px,
        );
        self.zoom_range = params.tuning.zoom_range.including(fit.0);
        self.target_zoom = self.zoom_range.clamp(self.target_zoom);
        self.zoom = self.zoom_range.clamp(self.zoom);

        let phase = params.reducer.phase;
        let previous_phase = self.last_phase.replace(phase);
        let mut changed = self.ease_zoom(params);
        changed |= self.coast(params);
        changed |= match phase {
            ClientPhase::Alive => self.follow_player(params, fit),
            ClientPhase::Dead => self.settle_after_death(),
            ClientPhase::Spectating => self.watch_board(params, previous_phase, fit),
            ClientPhase::Menu | ClientPhase::Queued => self.show_menu(params, previous_phase),
            ClientPhase::Joining => {
                self.tracked_life = None;
                false
            }
        };
        if let Some(cue) = &params.reducer.camera_cue {
            changed |= self.apply_cue(cue, fit, params);
        }
        changed |= self.follow_start_framing(params, fit);
        changed | self.advance(params)
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

    /// Eases the zoom towards its target, keeping the board under the pointer still; a dead
    /// player's view zooms about its centre so the death focus stays put. A flight zooms by
    /// itself.
    fn ease_zoom(&mut self, params: &CameraUpdateParams<'_>) -> bool {
        if self.flight.is_some() || (self.target_zoom - self.zoom).abs() <= ZOOM_EPSILON {
            return false;
        }
        let old_zoom = self.zoom;
        self.zoom +=
            (self.target_zoom - self.zoom) * lerp_factor(params.tuning.zoom_lerp, params.steps);
        let anchor = match params.canvas_ref.cast::<HtmlCanvasElement>() {
            Some(canvas) if params.reducer.phase != ClientPhase::Dead => {
                view_offset(&canvas, self.mouse_pos)
            }
            _ => Vec2::ZERO,
        };
        self.rescale_about(self.zoom / old_zoom, anchor);
        true
    }

    /// Carries the camera on with the momentum of a released pan, which decays every step.
    ///
    /// While the pointer is still pressed, the camera stays where the pan put it, and a pan
    /// held still loses its momentum.
    fn coast(&mut self, params: &CameraUpdateParams<'_>) -> bool {
        let tuning = &params.tuning;
        if params.is_dragging {
            if performance_now() - self.last_pan_ms > tuning.pan_idle_ms {
                self.velocity = Vec2::ZERO;
            }
            self.target_camera = self.camera;
            return false;
        }
        if self.velocity.abs().max_element() <= tuning.velocity_cutoff {
            self.velocity = Vec2::ZERO;
            return false;
        }
        // Distance a velocity decaying every step covers over the elapsed steps.
        let decay = tuning.inertia_decay.powf(params.steps);
        let distance = if tuning.inertia_decay < 1.0 {
            (1.0 - decay) / (1.0 - tuning.inertia_decay)
        } else {
            params.steps
        };
        self.camera += self.velocity * distance;
        self.velocity *= decay;
        self.target_camera = self.camera;
        true
    }

    /// Frames the local player on every new life, then lets them pan freely within the
    /// mode's pan limit around their king.
    fn follow_player(&mut self, params: &CameraUpdateParams<'_>, fit: MaxFitZoom) -> bool {
        let reducer = params.reducer;
        let Some(player) = reducer.state.players.get(&reducer.viewer_id()) else {
            return false;
        };
        let Some(king) = reducer.state.pieces.get(&player.king_id) else {
            return false;
        };
        let life = TrackedLife {
            king_id: player.king_id,
            join_time: player.join_time,
        };
        let king_center = square_center(king.position);
        let is_new_life = self.tracked_life.replace(life) != Some(life);
        if is_new_life {
            let center = params.starting_camera().center(king_center);
            self.frame_start(center, params, fit);
        }
        self.pull_within_pan_limit(params, king_center);
        is_new_life
    }

    /// Eases a camera that strayed past the mode's pan limit back to its edge, unless the
    /// player is panning or a flight runs.
    fn pull_within_pan_limit(&mut self, params: &CameraUpdateParams<'_>, king_center: Vec2) {
        let Some(mode) = &params.reducer.mode else {
            return;
        };
        if params.is_dragging || self.flight.is_some() {
            return;
        }
        let tile_size = params.tile_size(self.zoom);
        let piece_count = params
            .reducer
            .state
            .pieces_of(params.reducer.viewer_id())
            .count();
        let limit_px = mode.pan_limit(piece_count) * tile_size;
        let king_px = king_center * tile_size;
        let offset = self.camera - king_px;
        let distance = offset.length();
        if distance > limit_px && distance > PAN_EPSILON_PX {
            self.target_camera = king_px + offset * (limit_px / distance);
        }
    }

    /// Stops the camera once the player's life ended; hooks move the camera at the end of a
    /// life with camera cues, which may arrive before this phase.
    fn settle_after_death(&mut self) -> bool {
        if self.tracked_life.take().is_none() {
            return false;
        }
        if self.flight.is_none() {
            self.target_camera = self.camera;
            self.target_zoom = self.zoom;
        }
        self.velocity = Vec2::ZERO;
        true
    }

    /// Frames the board when starting to watch it, then lets the viewer pan freely, pulled
    /// back once the view centre leaves the board.
    ///
    /// Watching from the menu starts framed as playing would; watching the match just left
    /// keeps the view of its end.
    fn watch_board(
        &mut self,
        params: &CameraUpdateParams<'_>,
        previous_phase: Option<ClientPhase>,
        fit: MaxFitZoom,
    ) -> bool {
        self.tracked_life = None;
        let starts_watching = !matches!(
            previous_phase,
            Some(ClientPhase::Spectating | ClientPhase::Dead)
        );
        if starts_watching {
            let board_size = params.reducer.state.board_size;
            let center = params.starting_camera().center(board_center(board_size));
            self.frame_start(center, params, fit);
        }
        if !params.is_dragging && self.flight.is_none() {
            let tile_size = params.tile_size(self.zoom);
            let bounded = CameraBounds::of_board(params.reducer.state.board_size, tile_size)
                .clamp(self.camera);
            if bounded != self.camera {
                self.target_camera = bounded;
            }
        }
        starts_watching
    }

    /// Flies back to the middle of the board on returning to the menu.
    fn show_menu(
        &mut self,
        params: &CameraUpdateParams<'_>,
        previous_phase: Option<ClientPhase>,
    ) -> bool {
        self.tracked_life = None;
        let was_in_menu = matches!(
            previous_phase,
            Some(ClientPhase::Menu | ClientPhase::Queued)
        );
        if !was_in_menu {
            let origin = CameraPose {
                center: Vec2::ZERO,
                zoom: 1.0,
            };
            self.fly_to(origin, params);
        }
        !was_in_menu
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
            self.pose(params.tuning.tile_size_px),
            target,
            params.tuning.flight_ms,
        ));
        self.velocity = Vec2::ZERO;
        self.start_framing = None;
        target
    }

    /// Flies to the starting pose centred on `center`, and keeps it fitted to the screen.
    ///
    /// A replay opens straight in the starting pose, as there is no earlier view of it to fly
    /// from.
    fn frame_start(&mut self, center: Vec2, params: &CameraUpdateParams<'_>, fit: MaxFitZoom) {
        let pose = CameraPose {
            center,
            zoom: params.starting_camera().zoom(fit),
        };
        let pose = if params.reducer.is_replay() {
            self.jump_to(pose, params)
        } else {
            self.fly_to(pose, params)
        };
        self.start_framing = Some(StartFraming { pose, fit });
    }

    /// Puts the camera in `target` at once, its zoom clamped to the allowed range, and returns
    /// the pose it lands on.
    fn jump_to(&mut self, target: CameraPose, params: &CameraUpdateParams<'_>) -> CameraPose {
        let target = CameraPose {
            zoom: self.zoom_range.clamp(target.zoom),
            ..target
        };
        self.flight = None;
        self.velocity = Vec2::ZERO;
        self.set_pose(target, params.tuning.tile_size_px);
        target
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
        let starting_camera = params.starting_camera();
        if self.flight.is_none() && !starting_camera.resets_on_resize() {
            self.start_framing = None;
            return false;
        }
        if framing.fit == fit {
            return false;
        }
        let pose = CameraPose {
            center: framing.pose.center,
            zoom: self.zoom_range.clamp(starting_camera.zoom(fit)),
        };
        if self.flight.is_some() {
            self.fly_to(pose, params);
        } else {
            self.set_pose(pose, params.tuning.tile_size_px);
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
        fit: MaxFitZoom,
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
        let target = cue.target(fit);
        if target.position.is_none() && target.zoom.is_none() {
            return false;
        }
        let current = self.pose(params.tuning.tile_size_px);
        let pose = CameraPose {
            center: target.position.unwrap_or(current.center),
            zoom: target.zoom.unwrap_or(current.zoom),
        };
        self.fly_to(pose, params);
        true
    }

    /// Moves the camera along its flight, or eases it towards its target position.
    fn advance(&mut self, params: &CameraUpdateParams<'_>) -> bool {
        if let Some(flight) = &mut self.flight {
            let pose = flight.advance(params.steps * CAMERA_STEP_MS);
            if flight.is_done() {
                self.flight = None;
            }
            self.set_pose(pose, params.tuning.tile_size_px);
            return true;
        }
        let remaining = self.target_camera - self.camera;
        if params.is_dragging || remaining.abs().max_element() <= PAN_EPSILON_PX {
            return false;
        }
        self.camera += remaining * lerp_factor(params.tuning.pan_lerp, params.steps);
        true
    }
}
