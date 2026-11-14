//! Canvas rendering of the board, shops, shading, move hints, pieces, and fog of war.

use super::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgCache, RenderParams, Renderer};
use crate::math::Vec2;
use common::models::{GameModeClientConfig, Piece};
use common::rules::Catalog;
use common::types::{BoardCoord, ExprVars, PlayerId};
use glam::IVec2;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// Fill color around the board, matching the dark squares so the light rim marks the edge.
const OFF_BOARD_COLOR: &str = "#413931";
const LIGHT_SQUARE_COLOR: &str = "#ffd5b4";
const DARK_SQUARE_COLOR: &str = "#413931";
/// Color the view fades into beyond the fog-of-war radius, the off-board color as rgb
/// components.
const FOG_RGB: &str = "65, 57, 49";
const DEFAULT_SHOP_COLOR: &str = "#fde047";
const DEFAULT_SHOP_FOCUS_COLOR: &str = "rgba(253, 224, 71, 0.35)";

/// Size of the art pixel grid everything on the board snaps to, in art pixels per tile.
///
/// Piece sprites are this many pixels across, so their pixels match the board's, the move
/// hints', the shops' and the premove arrows'.
pub(super) const ART_PIXELS_PER_TILE: f64 = 16.0;
/// Width of the ring each square is framed with in the other square color, in art pixels.
const SQUARE_RIM_PIXELS: f64 = 1.0;
/// Width of the light rim around the whole board, in art pixels.
const BOARD_RIM_PIXELS: f64 = 1.0;
/// Inset of a shop's mark from the edge of its square, in art pixels.
const SHOP_INSET_PIXELS: f64 = 2.0;
/// Inset of the open shop's highlight from the edge of its square, in art pixels.
const SHOP_FOCUS_INSET_PIXELS: f64 = 1.0;

/// Opacity of predicted piece positions.
const GHOST_ALPHA: f64 = 0.4;

/// Size of a piece being dragged relative to pieces on the board.
const DRAGGED_SCALE: f64 = 1.2;
/// Color of the hard drop shadow under a piece being dragged.
const DRAGGED_SHADOW_COLOR: &str = "rgba(15, 23, 42, 0.3)";
/// Offset of the drop shadow down and right of a dragged piece, in art pixels.
const DRAGGED_SHADOW_ART_PIXELS: f64 = 1.5;

/// Things this many squares beyond the view radius are still drawn so they fade into the fog.
const VIEW_MARGIN_SQUARES: i32 = 2;

/// Fog radius used when no mode is known, scaled by army size.
const FALLBACK_FOG_RADIUS: f64 = 15.0;

/// Maps board coordinates to screen pixels for one frame.
#[derive(Clone, Copy)]
pub(super) struct FrameGeometry {
    pub(super) tile: f64,
    pub(super) zoom: f64,
    offset: Vec2,
    rotated: bool,
}

impl FrameGeometry {
    /// Maps a square to its on-screen square, flipping the board when rotated.
    fn map_square(&self, square: IVec2) -> IVec2 {
        if self.rotated {
            -square - IVec2::ONE
        } else {
            square
        }
    }

    /// Maps a fractional square position, flipping the board when rotated.
    fn map_pos(&self, pos: Vec2) -> Vec2 {
        if self.rotated { -pos - Vec2::ONE } else { pos }
    }

    /// Returns the fractional square position, already mapped for drawing, of a square
    /// centered on `pixel`.
    fn pixel_pos(&self, pixel: Vec2) -> Vec2 {
        (pixel - self.offset) / self.tile - 0.5
    }

    /// Returns the top-left pixel of a fractional square position already mapped for drawing.
    pub(super) fn mapped_origin(&self, mapped: Vec2) -> Vec2 {
        mapped * self.tile + self.offset
    }

    /// Returns the top-left pixel of a fractional square position.
    pub(super) fn pos_origin(&self, pos: Vec2) -> Vec2 {
        self.mapped_origin(self.map_pos(pos))
    }

    /// Returns the center pixel of a fractional square position.
    pub(super) fn pos_center(&self, pos: Vec2) -> Vec2 {
        self.pos_origin(pos) + self.tile / 2.0
    }

