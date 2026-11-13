//! Data structures used by the canvas renderer.

use super::shading::ShadeCache;
use crate::math::Vec2;
use crate::premoves::Projection;
use common::models::{GameModeClientConfig, GameState, Piece};
use common::rules::Catalog;
use common::types::{BoardCoord, PieceId, PieceTypeId, PlayerId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

/// Canvas renderer with cached configs and drawing state.
#[derive(Clone)]
pub struct Renderer {
    pub(crate) ctx: CanvasRenderingContext2d,
    pub(crate) catalog: Rc<Catalog>,
    pub(crate) svg_cache: Rc<RefCell<PieceSvgCache>>,
    pub(crate) shade_cache: Rc<RefCell<Option<ShadeCache>>>,
}

/// Cached SVG templates and colored image instances for piece icons.
#[derive(Default)]
pub(crate) struct PieceSvgCache {
    pub(crate) templates: HashMap<PieceTypeId, String>,
    /// Piece types whose template is being fetched or failed to load.
    pub(crate) requested: HashSet<PieceTypeId>,
    pub(crate) images: HashMap<PieceSvgKey, HtmlImageElement>,
    pub(crate) rasters: HashMap<PieceSvgKey, HtmlCanvasElement>,
}

/// Cache key for a specific piece icon tinted with team colors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PieceSvgKey {
    pub(crate) piece_type: PieceTypeId,
    pub(crate) primary: String,
    pub(crate) secondary: String,
    pub(crate) tertiary: String,
}

/// Parameters for drawing a single piece.
#[derive(Clone, Copy)]
pub(crate) struct PieceDrawParams<'a> {
    pub piece: &'a Piece,
    pub player_id: PlayerId,
    pub offset: Vec2,
    pub alpha: f64,
    pub state: &'a GameState,
    pub is_ghost: bool,
    /// Square position to draw at, possibly fractional while animating.
    pub pos: Vec2,
    pub tile_size: f64,
    pub zoom: f64,
    pub clock_offset_ms: i64,
}

/// Parameters for drawing a piece name overlay.
#[derive(Clone, Copy)]
pub(crate) struct PieceNameDrawParams<'a> {
    pub piece: &'a Piece,
    pub offset: Vec2,
    pub state: &'a GameState,
    pub zoom: f64,
    pub tile_size: f64,
    pub pos: Vec2,
}

/// Parameters for a single render pass.
#[derive(Clone, Copy)]
pub struct RenderParams<'a> {
    pub state: &'a GameState,
    pub player_id: PlayerId,
    pub selected_piece_id: Option<PieceId>,
    pub projection: &'a Projection,
    pub animated_positions: &'a HashMap<PieceId, Vec2>,
    pub camera_pos: Vec2,
    pub canvas_size: Vec2,
    pub zoom: f64,
    pub tile_size_px: f64,
    pub mode: Option<&'a GameModeClientConfig>,
    pub board_rotated_180: bool,
    pub active_shop_highlight_pos: Option<BoardCoord>,
    pub disable_fog_of_war: bool,
    pub clock_offset_ms: i64,
}

impl RenderParams<'_> {
    /// Returns the square `piece` is drawn on, fractional while it slides.
    pub(crate) fn displayed_square(&self, piece: &Piece) -> Vec2 {
        self.animated_positions
            .get(&piece.id)
            .copied()
            .unwrap_or_else(|| piece.position.0.as_dvec2())
    }
}
