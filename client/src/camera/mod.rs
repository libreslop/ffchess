//! Camera of the board view: where it looks and how far it is zoomed, moved by the player's
//! pans, zooms, and pinches, and every frame by momentum, easing, and scripted moves in
//! [`CameraManager::update`].

mod flight;
mod framing;
mod update;

pub use framing::view_offset;
pub use update::{CameraTuning, CameraUpdateParams};

use crate::math::Vec2;
use crate::reducer::ClientPhase;
use crate::utils::performance_now;
use common::models::CameraCue;
use common::types::{PieceId, TimestampMs};
use flight::CameraFlight;
use framing::{StartFraming, ZoomRange};
use std::rc::Rc;
use web_sys::HtmlCanvasElement;

/// Length of the step the camera tuning values are expressed per, in milliseconds.
///
/// The camera advances once per displayed frame, scaling its smoothing by how many of these
/// steps elapsed, so it moves at the same speed on any refresh rate.
pub const CAMERA_STEP_MS: f64 = 16.0;

/// Shortest gap between pointer moves used to measure pan momentum, in milliseconds, so two
/// moves in quick succession do not fling the camera.
const MIN_PAN_SAMPLE_MS: f64 = 2.0;

/// Damping exponent applied to pinch scale changes.
const PINCH_DAMPING: f64 = 0.8;

/// Two fingers on the screen, as of the last pinch step.
#[derive(Clone, Copy, Debug)]
struct Pinch {
    /// Distance between the fingers, in pixels.
    distance: f64,
    /// Client position of the midpoint between the fingers.
    center: Vec2,
}

impl Pinch {
    /// Returns the pinch of fingers at client positions `a` and `b`.
    fn of(a: Vec2, b: Vec2) -> Self {
        Self {
            distance: (b - a).length(),
            center: (a + b) * 0.5,
        }
    }
}

/// One life of the local player the camera follows, told apart from the next by its king and
/// join time.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TrackedLife {
    king_id: PieceId,
    join_time: TimestampMs,
}

/// Mutable camera state for smooth panning and zoom transitions.
///
/// Positions are in pixels of the board drawn at the current zoom, measured from the board
/// origin to the view centre.
pub struct CameraManager {
    camera: Vec2,
    target_camera: Vec2,
    zoom: f64,
    target_zoom: f64,
    /// Client position of the pointer, which wheel zooms keep still.
    mouse_pos: Vec2,
    /// Pan momentum, in pixels per camera step.
    velocity: Vec2,
    /// `performance.now()` of the last pan movement, used to drop stale momentum.
    last_pan_ms: f64,
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
    /// Life of the local player being followed, while alive.
    tracked_life: Option<TrackedLife>,
    /// Phase on the last update.
    last_phase: Option<ClientPhase>,
    /// Two-finger gesture in progress.
    pinch: Option<Pinch>,
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
            tracked_life: None,
            last_phase: None,
            pinch: None,
        }
    }

    /// Returns the camera position, in pixels from the board origin to the view centre.
    pub fn position(&self) -> Vec2 {
        self.camera
    }

    /// Returns the current zoom.
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Returns the last client position of the pointer.
    pub fn hovered(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Returns true while a scripted move is in progress and player input is ignored.
    pub fn input_locked(&self) -> bool {
        self.flight.is_some()
    }

    /// Records the client position of the pointer.
    pub fn hover(&mut self, pos: Vec2) {
        self.mouse_pos = pos;
    }

    /// Drops any pan momentum.
    pub fn stop_coasting(&mut self) {
        self.velocity = Vec2::ZERO;
    }

    /// Stops easing towards an earlier target, leaving the camera where it is.
    pub fn settle(&mut self) {
        self.target_camera = self.camera;
    }

    /// Drags the board by `delta` client pixels, measuring the momentum it keeps when let go.
    pub fn drag(&mut self, delta: Vec2, rotated: bool) {
        let movement = self.drag_view(delta, rotated);
        // Pointer moves arrive once per displayed frame, so the momentum is scaled to a
        // camera step to carry the same speed on any refresh rate.
        let now = performance_now();
        let elapsed_ms = (now - self.last_pan_ms).clamp(MIN_PAN_SAMPLE_MS, CAMERA_STEP_MS);
        self.velocity = movement * (CAMERA_STEP_MS / elapsed_ms);
        self.last_pan_ms = now;
    }

    /// Zooms by `factor` towards the pointer at client position `pos`, easing there over the
    /// next frames.
    pub fn zoom_towards(&mut self, pos: Vec2, factor: f64) {
        self.mouse_pos = pos;
        self.target_zoom = self.zoom_range.clamp(self.target_zoom * factor);
    }

    /// Starts a pinch with fingers at client positions `a` and `b`.
    pub fn begin_pinch(&mut self, a: Vec2, b: Vec2) {
        self.pinch = Some(Pinch::of(a, b));
        self.velocity = Vec2::ZERO;
    }

    /// Ends the pinch in progress, if any.
    pub fn end_pinch(&mut self) {
        self.pinch = None;
    }

    /// Moves the fingers of the pinch to client positions `a` and `b`: unless the camera is
    /// `locked`, pans by the moved midpoint and zooms by the changed distance, keeping the
    /// board under the midpoint still.
    pub fn pinch_to(
        &mut self,
        a: Vec2,
        b: Vec2,
        canvas: Option<&HtmlCanvasElement>,
        locked: bool,
        rotated: bool,
    ) {
        let pinch = Pinch::of(a, b);
        self.mouse_pos = pinch.center;
        self.velocity = Vec2::ZERO;
        let Some(previous) = self.pinch.replace(pinch) else {
            return;
        };
        if locked {
            return;
        }
        self.drag_view(pinch.center - previous.center, rotated);
        let factor = (pinch.distance / previous.distance).powf(PINCH_DAMPING);
        let zoom = self.zoom_range.clamp(self.zoom * factor);
        if let Some(canvas) = canvas {
            self.rescale_about(zoom / self.zoom, view_offset(canvas, pinch.center, rotated));
        }
        self.zoom = zoom;
        self.target_zoom = zoom;
        self.target_camera = self.camera;
    }

    /// Moves the camera as if the board was dragged `delta` client pixels, and returns the
    /// camera movement.
    fn drag_view(&mut self, delta: Vec2, rotated: bool) -> Vec2 {
        let movement = if rotated { delta } else { -delta };
        self.camera += movement;
        self.target_camera = self.camera;
        movement
    }

    /// Scales the camera and its target for a zoom changed by `ratio`, keeping the board
    /// point at `offset` from the view centre still.
    fn rescale_about(&mut self, ratio: f64, offset: Vec2) {
        // The board point under `offset` lies at `camera + offset` pixels at the old zoom and
        // `ratio` times that at the new one; the camera follows so the offset stays the same.
        self.camera = (self.camera + offset) * ratio - offset;
        self.target_camera = (self.target_camera + offset) * ratio - offset;
    }
}
