//! Active shop selection logic for the game view.

use crate::math::Vec2;
use crate::premoves::PremoveQueue;
use common::models::{GameState, Piece, Shop};
use common::rules::Catalog;
use common::types::{BoardCoord, PlayerId, ShopId};

/// The shop whose menu is shown and the piece that would buy from it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ActiveShop {
    pub(super) position: BoardCoord,
    pub(super) shop_id: ShopId,
    pub(super) piece: Piece,
}

/// Inputs needed to resolve the nearest actionable shop under one player piece.
pub(super) struct ActiveShopQuery<'a> {
    /// Board after every queued action, so shops reached by premoves can be used.
    pub(super) predicted: &'a GameState,
    pub(super) premoves: &'a PremoveQueue,
    pub(super) catalog: &'a Catalog,
    pub(super) player_id: PlayerId,
    pub(super) camera: Vec2,
    pub(super) tile_size: f64,
}

impl ActiveShopQuery<'_> {
    /// Resolves the shop whose menu to show, preferring the one closest to the camera.
    pub(super) fn resolve(&self) -> Option<ActiveShop> {
        self.predicted
            .shops
            .iter()
            .filter_map(|shop| self.candidate(shop))
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(shop, _)| shop)
    }

    /// Returns the shop with its camera distance when one of the player's pieces can buy there.
    ///
    /// Shops with only an automatic offer, or where a purchase is already queued as the
    /// piece's last action, show no menu.
    fn candidate(&self, shop: &Shop) -> Option<(ActiveShop, f64)> {
        let piece = self
            .predicted
            .piece_at(shop.position)
            .filter(|piece| piece.owner_id == Some(self.player_id))?;
        self.catalog
            .shops
            .get(&shop.shop_id)?
            .manual_items_for(piece)?;
        if self.premoves.ends_with_purchase(piece.id) {
            return None;
        }

        let center = (piece.position.0.as_dvec2() + 0.5) * self.tile_size;
        let distance = (center - self.camera).length_squared();
        Some((
            ActiveShop {
                position: shop.position,
                shop_id: shop.shop_id.clone(),
                piece: piece.clone(),
            },
            distance,
        ))
    }
}
