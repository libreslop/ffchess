//! How much of the board the screen can show: the zoom fitting the whole board, and the zoom
//! range the camera keeps to.

use crate::math::Vec2;
use common::models::MaxFitZoom;
use common::types::BoardSize;
use yew::NodeRef;

/// Zoom levels the camera may take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomRange {
    min: f64,
    max: f64,
}

impl ZoomRange {
    /// Creates the range from `min` to `max`.
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Returns the range widened to reach `zoom`.
    pub fn including(self, zoom: f64) -> Self {
        Self {
            min: self.min.min(zoom),
            max: self.max.max(zoom),
        }
    }

    /// Returns the zoom of the range closest to `zoom`.
    pub fn clamp(self, zoom: f64) -> f64 {
        zoom.clamp(self.min, self.max)
    }
}

/// Returns the zoom at which a board of `board_size` squares of `tile_size_px` pixels fits
/// in the part of the canvas on screen, centred on the view centre.
///
/// Views that slide the canvas partly off screen to leave room for a panel, like the replay
/// viewer, centre the board in the room left, so the fit leaves the panel uncovered too.
/// Falls back to zoom 1 until the canvas is laid out.
pub fn max_fit_zoom(canvas_ref: &NodeRef, board_size: BoardSize, tile_size_px: f64) -> MaxFitZoom {
    visible_extent(canvas_ref)
        .map(|extent| extent.min_element() / (board_size.as_i32() as f64 * tile_size_px))
        .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
        .map_or(MaxFitZoom(1.0), MaxFitZoom)
}

/// Returns the size, in CSS pixels, of the largest on-screen area of the canvas centred on
/// its centre.
fn visible_extent(canvas_ref: &NodeRef) -> Option<Vec2> {
    let canvas = canvas_ref.cast::<web_sys::HtmlCanvasElement>()?;
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
