//! Geometry and hit-testing helpers for the game view.

use crate::math::{Vec2, vec2};
use glam::IVec2;
use wasm_bindgen::JsCast;
use web_sys::{DomRect, Element, EventTarget, HtmlCanvasElement};

/// Reads the current browser window size in CSS pixels.
pub(super) fn read_window_size() -> Vec2 {
    let window = web_sys::window().expect("window available");
    let width = window
        .inner_width()
        .expect("window width")
        .as_f64()
        .expect("window width as f64");
    let height = window
        .inner_height()
        .expect("window height")
        .as_f64()
        .expect("window height as f64");
    vec2(width, height)
}

/// Converts a screen-space pointer position into a grid coordinate.
pub(super) fn screen_to_grid(
    pos: Vec2,
    rect: &DomRect,
    canvas: &HtmlCanvasElement,
    camera: Vec2,
    tile_size: f64,
    board_rotated_180: bool,
) -> IVec2 {
    let screen_pos = pos - vec2(rect.left(), rect.top());
    let canvas_center = vec2(canvas.width() as f64 / 2.0, canvas.height() as f64 / 2.0);
    let world_pos = if board_rotated_180 {
        camera + canvas_center - screen_pos
    } else {
        screen_pos + camera - canvas_center
    };
    let grid = (world_pos / tile_size).floor();
    IVec2::new(grid.x as i32, grid.y as i32)
}

/// Returns true when a pointer/touch event originates from exempt UI.
pub(super) fn is_ui_exempt_target(target: Option<EventTarget>) -> bool {
    let Some(target) = target else {
        return false;
    };
    let Ok(element) = target.dyn_into::<Element>() else {
        return false;
    };
    element
        .closest("[data-ui-exempt], [data-shop-ui], [data-chat-ui]")
        .ok()
        .flatten()
        .is_some()
}
