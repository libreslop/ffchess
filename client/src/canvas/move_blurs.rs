//! Pixelated speed lines behind pieces that just jumped to a new square.

use super::color::{PieceIconColors, lighten, piece_icon_colors};
use super::renderer::{ArtRect, FrameGeometry, Visibility};
use super::types::{MoveBlur, RenderParams, Renderer};
use crate::math::Vec2;
use glam::IVec2;
use std::collections::{HashMap, HashSet};

/// How far every speed line's shade is blended towards white, so the lines read as light
/// streaks with a hint of the team's color.
const SPEED_LINE_LIGHTEN: f64 = 0.4;

/// One of the shades of a team's color that its piece sprites are tinted with.
#[derive(Clone, Copy)]
enum Shade {
    Light,
    Mid,
    Dark,
}

impl Shade {
    /// Returns this shade of `colors`.
    fn of(self, colors: &PieceIconColors) -> &str {
        match self {
            Self::Light => &colors.primary,
            Self::Mid => &colors.secondary,
            Self::Dark => &colors.tertiary,
        }
    }
}

/// One speed line, running parallel to the move.
struct SpeedLine {
    /// Distance of the line to the side of the move's path, in art pixels.
    offset: f64,
    /// Fraction of the move the line starts short of, so the lines are staggered rather
    /// than ending in one row.
    lag: f64,
    shade: Shade,
}

/// The speed lines drawn behind every moved piece, spread across the width of a sprite.
const SPEED_LINES: [SpeedLine; 4] = [
    SpeedLine {
        offset: -5.0,
        lag: 0.45,
        shade: Shade::Light,
    },
    SpeedLine {
        offset: -2.0,
        lag: 0.1,
        shade: Shade::Mid,
    },
    SpeedLine {
        offset: 1.0,
        lag: 0.0,
        shade: Shade::Dark,
    },
    SpeedLine {
        offset: 4.0,
        lag: 0.25,
        shade: Shade::Light,
    },
];

impl MoveBlur {
    /// Returns the art pixels covered by the speed lines of this blur, with the shade of
    /// the line covering each.
    ///
    /// The lines run from the square the piece left to the center of the square it landed
    /// on, under its sprite, and retract towards it as the blur progresses.
    fn pixels(&self, geometry: &FrameGeometry) -> impl Iterator<Item = (Shade, IVec2)> {
        let (from, to) = (
            geometry.square_art_center(self.from),
            geometry.square_art_center(self.to),
        );
        let length = from.distance(to);
        let along = (to - from).normalize_or_zero();
        let side = along.perp();
        let retracted = length * self.progress;
        SPEED_LINES.iter().flat_map(move |line| {
            let tail = (retracted + length * line.lag).min(length);
            let shift = side * line.offset;
            line_pixels(from + along * tail + shift, to + shift).map(|pixel| (line.shade, pixel))
        })
    }
}

/// Returns the art pixels of a one pixel thick line from `start` to `end`, stepping one
/// pixel at a time along its longer axis.
fn line_pixels(start: Vec2, end: Vec2) -> impl Iterator<Item = IVec2> {
    let span = end - start;
    let steps = span.abs().max_element().round() as i32;
    (0..steps).map(move |step| {
        (start + span * (step as f64 / steps as f64))
            .floor()
            .as_ivec2()
    })
}

impl Renderer {
    /// Draws the speed lines of every visible moved piece in light shades of its team's
    /// color.
    ///
    /// Pixels are grouped by color so each color is set once and each pixel of it filled
    /// once.
    pub(super) fn draw_move_blurs(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        let mut pixels_by_color: HashMap<String, HashSet<IVec2>> = HashMap::new();
        for blur in params
            .move_blurs
            .iter()
            .filter(|blur| visibility.shows(blur.to))
        {
            let Some(piece) = params.state.pieces.get(&blur.piece_id) else {
                continue;
            };
            let colors = piece_icon_colors(&Self::team_color(piece, params.state));
            for (shade, pixel) in blur.pixels(geometry) {
                pixels_by_color
                    .entry(lighten(shade.of(&colors), SPEED_LINE_LIGHTEN))
                    .or_default()
                    .insert(pixel);
            }
        }
        for (color, pixels) in pixels_by_color {
            self.ctx.set_fill_style_str(&color);
            for pixel in pixels {
                let min = pixel.as_dvec2();
                self.fill_art_rect(
                    ArtRect {
                        min,
                        max: min + 1.0,
                    },
                    geometry,
                );
            }
        }
    }
}
