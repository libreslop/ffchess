//! Drawing of piece shading: square backgrounds and threat marks configured per piece type.

use super::color::{NPC_PIECE_COLOR, hex_to_rgba};
use super::renderer::{ArtRect, FrameGeometry, Visibility};
use super::types::{RenderParams, Renderer};
use crate::math::Vec2;
use common::models::{GameState, Piece};
use common::rules::{PaintColor, Shade, ShadeMark};
use common::types::{OpacityPercent, PieceId, PlayerId};
use std::collections::HashMap;
use std::rc::Rc;

/// Distance from a piece's center to the inner end of its threat marks, in tiles.
const LINE_INNER_TILES: f64 = 0.36;
/// Distance from a piece's center to the outer end of its threat marks, in tiles.
const LINE_OUTER_TILES: f64 = 0.5;
/// Thickness of a threat mark at zoom 1, in pixels.
const LINE_WIDTH_PX: f64 = 3.0;

/// Shades resolved for one board and viewer, reused until either changes.
pub(crate) struct ShadeCache {
    pieces: HashMap<PieceId, Piece>,
    viewer: PlayerId,
    shades: Rc<Vec<Shade>>,
}

impl ShadeCache {
    /// Returns the cached shades when they were resolved for `state` and `viewer`.
    fn get(&self, state: &GameState, viewer: PlayerId) -> Option<Rc<Vec<Shade>>> {
        (self.viewer == viewer && self.pieces == state.pieces).then(|| self.shades.clone())
    }
}

/// Returns the CSS color of `paint` at `opacity`, looking team colors up in `state`.
fn paint_css(paint: &PaintColor, state: &GameState, opacity: OpacityPercent) -> String {
    let hex = paint.hex(state).map_or(NPC_PIECE_COLOR, AsRef::as_ref);
    hex_to_rgba(hex, opacity.alpha())
}

impl Renderer {
    /// Returns the shades the local player sees, resolving them only when the board changed.
    pub(super) fn shades(&self, params: &RenderParams<'_>) -> Rc<Vec<Shade>> {
        let mut cache = self.shade_cache.borrow_mut();
        if let Some(shades) = cache
            .as_ref()
            .and_then(|cache| cache.get(params.state, params.player_id))
        {
            return shades;
        }
        let shades = Rc::new(self.catalog.shading(params.state, params.player_id));
        *cache = Some(ShadeCache {
            pieces: params.state.pieces.clone(),
            viewer: params.player_id,
            shades: shades.clone(),
        });
        shades
    }

    /// Fills the squares of shaded pieces on the art pixel grid, drawn below the pieces.
    pub(super) fn draw_shade_backgrounds(
        &self,
        shades: &[Shade],
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        for (piece, color, opacity) in shades.iter().filter_map(|shade| match &shade.mark {
            ShadeMark::Background { color, opacity } => Some((shade.piece_id, color, *opacity)),
            ShadeMark::Line { .. } => None,
        }) {
            let Some(square) = Self::shown_square(params, visibility, piece) else {
                continue;
            };
            self.ctx
                .set_fill_style_str(&paint_css(color, params.state, opacity));
            self.fill_art_rect(
                ArtRect::square(geometry.pos_art_origin(square)),
                geometry,
            );
        }
    }

    /// Draws a short line segment on the rim of each shaded piece, pointing straight at a
    /// related piece, above the pieces.
    pub(super) fn draw_shade_lines(
        &self,
        shades: &[Shade],
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        self.ctx
            .set_line_width((LINE_WIDTH_PX * geometry.zoom).max(1.5));
        self.ctx.set_line_cap("round");
        for (piece, towards, color, opacity) in
            shades.iter().filter_map(|shade| match &shade.mark {
                ShadeMark::Line {
                    towards,
                    color,
                    opacity,
                } => Some((shade.piece_id, *towards, color, *opacity)),
                ShadeMark::Background { .. } => None,
            })
        {
            let Some(square) = Self::shown_square(params, visibility, piece) else {
                continue;
            };
            let Some(other) = params.state.pieces.get(&towards) else {
                continue;
            };
            let center = geometry.pos_center(square);
            let direction = geometry.pos_center(params.displayed_square(other)) - center;
            if direction == Vec2::ZERO {
                continue;
            }
            let unit = direction.normalize();
            let (inner, outer) = (
                center + unit * geometry.tile * LINE_INNER_TILES,
                center + unit * geometry.tile * LINE_OUTER_TILES,
            );
            self.ctx
                .set_stroke_style_str(&paint_css(color, params.state, opacity));
            self.ctx.begin_path();
            self.ctx.move_to(inner.x, inner.y);
            self.ctx.line_to(outer.x, outer.y);
            self.ctx.stroke();
        }
        self.ctx.set_line_cap("butt");
    }

    /// Returns where `piece` is drawn, or `None` when it is gone or out of view.
    fn shown_square(
        params: &RenderParams<'_>,
        visibility: &Visibility,
        piece: PieceId,
    ) -> Option<Vec2> {
        params
            .state
            .pieces
            .get(&piece)
            .filter(|piece| visibility.shows(piece.position))
            .map(|piece| params.displayed_square(piece))
    }
}
