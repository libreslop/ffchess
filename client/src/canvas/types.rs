use common::models::{GameState, Piece, PieceConfig, ShopConfig};
use std::collections::HashMap;
use uuid::Uuid;
use web_sys::CanvasRenderingContext2d;

#[derive(Clone, PartialEq)]
pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub width: f64,
    pub height: f64,
    pub tile_size: f64,
    pub zoom: f64,
    pub piece_configs: HashMap<String, PieceConfig>,
    pub shop_configs: HashMap<String, ShopConfig>,
}

pub struct PieceDrawParams<'a> {
    pub piece: &'a Piece,
    pub player_id: Uuid,
    pub offset_x: f64,
    pub offset_y: f64,
    pub alpha: f64,
    pub state: &'a GameState,
    pub draw_name: bool,
    pub is_ghost: bool,
    pub pos_override: Option<(f64, f64)>,
}
