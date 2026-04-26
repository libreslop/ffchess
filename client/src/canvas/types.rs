//! Data structures used by the canvas renderer.

use crate::math::Vec2;
use crate::reducer::Pmove;
use common::models::{GameModeClientConfig, GameState, Piece, PieceConfig, ShopConfig};
use common::types::{PieceId, PieceTypeId, PlayerId, ShopId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

/// Canvas renderer with cached configs and drawing state.
#[derive(Clone)]
pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub piece_configs: HashMap<PieceTypeId, PieceConfig>,
    pub(crate) svg_cache: Rc<RefCell<PieceSvgCache>>,
}

/// Cached SVG templates and colored image instances for piece icons.
#[derive(Default)]
pub(crate) struct PieceSvgCache {
    pub(crate) templates: HashMap<PieceTypeId, String>,
    pub(crate) pending: HashSet<PieceTypeId>,
    pub(crate) images: HashMap<PieceSvgKey, HtmlImageElement>,
    pub(crate) rasters: HashMap<PieceSvgKey, HtmlCanvasElement>,
}

impl PieceSvgCache {
    /// Creates an empty SVG cache.
    pub(crate) fn new() -> Self {
        Self::default()
    }
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
pub struct PieceDrawParams<'a> {
    pub piece: &'a Piece,
    pub player_id: PlayerId,
    pub offset_x: f64,
    pub offset_y: f64,
    pub alpha: f64,
    pub state: &'a GameState,
    pub draw_name: bool,
    pub is_ghost: bool,
    pub pos_override: Option<Vec2>,
    pub tile_size_px: f64,
    pub clock_offset_ms: i64,
}

/// Parameters for drawing a piece name overlay.
#[derive(Clone, Copy)]
pub struct PieceNameDrawParams<'a> {
    pub piece: &'a Piece,
    pub offset_x: f64,
    pub offset_y: f64,
    pub alpha: f64,
    pub state: &'a GameState,
    pub zoom: f64,
    pub tile_size_px: f64,
    pub pos_override: Option<Vec2>,
}

/// Parameters for a single render pass.
#[derive(Clone, Copy)]
pub struct RenderParams<'a> {
    pub state: &'a GameState,
    pub player_id: PlayerId,
    pub selected_piece_id: Option<PieceId>,
    pub pm_queue: &'a [Pmove],
    pub ghost_pieces: &'a HashMap<PieceId, Piece>,
    pub animated_positions: &'a HashMap<PieceId, Vec2>,
    pub camera_pos: Vec2,
    pub canvas_size: Vec2,
    pub zoom: f64,
    pub tile_size_px: f64,
    pub mode: Option<&'a GameModeClientConfig>,
    pub board_rotated_180: bool,
    pub shop_configs: &'a HashMap<ShopId, ShopConfig>,
    pub disable_fog_of_war: bool,
    pub clock_offset_ms: i64,
}
