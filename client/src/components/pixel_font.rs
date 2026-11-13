//! Sizing for Upheaval, the pixel font the UI is set in.

use crate::utils::is_mobile;

/// Font size at which one Upheaval pixel is one screen pixel.
pub const NATIVE_PX: u32 = 20;

/// Trims a text box to the glyphs, which fill only the bottom of the font's line box, so
/// centering the box centers the visible text at any font size.
pub const TRIM_TO_GLYPHS: &str = "text-box: trim-both cap alphabetic;";

/// Returns the on-screen size of one pixel of the main screen and end screen titles.
pub fn title_pixel_px() -> u32 {
    if is_mobile() { 3 } else { 4 }
}
