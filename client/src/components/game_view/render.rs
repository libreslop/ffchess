//! Drawing the board after every render.

use super::anims::PieceAnimations;
use super::shop_context::ActiveShop;
use crate::canvas::{DraggedPiece, RenderParams, Renderer};
use crate::math::Vec2;
use crate::premoves::Projection;
use crate::reducer::GameStateReducer;
use crate::utils::performance_now;
use common::types::PieceId;
use std::cell::RefCell;
use std::rc::Rc;
use yew::hook;
use yew::prelude::*;

/// Everything one frame of the board needs.
pub(super) struct BoardFrame {
    pub(super) renderer: Option<Renderer>,
    pub(super) reducer: UseReducerHandle<GameStateReducer>,
    pub(super) projection: Rc<Projection>,
    pub(super) anims: Rc<RefCell<PieceAnimations>>,
    pub(super) active_shop: Option<ActiveShop>,
    pub(super) selected: Option<PieceId>,
    pub(super) dragged: Option<DraggedPiece>,
    pub(super) camera: Vec2,
    pub(super) zoom: f64,
    pub(super) canvas_size: Vec2,
    pub(super) tile_size_px: f64,
}

/// Starts slide animations whenever the authoritative pieces move.
#[hook]
pub(super) fn use_piece_animations(
    reducer: &UseReducerHandle<GameStateReducer>,
) -> Rc<RefCell<PieceAnimations>> {
    let anims = use_mut_ref(PieceAnimations::default);
    {
        let anims = anims.clone();
        use_effect_with(reducer.state.pieces.clone(), move |pieces| {
            anims.borrow_mut().observe(pieces, performance_now());
        });
    }
    anims
}

/// Redraws the board after every render of the view.
#[hook]
pub(super) fn use_board_render(frame: BoardFrame) {
    use_effect(move || {
        if let Some(renderer) = &frame.renderer {
            frame.draw(renderer);
        }
    });
}

impl BoardFrame {
    /// Draws this frame with `renderer`.
    fn draw(&self, renderer: &Renderer) {
        let reducer = &self.reducer;
        let has_match_result = reducer.has_match_result();
        let animated_positions = self.anims.borrow_mut().positions_at(performance_now());
        renderer.draw(RenderParams {
            state: &reducer.state,
            player_id: reducer.viewer_id(),
            selected_piece_id: self.selected.filter(|_| !has_match_result),
            dragged_piece: self.dragged.filter(|_| !has_match_result),
            projection: &self.projection,
            animated_positions: &animated_positions,
            camera_pos: self.camera,
            canvas_size: self.canvas_size,
            zoom: self.zoom,
            tile_size_px: self.tile_size_px,
            mode: reducer.mode.as_ref(),
            board_rotated_180: reducer.board_rotated_180(),
            active_shop_highlight_pos: self.active_shop.as_ref().map(|shop| shop.position),
            disable_fog_of_war: has_match_result,
            clock_offset_ms: reducer.clock_offset_ms,
        });
    }
}
