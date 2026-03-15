//! Color conversion helpers for canvas drawing.

/// Converts a hex color string into an rgba() CSS string.
///
/// `hex` is a `#rgb` or `#rrggbb` string, `alpha` is the opacity.
/// Returns an `rgba(r, g, b, a)` CSS color string.
pub fn hex_to_rgba(hex: &str, alpha: f64) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        format!("rgba({}, {}, {}, {})", r, g, b, alpha)
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
        format!("rgba({}, {}, {}, {})", r, g, b, alpha)
    } else {
        format!("rgba(0, 0, 0, {})", alpha)
    }
}
