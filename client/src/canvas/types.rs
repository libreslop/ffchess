//! Data structures used by the canvas renderer.

use super::renderer::FrameGeometry;
use super::shading::ShadeCache;
use crate::math::Vec2;
use crate::premoves::{FailedMove, PremoveLine, Projection};
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
pub(super) struct PieceDrawParams<'a> {
    pub(super) piece: &'a Piece,
    pub(super) player_id: PlayerId,
    pub(super) geometry: FrameGeometry,
    pub(super) alpha: f64,
    pub(super) state: &'a GameState,
    pub(super) is_ghost: bool,
    /// Square position to draw at, already mapped for drawing, possibly fractional while
    /// animating.
    pub(super) pos: Vec2,
    pub(super) clock_offset_ms: i64,
}

/// Parameters for drawing a piece name overlay.
#[derive(Clone, Copy)]
pub(super) struct PieceNameDrawParams<'a> {
    pub(super) piece: &'a Piece,
    pub(super) geometry: FrameGeometry,
    pub(super) state: &'a GameState,
    /// Square position the name is drawn above, already mapped for drawing.
    pub(super) pos: Vec2,
}

/// A piece being dragged, drawn centered under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraggedPiece {
    pub piece_id: PieceId,
    /// Pointer position in canvas pixels.
    pub canvas_pos: Vec2,
}

/// A failed premove trail flashing after it disappeared.
#[derive(Clone, Debug, PartialEq)]
pub struct FailedTrail {
    /// The lines of the trail as last drawn.
    pub lines: Vec<PremoveLine>,
    /// The moves that failed, whose target squares flash with the trail.
    pub failed_moves: Vec<FailedMove>,
    /// True while the flash is black rather than the trail's color.
    pub dark: bool,
}

/// Speed lines behind a piece that just jumped from one square to another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveBlur {
    pub piece_id: PieceId,
    pub from: BoardCoord,
    pub to: BoardCoord,
    /// How far the lines have retracted towards `to`, from 0 to 1.
    pub progress: f64,
}

/// Parameters for a single render pass.
#[derive(Clone, Copy)]
pub struct RenderParams<'a> {
    pub state: &'a GameState,
    pub player_id: PlayerId,
    pub selected_piece_id: Option<PieceId>,
    pub dragged_piece: Option<DraggedPiece>,
    pub projection: &'a Projection,
    pub failed_trails: &'a [FailedTrail],
    pub move_blurs: &'a [MoveBlur],
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