    /// Returns the top-left pixel of a square.
    fn square_origin(&self, square: BoardCoord) -> Vec2 {
        self.map_square(square.0).as_dvec2() * self.tile + self.offset
    }

    /// Returns the center pixel of a square.
    fn square_center(&self, square: BoardCoord) -> Vec2 {
        self.square_origin(square) + self.tile / 2.0
    }

    /// Returns the top-left corner of a square, in art pixels counted from the board origin.
    ///
    /// Squares span whole art pixels, so shapes built from them never depend on the zoom
    /// level.
    pub(super) fn square_art_origin(&self, square: BoardCoord) -> Vec2 {
        self.map_square(square.0).as_dvec2() * ART_PIXELS_PER_TILE
    }

    /// Returns the top-left corner of a fractional square position, already mapped for
    /// drawing, rounded to whole art pixels counted from the board origin.
    ///
    /// Things drawn there, such as sliding pieces, stay on the art pixel grid.
    pub(super) fn mapped_art_origin(&self, mapped: Vec2) -> Vec2 {
        (mapped * ART_PIXELS_PER_TILE).round()
    }

    /// Returns the top-left corner of a fractional square position, rounded to whole art
    /// pixels counted from the board origin.
    pub(super) fn pos_art_origin(&self, pos: Vec2) -> Vec2 {
        self.mapped_art_origin(self.map_pos(pos))
    }

    /// Returns the center of a square, in art pixels counted from the board origin.
    pub(super) fn square_art_center(&self, square: BoardCoord) -> Vec2 {
        self.square_art_origin(square) + ART_PIXELS_PER_TILE / 2.0
    }

    /// Converts art pixels to screen pixels, possibly fractional.
    fn exact_screen_point(&self, art: Vec2) -> Vec2 {
        art * self.tile / ART_PIXELS_PER_TILE + self.offset
    }

    /// Converts art pixels to the nearest screen pixel.
    fn screen_point(&self, art: Vec2) -> Vec2 {
        self.exact_screen_point(art).round()
    }

    /// Returns the screen pixels covering `rect`.
    ///
    /// Both edges are rounded, so neighbouring rectangles meet without gaps or overlaps.
    pub(super) fn screen_rect(&self, rect: ArtRect) -> ScreenRect {
        let min = self.screen_point(rect.min);
        ScreenRect {
            min,
            size: self.screen_point(rect.max) - min,
        }
    }

    /// Returns the exact, possibly fractional, screen area of `rect`.
    fn exact_screen_rect(&self, rect: ArtRect) -> ScreenRect {
        let min = self.exact_screen_point(rect.min);
        ScreenRect {
            min,
            size: self.exact_screen_point(rect.max) - min,
        }
    }

    /// Returns true when any part of `rect` lies on a canvas of `canvas_size`.
    fn shows(&self, rect: ArtRect, canvas_size: Vec2) -> bool {
        let screen = self.screen_rect(rect);
        let max = screen.min + screen.size;
        max.x > 0.0 && max.y > 0.0 && screen.min.x < canvas_size.x && screen.min.y < canvas_size.y
    }
}

/// A rectangle of whole art pixels, from its top-left to its bottom-right corner.
#[derive(Clone, Copy)]
pub(super) struct ArtRect {
    pub(super) min: Vec2,
    pub(super) max: Vec2,
}

impl ArtRect {
    /// Returns the rectangle of a whole square whose top-left corner is `origin`.
    pub(super) fn square(origin: Vec2) -> Self {
        Self {
            min: origin,
            max: origin + ART_PIXELS_PER_TILE,
        }
    }

    /// Returns the rectangle shrunk by `pixels` on every side.
    pub(super) fn inset(self, pixels: f64) -> Self {
        Self {
            min: self.min + pixels,
            max: self.max - pixels,
        }
    }
}

/// A rectangle of whole screen pixels.
#[derive(Clone, Copy)]
pub(super) struct ScreenRect {
    pub(super) min: Vec2,
    pub(super) size: Vec2,
}

/// The part of the board the local player can see.
#[derive(Clone, Copy)]
pub(super) struct Visibility {
    center: BoardCoord,
    radius_squares: i32,
    radius_px: f64,
    fogged: bool,
}

