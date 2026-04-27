//! Active shop selection logic for the game view.

use crate::math::{Vec2, vec2};
use crate::reducer::Pmove;
use common::models::{GameState, Piece, Shop, ShopConfig};
use common::types::{BoardCoord, PieceId, PlayerId, ShopId};
use std::collections::HashMap;

/// Inputs needed to resolve the nearest actionable shop under one player piece.
pub(super) struct ActiveShopMenuQuery<'a> {
    pub state: &'a GameState,
    pub ghosts: &'a HashMap<PieceId, Piece>,
    pub pm_queue: &'a [Pmove],
    pub shop_configs: &'a HashMap<ShopId, ShopConfig>,
    pub player_id: PlayerId,
    pub camera: Vec2,
    pub tile_size: f64,
    pub submitted_shop_actions: &'a [(PieceId, BoardCoord)],
}

impl ActiveShopMenuQuery<'_> {
    /// Resolves the currently focused shop, preferring the closest eligible one to the camera.
    pub fn resolve(&self) -> Option<(BoardCoord, ShopId, Piece)> {
        self.state
            .shops
            .iter()
            .filter_map(|shop| self.shop_candidate(shop))
            .min_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(pos, shop_id, piece, _)| (pos, shop_id, piece))
    }

    fn shop_candidate(&self, shop: &Shop) -> Option<(BoardCoord, ShopId, Piece, f64)> {
        let piece = self.ghosts.values().find(|piece| {
            piece.position == shop.position && piece.owner_id == Some(self.player_id)
        })?;
        let shop_config = self.shop_configs.get(&shop.shop_id)?;
        let group = common::logic::select_shop_group(shop_config, Some(piece))?;
        if group.items.is_empty() {
            return None;
        }
        if shop_config.auto_upgrade_single_item && group.items.len() == 1 {
            return None;
        }
        if self.has_pending_shop_action(piece, shop.position) {
            return None;
        }

        let piece_pos = vec2(
            piece.position.0.x as f64 * self.tile_size + self.tile_size / 2.0,
            piece.position.0.y as f64 * self.tile_size + self.tile_size / 2.0,
        );
        let dist_sq = (piece_pos - self.camera).length_squared();
        Some((shop.position, shop.shop_id.clone(), piece.clone(), dist_sq))
    }

    fn has_pending_shop_action(&self, piece: &Piece, shop_position: BoardCoord) -> bool {
        self.pm_queue.iter().any(|pm| {
            pm.shop_item_index.is_some() && pm.piece_id == piece.id && pm.target == shop_position.0
        }) || self
            .submitted_shop_actions
            .contains(&(piece.id, shop_position))
    }
}
