//! Pixelated arrows tracing each piece's queued moves.

use super::color::{ARROW_ALPHA, arrow_color};
use super::renderer::{ArtRect, FrameGeometry};
use super::types::{RenderParams, Renderer};
use crate::math::{Vec2, vec2};
use crate::premoves::PremoveLine;
use common::types::PieceId;

/// Color of a failed premove trail during the dark phases of its flash.
const FAILED_TRAIL_DARK_COLOR: &str = "#000000";
/// Thickness of a premove arrow's shaft, in art pixels.
const ARROW_SHAFT_PIXELS: f64 = 4.0;
/// Width of a premove arrow's head at its base, in art pixels.
///
/// Twice the head length makes a right-angled tip, whose sides run along pixel rows and
/// columns on diagonal arrows.
const ARROW_HEAD_WIDTH_PIXELS: f64 = 12.0;
/// Length of a premove arrow's head, in art pixels.
const ARROW_HEAD_LENGTH_PIXELS: f64 = 6.0;

/// One queued move of a piece, between two square centers in art pixels.
#[derive(Clone, Copy)]
struct Leg {
    from: Vec2,
    to: Vec2,
}

impl Leg {
    /// Returns true when `point` lies within half a shaft's thickness of the leg.
    ///
    /// The ends are round, so consecutive legs join without notches at the turns.
    fn shaft_contains(&self, point: Vec2) -> bool {
        let span = self.to - self.from;
        let t = if span == Vec2::ZERO {
            0.0
        } else {
            ((point - self.from).dot(span) / span.length_squared()).clamp(0.0, 1.0)
        };
        point.distance(self.from + span * t) <= ARROW_SHAFT_PIXELS / 2.0
    }
}

/// The triangular head of an arrow, pointing along its last leg.
struct ArrowHead {
    base: Vec2,
    along: Vec2,
    length: f64,
}

impl ArrowHead {
    /// Places the head at the end of `leg`, shortened to fit legs shorter than a head.
    fn ending(leg: Leg) -> Self {
        let along = (leg.to - leg.from).normalize_or_zero();
        let length = ARROW_HEAD_LENGTH_PIXELS.min(leg.from.distance(leg.to));
        Self {
            base: leg.to - along * length,
            along,
            length,
        }
    }

    /// Returns the point the head ends at.
    fn tip(&self) -> Vec2 {
        self.base + self.along * self.length
    }

    /// Returns true when `point` lies inside the head.
    fn contains(&self, point: Vec2) -> bool {
        let offset = point - self.base;
        let depth = offset.dot(self.along);
        let spread = offset.dot(self.along.perp()).abs();
        self.length > 0.0
            && (0.0..=self.length).contains(&depth)
            && spread <= ARROW_HEAD_WIDTH_PIXELS / 2.0 * (self.length - depth) / self.length
    }
}

/// An arrow through all of a piece's queued moves, with its head on the last one.
///
/// It is laid out in art pixels, so it keeps its shape at every zoom level.
struct PixelArrow {
    /// The legs in queue order, the last one stopping at the head's base.
    shafts: Vec<Leg>,
    head: ArrowHead,
}

impl PixelArrow {
    /// Builds the arrow through `legs`, or `None` when there are none.
    fn new(mut legs: Vec<Leg>) -> Option<Self> {
        let last = legs.last_mut()?;
        let head = ArrowHead::ending(*last);
        last.to = head.base;
        Some(Self { shafts: legs, head })
    }

    /// Returns true when `point` lies inside the arrow.
    fn contains(&self, point: Vec2) -> bool {
        self.head.contains(point) || self.shafts.iter().any(|leg| leg.shaft_contains(point))
    }

    /// Returns the whole art pixels that can hold any part of the arrow.
    fn bounds(&self) -> ArtRect {
        let (min, max) = self
            .shafts
            .iter()
            .flat_map(|leg| [leg.from, leg.to])
            .chain([self.head.tip()])
            .fold(
                (Vec2::splat(f64::INFINITY), Vec2::splat(f64::NEG_INFINITY)),
                |(min, max), point| (min.min(point), max.max(point)),
            );
        let reach = ARROW_HEAD_WIDTH_PIXELS / 2.0;
        ArtRect {
            min: (min - reach).floor(),
            max: (max + reach).ceil(),
        }
    }

    /// Returns the art pixels whose centers fall inside the arrow, merged into one rectangle
    /// per run along each row.
    fn runs(&self) -> Vec<ArtRect> {
        let bounds = self.bounds();
        let cols = (bounds.max.x - bounds.min.x) as usize;
        let rows = (bounds.max.y - bounds.min.y) as usize;
        let mut runs = Vec::new();
        for row in 0..rows {
            let y = bounds.min.y + row as f64;
            let mut run_start = None;
            for col in 0..=cols {
                let x = bounds.min.x + col as f64;
                let filled = col < cols && self.contains(vec2(x, y) + 0.5);
                match (filled, run_start) {
                    (true, None) => run_start = Some(x),
                    (false, Some(start)) => {
                        runs.push(ArtRect {
                            min: vec2(start, y),
                            max: vec2(x, y + 1.0),
                        });
                        run_start = None;
                    }
                    _ => {}
                }
            }
        }
        runs
    }
}

/// Builds one arrow per piece through its `lines`, which come in queue order.
fn piece_arrows(lines: &[PremoveLine], geometry: &FrameGeometry) -> Vec<(PieceId, PixelArrow)> {
    let mut paths: Vec<(PieceId, Vec<Leg>)> = Vec::new();
    for line in lines {
        let leg = Leg {
            from: geometry.square_art_center(line.start),
            to: geometry.square_art_center(line.target),
        };
        match paths.iter_mut().find(|(id, _)| *id == line.piece_id) {
            Some((_, legs)) => legs.push(leg),
            None => paths.push((line.piece_id, vec![leg])),
        }
    }
    paths
        .into_iter()
        .filter_map(|(piece_id, legs)| Some((piece_id, PixelArrow::new(legs)?)))
        .collect()
}

impl Renderer {
    /// Draws one pixelated arrow per piece through all its predicted moves, each piece in a
    /// color of its own, then the flashes of failed trails.
    ///
    /// Each arrow is filled as non-overlapping runs, so its translucent color does not stack.
    pub(super) fn draw_premove_arrows(&self, params: &RenderParams<'_>, geometry: &FrameGeometry) {
        for (piece_id, arrow) in piece_arrows(&params.projection.lines, geometry) {
            self.fill_arrow(
                &arrow,
                &arrow_color(piece_id.0.as_u128(), ARROW_ALPHA),
                geometry,
            );
        }
        for trail in params.failed_trails {
            for (piece_id, arrow) in piece_arrows(&trail.lines, geometry) {
                let color = if trail.dark {
                    FAILED_TRAIL_DARK_COLOR.to_string()
                } else {
                    arrow_color(piece_id.0.as_u128(), 1.0)
                };
                self.fill_arrow(&arrow, &color, geometry);
            }
        }
    }

    /// Fills `arrow` with `color`.
    fn fill_arrow(&self, arrow: &PixelArrow, color: &str, geometry: &FrameGeometry) {
        self.ctx.set_fill_style_str(color);
        for run in arrow.runs() {
            self.fill_art_rect(run, geometry);
        }
    }
}