impl Visibility {
    /// Returns true when `pos` is close enough to the view center to be drawn.
    pub(super) fn shows(&self, pos: BoardCoord) -> bool {
        pos.chebyshev_distance(self.center) <= self.radius_squares + VIEW_MARGIN_SQUARES
    }
}

/// Evaluates the fog-of-war radius for a player owning `piece_count` pieces.
///
/// Returns `None` when the mode has no fog of war.
fn fog_radius(mode: Option<&GameModeClientConfig>, piece_count: usize) -> Option<f64> {
    match mode {
        Some(mode) => {
            let vars = ExprVars::new().with("player_piece_count", piece_count);
            mode.fog_of_war_radius
                .as_ref()
                .map(|expr| expr.evaluate(&vars))
        }
        None => Some(FALLBACK_FOG_RADIUS * (piece_count as f64).sqrt().max(1.0)),
    }
}

impl Renderer {
    /// Creates a renderer drawing onto `canvas` with pieces from `catalog`.
    pub fn new(canvas: HtmlCanvasElement, catalog: Rc<Catalog>) -> Option<Self> {
        let ctx = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .ok()?;
        Some(Self {
            ctx,
            catalog,
            svg_cache: Rc::new(RefCell::new(PieceSvgCache::default())),
            shade_cache: Rc::new(RefCell::new(None)),
        })
    }

    /// Renders a full frame including board, shops, pieces, and fog of war.
    pub fn draw(&self, params: RenderParams<'_>) {
        let tile = params.tile_size_px * params.zoom;
        let canvas_center = params.canvas_size * 0.5;
        let geometry = FrameGeometry {
            tile,
            zoom: params.zoom,
            offset: if params.board_rotated_180 {
                canvas_center + params.camera_pos
            } else {
                canvas_center - params.camera_pos
            },
            rotated: params.board_rotated_180,
        };
        let visibility = self.visibility(&params, tile);

        self.ctx.set_fill_style_str(OFF_BOARD_COLOR);
        self.ctx
            .fill_rect(0.0, 0.0, params.canvas_size.x, params.canvas_size.y);
        self.draw_board(&params, &geometry);
        self.draw_shops(&params, &geometry, &visibility);
        let shades = self.shades(&params);
        self.draw_shade_backgrounds(&shades, &params, &geometry, &visibility);
        let hints = self.move_hints(&params);
        self.draw_quiet_hints(&hints, &geometry);
        self.draw_premove_arrows(&params, &geometry);
        self.draw_pieces(&params, &geometry, &visibility);
        self.draw_shade_lines(&shades, &params, &geometry, &visibility);
        self.draw_capture_hints(&hints, &geometry);
        self.draw_king_names(&params, &geometry, &visibility);
        self.draw_fog(&params, &geometry, &visibility);
        self.draw_dragged_piece(&params, &geometry);
    }

    /// Works out what the local player sees: everything, or a fogged circle around their king.
    fn visibility(&self, params: &RenderParams<'_>, tile: f64) -> Visibility {
        let state = params.state;
        let king = state.king_of(params.player_id);
        let fog = fog_radius(params.mode, state.pieces_of(params.player_id).count());
        let fogged = !params.disable_fog_of_war
            && fog.is_some()
            && !params.player_id.is_nil()
            && king.is_some();
        let radius_squares = if fogged {
            fog.unwrap_or_default().max(0.0) as i32
        } else {
            state.board_size.as_i32()
        };
        Visibility {
            center: king.map_or_else(BoardCoord::default, |king| king.position),
            radius_squares,
            radius_px: (radius_squares as f64 + 0.5) * tile,
            fogged,
        }
    }

