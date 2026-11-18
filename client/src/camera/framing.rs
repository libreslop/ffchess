//! How much of the board the screen can show, and where the camera starts: the zoom fitting
//! the whole board, the zoom range the camera keeps to, and the starting pose.

use super::flight::CameraPose;
use crate::math::Vec2;
use common::models::{GameModeClientConfig, GameState, MaxFitZoom, StartingCameraPosition};
use common::types::{BoardCoord, BoardSize, PlayerId};
use web_sys::HtmlCanvasElement;
use yew::NodeRef;

/// Zoom levels the camera may take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ZoomRange {
    min: f64,
    max: f64,
}

impl ZoomRange {
    /// Creates the range from `min` to `max`.
    pub(super) fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Returns the range widened to reach `zoom`.
    pub(super) fn including(self, zoom: f64) -> Self {
        Self {
            min: self.min.min(zoom),
            max: self.max.max(zoom),
        }
    }

    /// Returns the zoom of the range closest to `zoom`.
    pub(super) fn clamp(self, zoom: f64) -> f64 {
        zoom.clamp(self.min, self.max)
    }
}

/// Returns the zoom at which a board of `board_size` squares of `tile_size_px` pixels fits
/// in the part of the canvas on screen, centred on the view centre.
///
/// Views that slide the canvas partly off screen to leave room for a panel, like the replay
/// viewer, centre the board in the room left, so the fit leaves the panel uncovered too.
/// Falls back to zoom 1 until the canvas is laid out.
pub(super) fn max_fit_zoom(
    canvas_ref: &NodeRef,
    board_size: BoardSize,
    tile_size_px: f64,
) -> MaxFitZoom {
    visible_extent(canvas_ref)
        .map(|extent| extent.min_element() / (board_size.as_i32() as f64 * tile_size_px))
        .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
        .map_or(MaxFitZoom(1.0), MaxFitZoom)
}

/// Returns the size, in CSS pixels, of the largest on-screen area of the canvas centred on
/// its centre.
fn visible_extent(canvas_ref: &NodeRef) -> Option<Vec2> {
    let canvas = canvas_ref.cast::<HtmlCanvasElement>()?;
    let window = web_sys::window()?;
    let screen = Vec2::new(
        window.inner_width().ok()?.as_f64()?,
        window.inner_height().ok()?.as_f64()?,
    );
    let rect = canvas.get_bounding_client_rect();
    let low = Vec2::new(rect.left(), rect.top()).max(Vec2::ZERO);
    let high = Vec2::new(rect.right(), rect.bottom()).min(screen);
    let center = Vec2::new(
        rect.left() + rect.width() / 2.0,
        rect.top() + rect.height() / 2.0,
    );
    Some((center - low).min(high - center) * 2.0)
}

/// Returns the offset of the client position `pos` from the centre of `canvas`, in pixels of
/// the board as drawn, which is turned around when `rotated`.
pub fn view_offset(canvas: &HtmlCanvasElement, pos: Vec2, rotated: bool) -> Vec2 {
    let rect = canvas.get_bounding_client_rect();
    let canvas_center = Vec2::new(canvas.width() as f64, canvas.height() as f64) / 2.0;
    let offset = pos - Vec2::new(rect.left(), rect.top()) - canvas_center;
    if rotated { -offset } else { offset }
}

/// Returns the center of `square`, in squares from the board origin.
pub(super) fn square_center(square: BoardCoord) -> Vec2 {
    square.0.as_dvec2() + 0.5
}

/// Returns the centre of a board of `board_size`, in squares from the board origin.
pub(super) fn board_center(board_size: BoardSize) -> Vec2 {
    Vec2::splat((board_size.limit_pos() - board_size.half()) as f64 / 2.0)
}

/// Area of camera positions, in pixels, whose view centre lies on the board.
pub(super) struct CameraBounds {
    min: Vec2,
    max: Vec2,
}

impl CameraBounds {
    /// Returns the camera bounds of a board of `board_size` drawn with `tile_size` pixel
    /// squares.
    pub(super) fn of_board(board_size: BoardSize, tile_size: f64) -> Self {
        Self {
            min: Vec2::splat(-board_size.half() as f64 * tile_size),
            max: Vec2::splat(board_size.limit_pos() as f64 * tile_size),
        }
    }

    /// Returns the point of the bounds closest to `camera`.
    pub(super) fn clamp(&self, camera: Vec2) -> Vec2 {
        camera.clamp(self.min, self.max)
    }
}

/// Starting pose kept fitted to the screen.
#[derive(Clone, Copy, Debug)]
pub(super) struct StartFraming {
    /// Pose the camera was sent to.
    pub(super) pose: CameraPose,
    /// Fit zoom the pose was worked out for.
    pub(super) fit: MaxFitZoom,
}

/// Where the camera starts when a player joins or starts watching, as the mode configures.
pub(super) struct StartingCamera<'a> {
    pub(super) mode: Option<&'a GameModeClientConfig>,
    pub(super) state: &'a GameState,
    pub(super) viewer: PlayerId,
}

impl StartingCamera<'_> {
    /// Returns the point the camera centres on, or `fallback` when the mode names no piece
    /// the viewer owns.
    pub(super) fn center(&self, fallback: Vec2) -> Vec2 {
        let Some(mode) = self.mode else {
            return fallback;
        };
        match &mode.starting_camera.position {
            StartingCameraPosition::Point(point) => Vec2::new(point[0], point[1]),
            StartingCameraPosition::Piece { piece_id } => self
                .state
                .pieces_of(self.viewer)
                .find(|piece| piece.piece_type == *piece_id)
                .map_or(fallback, |piece| square_center(piece.position)),
        }
    }

    /// Returns the zoom the camera starts at on a screen fitting the board at `fit`.
    pub(super) fn zoom(&self, fit: MaxFitZoom) -> f64 {
        self.mode.map_or(1.0, |mode| mode.starting_camera.zoom(fit))
    }

    /// Returns true when the mode puts the camera back in the starting pose on every resize.
    pub(super) fn resets_on_resize(&self) -> bool {
        self.mode
            .is_some_and(|mode| mode.starting_camera.reset_on_resize)
    }
}
