//! Color conversion helpers for canvas drawing.

/// Derived color shades for rendering team-tinted piece icons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PieceIconColors {
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
}

/// Default color used for neutral NPC-owned piece rendering.
pub const NPC_PIECE_COLOR: &str = "#555555";

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

/// Builds lighter and darker shades for piece SVG tinting.
///
/// `base_hex` is the team color in hex. Returns primary/secondary/tertiary shades.
pub(crate) fn piece_icon_colors(base_hex: &str) -> PieceIconColors {
    let base = parse_hex_color(base_hex).unwrap_or((85, 85, 85));
    let primary = blend_hex(base, (255, 255, 255), 0.55);
    let secondary = blend_hex(base, (255, 255, 255), 0.25);
    let tertiary = blend_hex(base, (0, 0, 0), 0.2);
    PieceIconColors {
        primary,
        secondary,
        tertiary,
    }
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
        Some((r, g, b))
    } else {
        None
    }
}

fn blend_hex(base: (u8, u8, u8), target: (u8, u8, u8), amount: f64) -> String {
    let r = blend_channel(base.0, target.0, amount);
    let g = blend_channel(base.1, target.1, amount);
    let b = blend_channel(base.2, target.2, amount);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn blend_channel(base: u8, target: u8, amount: f64) -> u8 {
    let clamped = amount.clamp(0.0, 1.0);
    let base = base as f64;
    let target = target as f64;
    (base + (target - base) * clamped).round().clamp(0.0, 255.0) as u8
}