    /// Draws the board, antialiased: a light rim around the whole board, and each square
    /// framed by a ring in the other square color.
    ///
    /// Only squares on the canvas are drawn.
    fn draw_board(&self, params: &RenderParams<'_>, geometry: &FrameGeometry) {
        let board_size = params.state.board_size;
        let (low, high) = (-board_size.half(), board_size.limit_pos());
        // Corner squares swap places when the board is rotated.
        let (first, last) = (
            geometry.square_art_origin(BoardCoord::new(low, low)),
            geometry.square_art_origin(BoardCoord::new(high - 1, high - 1)),
        );
        let board = ArtRect {
            min: first.min(last),
            max: first.max(last) + ART_PIXELS_PER_TILE,
        };
        self.ctx.set_fill_style_str(LIGHT_SQUARE_COLOR);
        self.fill_smooth_art_rect(board.inset(-BOARD_RIM_PIXELS), geometry);

        // Dark squares keep a rim of the light board underneath; light squares are filled
        // dark, then light inside their rim.
        let squares: Vec<(ArtRect, bool)> = (low..high)
            .flat_map(|x| (low..high).map(move |y| BoardCoord::new(x, y)))
            .map(|square| {
                let dark = (square.0.x + square.0.y).rem_euclid(2) != 0;
                (ArtRect::square(geometry.square_art_origin(square)), dark)
            })
            .filter(|(rect, _)| geometry.shows(*rect, params.canvas_size))
            .collect();
        self.ctx.set_fill_style_str(DARK_SQUARE_COLOR);
        for (rect, dark) in &squares {
            let fill = if *dark {
                rect.inset(SQUARE_RIM_PIXELS)
            } else {
                *rect
            };
            self.fill_smooth_art_rect(fill, geometry);
        }
        self.ctx.set_fill_style_str(LIGHT_SQUARE_COLOR);
        for (rect, _) in squares.iter().filter(|(_, dark)| !dark) {
            self.fill_smooth_art_rect(rect.inset(SQUARE_RIM_PIXELS), geometry);
        }
    }

    /// Draws shop squares and the highlight of the shop whose menu is open.
    fn draw_shops(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        for shop in params
            .state
            .shops
            .iter()
            .filter(|shop| visibility.shows(shop.position))
        {
            let color = self
                .catalog
                .shops
                .get(&shop.shop_id)
                .and_then(|config| config.color.as_deref())
                .unwrap_or(DEFAULT_SHOP_COLOR);
            let square = ArtRect::square(geometry.square_art_origin(shop.position));
            self.ctx.set_fill_style_str(color);
            self.fill_art_rect(square.inset(SHOP_INSET_PIXELS), geometry);
        }

        let Some(highlight) = params
            .active_shop_highlight_pos
            .filter(|pos| visibility.shows(*pos))
        else {
            return;
        };
        let color = params
            .state
            .shop_at(highlight)
            .and_then(|shop| self.catalog.shops.get(&shop.shop_id))
            .and_then(|config| config.focus_color.as_deref().or(config.color.as_deref()))
            .unwrap_or(DEFAULT_SHOP_FOCUS_COLOR);
        let square = ArtRect::square(geometry.square_art_origin(highlight));
        self.ctx.set_fill_style_str(color);
        self.fill_art_rect(square.inset(SHOP_FOCUS_INSET_PIXELS), geometry);
    }

    /// Fills the screen pixels covering `rect`.
    ///
    /// Both edges are rounded, so neighbouring rectangles meet without gaps or overlaps and
    /// translucent colors do not stack.
    pub(super) fn fill_art_rect(&self, rect: ArtRect, geometry: &FrameGeometry) {
        self.fill_screen_rect(geometry.screen_rect(rect));
    }

    /// Fills the exact screen area of `rect`, antialiasing edges that fall between screen
    /// pixels, so every art pixel keeps the same apparent size at any zoom.
    ///
    /// Only for opaque colors whose rectangles never share an edge with another of the same
    /// color, which would show a faint seam.
    fn fill_smooth_art_rect(&self, rect: ArtRect, geometry: &FrameGeometry) {
        self.fill_screen_rect(geometry.exact_screen_rect(rect));
    }

    /// Fills `screen`.
    fn fill_screen_rect(&self, screen: ScreenRect) {
        self.ctx
            .fill_rect(screen.min.x, screen.min.y, screen.size.x, screen.size.y);
    }

