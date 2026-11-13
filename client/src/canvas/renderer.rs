//! Canvas rendering of the board, shops, shading, move hints, pieces, and fog of war.

use super::color::hex_to_rgba;
use super::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgCache, RenderParams, Renderer};
use crate::math::{Vec2, vec2};
use common::models::{GameModeClientConfig, Piece};
use common::rules::{Catalog, LegalMove};
use common::types::{BoardCoord, ExprVars, PlayerId};
use glam::IVec2;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// Fill color around the board.
const OFF_BOARD_COLOR: &str = "#e2e8f0";
const BOARD_COLOR: &str = "#ffffff";
const DARK_SQUARE_COLOR: &str = "#f1f5f9";
const GRID_COLOR: &str = "#cbd5e1";
const BORDER_COLOR: &str = "#1e293b";
const DEFAULT_SHOP_COLOR: &str = "#fde047";
const DEFAULT_SHOP_FOCUS_COLOR: &str = "rgba(253, 224, 71, 0.35)";
const DEFAULT_HINT_COLOR: &str = "rgba(59, 130, 246, 0.4)";
const DEFAULT_LINE_COLOR: &str = "rgba(59, 130, 246, 0.5)";

/// Opacity of predicted piece positions.
const GHOST_ALPHA: f64 = 0.4;

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

    /// Maps a grid line coordinate, flipping the board when rotated.
    fn map_line(&self, line: i32) -> i32 {
        if self.rotated { -line } else { line }
    }

    /// Returns the top-left pixel of a fractional square position.
    pub(super) fn pos_origin(&self, pos: Vec2) -> Vec2 {
        self.map_pos(pos) * self.tile + self.offset
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

    /// Returns the pixel of a grid line intersection.
    fn line_point(&self, x: i32, y: i32) -> Vec2 {
        vec2(self.map_line(x) as f64, self.map_line(y) as f64) * self.tile + self.offset
    }
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
        self.draw_premove_lines(&params, &geometry);
        self.draw_pieces(&params, &geometry, &visibility);
        self.draw_shade_lines(&shades, &params, &geometry, &visibility);
        self.draw_capture_hints(&hints, &geometry);
        self.draw_king_names(&params, &geometry, &visibility);
        self.draw_fog(&params, &geometry, &visibility);
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

    /// Draws the board surface, checkerboard, grid lines, and border.
    fn draw_board(&self, params: &RenderParams<'_>, geometry: &FrameGeometry) {
        let board_size = params.state.board_size;
        let (low, high) = (-board_size.half(), board_size.limit_pos());
        let tile = geometry.tile;
        let corner = vec2(low as f64, low as f64) * tile + geometry.offset;
        let dim = board_size.as_i32() as f64 * tile;

        self.ctx.set_fill_style_str(BOARD_COLOR);
        self.ctx.fill_rect(corner.x, corner.y, dim, dim);

        self.ctx.set_fill_style_str(DARK_SQUARE_COLOR);
        for x in low..high {
            for y in low..high {
                if (x.rem_euclid(2) + y.rem_euclid(2)) % 2 != 0 {
                    let origin = geometry.square_origin(BoardCoord::new(x, y));
                    self.ctx.fill_rect(origin.x, origin.y, tile, tile);
                }
            }
        }

        self.ctx.set_stroke_style_str(GRID_COLOR);
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        for line in low..=high {
            for (from, to) in [
                (
                    geometry.line_point(line, low),
                    geometry.line_point(line, high),
                ),
                (
                    geometry.line_point(low, line),
                    geometry.line_point(high, line),
                ),
            ] {
                self.ctx.move_to(from.x, from.y);
                self.ctx.line_to(to.x, to.y);
            }
        }
        self.ctx.stroke();

        self.ctx.set_stroke_style_str(BORDER_COLOR);
        self.ctx.set_line_width(2.0);
        self.ctx.stroke_rect(corner.x, corner.y, dim, dim);
    }

    /// Draws shop squares and the highlight of the shop whose menu is open.
    fn draw_shops(
        &self,
        params: &RenderParams<'_>,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        let (tile, zoom) = (geometry.tile, geometry.zoom);
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
            let origin = geometry.square_origin(shop.position);
            self.ctx.set_fill_style_str(color);
            self.ctx.fill_rect(
                origin.x + 5.0 * zoom,
                origin.y + 5.0 * zoom,
                tile - 10.0 * zoom,
                tile - 10.0 * zoom,
            );
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
        let origin = geometry.square_origin(highlight);
        self.ctx.set_fill_style_str(color);
        self.ctx
            .fill_rect(origin.x + 2.0, origin.y + 2.0, tile - 4.0, tile - 4.0);
    }

    /// Lists the legal moves of the selected piece and the color to mark them in.
    fn move_hints(&self, params: &RenderParams<'_>) -> (Vec<LegalMove>, String) {
        let predicted = &params.projection.state;
        let Some(piece) = params
            .selected_piece_id
            .and_then(|id| predicted.pieces.get(&id))
        else {
            return (Vec::new(), DEFAULT_HINT_COLOR.to_string());
        };
        let color = piece
            .owner_id
            .and_then(|owner| params.state.players.get(&owner))
            .map_or_else(
                || DEFAULT_HINT_COLOR.to_string(),
                |player| hex_to_rgba(player.color.as_ref(), 0.4),
            );
        (self.catalog.legal_moves(predicted, piece), color)
    }

    /// Marks the destinations of moves that leave every other piece alone with a dot.
    fn draw_quiet_hints(
        &self,
        (hints, color): &(Vec<LegalMove>, String),
        geometry: &FrameGeometry,
    ) {
        self.ctx.set_fill_style_str(color);
        let radius = geometry.tile * 0.16;
        let targets = hints
            .iter()
            .filter(|legal| legal.affected.is_empty())
            .flat_map(LegalMove::handles);
        for target in targets {
            let center = geometry.square_center(*target);
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(center.x, center.y, radius, 0.0, std::f64::consts::TAU);
            self.ctx.fill();
        }
    }

    /// Frames the pieces that moves capture or carry along (such as the rook when castling)
    /// with four corner triangles, drawn above the pieces.
    fn draw_capture_hints(
        &self,
        (hints, color): &(Vec<LegalMove>, String),
        geometry: &FrameGeometry,
    ) {
        self.ctx.set_fill_style_str(color);
        let (tile, corner) = (geometry.tile, geometry.tile * 0.22);
        let targets = hints
            .iter()
            .filter(|legal| !legal.affected.is_empty())
            .flat_map(LegalMove::handles);
        for target in targets {
            let origin = geometry.square_origin(*target);
            for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let tip = origin
                    + vec2(
                        if dx > 0.0 { 0.0 } else { tile },
                        if dy > 0.0 { 0.0 } else { tile },
                    );
                self.ctx.begin_path();
                self.ctx.move_to(tip.x, tip.y);
                self.ctx.line_to(tip.x + dx * corner, tip.y);
                self.ctx.line_to(tip.x, tip.y + dy * corner);
                self.ctx.close_path();
                self.ctx.fill();
            }
        }
    }

    /// Draws a line for every predicted move, in the moving player's color.
    fn draw_premove_lines(&self, params: &RenderParams<'_>, geometry: &FrameGeometry) {
        for line in &params.projection.lines {
            let Some(piece) = params.state.pieces.get(&line.piece_id) else {
                continue;
            };
            let color = piece
                .owner_id
                .and_then(|owner| params.state.players.get(&owner))
                .map_or_else(
                    || DEFAULT_LINE_COLOR.to_string(),
                    |player| hex_to_rgba(player.color.as_ref(), 0.5),
                );
            let (from, to) = (
                geometry.square_center(line.start),
                geometry.square_center(line.target),
            );
            self.ctx.set_stroke_style_str(&color);
            self.ctx.set_line_width(2.0);
            self.ctx.begin_path();
            self.ctx.move_to(from.x, from.y);
            self.ctx.line_to(to.x, to.y);
            self.ctx.stroke();
        }
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
            offset: geometry.offset,
            alpha,
            state: params.state,
            is_ghost,
            pos: geometry.map_pos(square),
            tile_size: geometry.tile,
            zoom: geometry.zoom,
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
                offset: geometry.offset,
                state: params.state,
                zoom: geometry.zoom,
                tile_size: geometry.tile,
                pos: geometry.map_pos(square),
            });
        }
    }

    /// Fades everything outside the view radius around the local king to white.
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
        let _ = gradient.add_color_stop(0.0, "rgba(255, 255, 255, 0.0)");
        let _ = gradient.add_color_stop(1.0, "rgba(255, 255, 255, 1.0)");
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx
            .fill_rect(0.0, 0.0, params.canvas_size.x, params.canvas_size.y);
    }
}
