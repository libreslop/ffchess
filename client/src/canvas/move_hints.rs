//! Marks on the squares the selected piece can move to.

use super::color::hex_to_rgba;
use super::renderer::{ART_PIXELS_PER_TILE, ArtRect, FrameGeometry};
use super::types::{RenderParams, Renderer};
use crate::math::{Vec2, vec2};
use common::rules::LegalMove;
use common::types::{BoardCoord, TimestampMs};

const DEFAULT_HINT_COLOR: &str = "rgba(59, 130, 246, 0.4)";
/// Side of the square marking a quiet move, in art pixels.
const QUIET_HINT_PIXELS: f64 = 4.0;
/// Number of stair steps in each corner of a capture mark.
const CAPTURE_HINT_STEPS: usize = 4;

/// Legal moves of the selected piece and the color they are marked in.
pub(super) struct MoveHints {
    moves: Vec<LegalMove>,
    color: String,
}

impl MoveHints {
    /// Returns the squares to mark for moves that capture or carry along other pieces when
    /// `captures` is true, or for moves that leave every other piece alone otherwise.
    fn targets(&self, captures: bool) -> impl Iterator<Item = &BoardCoord> {
        self.moves
            .iter()
            .filter(move |legal| legal.affected.is_empty() != captures)
            .flat_map(LegalMove::handles)
    }
}

/// Returns the art pixel rows of the staircase in one corner of a square, the widest row
/// against the corner.
///
/// `origin` is the square's top-left corner in art pixels. Rows never overlap, so the
/// translucent color does not stack.
fn capture_corner(origin: Vec2, from_left: bool, from_top: bool) -> impl Iterator<Item = ArtRect> {
    let far = origin + ART_PIXELS_PER_TILE;
    (0..CAPTURE_HINT_STEPS).map(move |step| {
        let width = (CAPTURE_HINT_STEPS - step) as f64;
        let x = if from_left { origin.x } else { far.x - width };
        let y = if from_top {
            origin.y + step as f64
        } else {
            far.y - step as f64 - 1.0
        };
        ArtRect {
            min: vec2(x, y),
            max: vec2(x + width, y + 1.0),
        }
    })
}

impl Renderer {
    /// Lists the legal moves of the selected piece and the color to mark them in.
    pub(super) fn move_hints(&self, params: &RenderParams<'_>) -> MoveHints {
        let predicted = &params.projection.state;
        let Some(piece) = params
            .selected_piece_id
            .and_then(|id| predicted.pieces.get(&id))
        else {
            return MoveHints {
                moves: Vec::new(),
                color: DEFAULT_HINT_COLOR.to_string(),
            };
        };
        let color = piece
            .owner_id
            .and_then(|owner| params.state.players.get(&owner))
            .map_or_else(
                || DEFAULT_HINT_COLOR.to_string(),
                |player| hex_to_rgba(player.color.as_ref(), 0.4),
            );
        let now = TimestampMs::from_millis(js_sys::Date::now() as i64 + params.clock_offset_ms);
        let timing = params.projection.move_timing(piece, now);
        MoveHints {
            moves: self.catalog.legal_moves(predicted, piece, timing),
            color,
        }
    }

    /// Marks the destinations of moves that leave every other piece alone with a small square.
    pub(super) fn draw_quiet_hints(&self, hints: &MoveHints, geometry: &FrameGeometry) {
        self.ctx.set_fill_style_str(&hints.color);
        for target in hints.targets(false) {
            let center = geometry.square_art_center(*target);
            let rect = ArtRect {
                min: center - QUIET_HINT_PIXELS / 2.0,
                max: center + QUIET_HINT_PIXELS / 2.0,
            };
            self.fill_art_rect(rect, geometry);
        }
    }

    /// Frames the pieces that moves capture or carry along (such as the rook when castling)
    /// with a pixel staircase in each corner, drawn above the pieces.
    pub(super) fn draw_capture_hints(&self, hints: &MoveHints, geometry: &FrameGeometry) {
        self.ctx.set_fill_style_str(&hints.color);
        for target in hints.targets(true) {
            let origin = geometry.square_art_origin(*target);
            let rows = [(true, true), (false, true), (true, false), (false, false)]
                .into_iter()
                .flat_map(|(from_left, from_top)| capture_corner(origin, from_left, from_top));
            for rect in rows {
                self.fill_art_rect(rect, geometry);
            }
        }
    }
}