    /// Draws every visible piece, plus a faded copy where queued actions will move it.
    ///
    /// Pieces whose queued actions only change them in place (such as an upgrade) are drawn
    /// as predicted, with their real cooldown.
    fn draw_pieces(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        let predicted = &params.projection.state.pieces;
        for (id, real) in &params.state.pieces {
            let ghost = predicted.get(id);
            let shown = match ghost {
                Some(ghost) if ghost.position != real.position => {
                    if visibility.shows(ghost.position) {
                        self.draw_piece(self.piece_params(
                            params,
                            geometry,
                            ghost,
                            GHOST_ALPHA,
                            true,
                            None,
                        ));
                    }
                    real.clone()
                }
                Some(ghost) => Piece {
                    last_move_time: real.last_move_time,
                    cooldown_ms: real.cooldown_ms,
                    ..ghost.clone()
                },
                None => real.clone(),
            };
            if visibility.shows(shown.position) {
                let animated = params.animated_positions.get(id).copied();
                self.draw_piece(self.piece_params(params, geometry, &shown, 1.0, false, animated));
            }
        }
    }

    /// Draws a lifted copy of the dragged piece, as predicted, under the pointer: slightly
    /// enlarged and casting a hard, unblurred drop shadow offset by whole screen pixels to
    /// match the pixel art.
    fn draw_dragged_piece(&self, params: &RenderParams<'_>, geometry: &FrameGeometry) {
        let Some(dragged) = params.dragged_piece else {
            return;
        };
        let Some(piece) = params.projection.state.pieces.get(&dragged.piece_id) else {
            return;
        };
        let lifted = FrameGeometry {
            tile: geometry.tile * DRAGGED_SCALE,
            zoom: geometry.zoom * DRAGGED_SCALE,
            ..*geometry
        };
        self.ctx.save();
        let shadow_offset = (DRAGGED_SHADOW_ART_PIXELS * lifted.tile / ART_PIXELS_PER_TILE)
            .round()
            .max(1.0);
        self.ctx.set_shadow_color(DRAGGED_SHADOW_COLOR);
        self.ctx.set_shadow_blur(0.0);
        self.ctx.set_shadow_offset_x(shadow_offset);
        self.ctx.set_shadow_offset_y(shadow_offset);
        self.draw_piece(PieceDrawParams {
            pos: lifted.pixel_pos(dragged.canvas_pos),
            ..self.piece_params(params, &lifted, piece, 1.0, true, None)
        });
        self.ctx.restore();
    }

    /// Builds the draw parameters of one piece.
    fn piece_params<'a>(
        &self,
        params: &RenderParams<'a>,
        geometry: &FrameGeometry,
        piece: &'a Piece,
        alpha: f64,
        is_ghost: bool,
        animated: Option<Vec2>,
    ) -> PieceDrawParams<'a> {
        let square = animated.unwrap_or_else(|| piece.position.0.as_dvec2());
        PieceDrawParams {
            piece,
            player_id: params.player_id,
            geometry: *geometry,
            alpha,
            state: params.state,
            is_ghost,
            pos: geometry.map_pos(square),
            clock_offset_ms: params.clock_offset_ms,
        }
    }

    /// Draws player names above kings, on top of everything else on the board.
    fn draw_king_names(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        for king in params.state.pieces.values() {
            if !king.piece_type.is_king() || !visibility.shows(king.position) {
                continue;
            }
            let square = params.displayed_square(king);
            self.draw_piece_name(PieceNameDrawParams {
                piece: king,
                geometry: *geometry,
                state: params.state,
                pos: geometry.map_pos(square),
            });
        }
    }

    /// Fades everything outside the view radius around the local king into the off-board color.
    fn draw_fog(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        if !visibility.fogged || params.player_id == PlayerId::nil() {
            return;
        }
        let center = geometry.square_center(visibility.center);
        let radius = visibility.radius_px;
        let Ok(gradient) = self.ctx.create_radial_gradient(
            center.x,
            center.y,
            radius * 0.6,
            center.x,
            center.y,
            radius,
        ) else {
            return;
        };
        let _ = gradient.add_color_stop(0.0, &format!("rgba({FOG_RGB}, 0.0)"));
        let _ = gradient.add_color_stop(1.0, &format!("rgba({FOG_RGB}, 1.0)"));
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx
            .fill_rect(0.0, 0.0, params.canvas_size.x, params.canvas_size.y);
    }
}
