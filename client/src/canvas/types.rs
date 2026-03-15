use crate::reducer::Pmove;
use common::models::{GameModeClientConfig, GameState, Piece, PieceConfig, ShopConfig};
use common::types::{PieceId, PieceTypeId, PlayerId, ShopId};
use std::collections::HashMap;
use web_sys::CanvasRenderingContext2d;

#[derive(Clone, PartialEq)]
pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub width: f64,
    pub height: f64,
    pub tile_size: f64,
    pub zoom: f64,
    pub piece_configs: HashMap<PieceTypeId, PieceConfig>,
    pub shop_configs: HashMap<ShopId, ShopConfig>,
}

pub struct PieceDrawParams<'a> {
    pub piece: &'a Piece,
    pub player_id: PlayerId,
    pub offset_x: f64,
    pub offset_y: f64,
    pub alpha: f64,
    pub state: &'a GameState,
    pub draw_name: bool,
    pub is_ghost: bool,
    pub pos_override: Option<(f64, f64)>,
}

/// Parameters for a single render pass.
pub struct RenderParams<'a> {
    pub state: &'a GameState,
    pub player_id: PlayerId,
    pub selected_piece_id: Option<PieceId>,
    pub pm_queue: &'a [Pmove],
    pub ghost_pieces: &'a HashMap<PieceId, Piece>,
    pub animated_positions: &'a HashMap<PieceId, (f64, f64)>,
    pub camera_pos: (f64, f64),
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    pub mode: Option<&'a GameModeClientConfig>,
    pub shop_configs: &'a HashMap<ShopId, ShopConfig>,
}
