use common::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub width: f64,
    pub height: f64,
    pub tile_size: f64,
    pub zoom: f64,
}

pub struct PieceDrawParams<'a> {
    pub piece: &'a Piece,
    pub player_id: Uuid,
    pub offset_x: f64,
    pub offset_y: f64,
    pub alpha: f64,
    pub state: &'a GameState,
    pub draw_name: bool,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement, zoom: f64) -> Self {
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        Self {
            ctx,
            width: canvas.width() as f64,
            height: canvas.height() as f64,
            tile_size: 40.0 * zoom,
            zoom,
        }
    }
}
