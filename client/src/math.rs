//! Math helpers and vector aliases for client coordinates.

use glam::DVec2;

/// Double-precision 2D vector for screen and world coordinates.
pub type Vec2 = DVec2;

/// Creates a `Vec2` from x/y components.
pub fn vec2(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}
