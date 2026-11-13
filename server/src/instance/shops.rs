//! Placement of shops on the board.

use super::world::World;
use crate::spawning::find_spawn_pos;
use common::models::Shop;
use common::types::{BoardCoord, ShopId};

impl World {
    /// Places the fixed and randomly positioned shops configured for the mode.
    pub(super) fn spawn_initial_shops(&mut self) {
        let setup = self.setup.clone();
        for fixed in &setup.mode.fixed_shops {
            self.place_shop(&fixed.shop_id, fixed.board_coord());
        }
        let mut rng = rand::thread_rng();
        for shop_count in &setup.mode.shop_counts {
            for _ in 0..shop_count.count {
                let pos = find_spawn_pos(&self.state, &mut rng);
                self.place_shop(&shop_count.shop_id, pos);
            }
        }
    }

    /// Replaces a depleted shop with a fresh one somewhere else.
    pub(super) fn respawn_shop(&mut self, shop_id: &ShopId) {
        let pos = find_spawn_pos(&self.state, &mut rand::thread_rng());
        self.place_shop(shop_id, pos);
    }

    /// Places a shop with its full number of uses, skipping unknown shop types.
    fn place_shop(&mut self, shop_id: &ShopId, position: BoardCoord) {
        match self.setup.catalog.shop(shop_id) {
            Ok(config) => self.state.shops.push(Shop {
                position,
                uses_remaining: config.default_uses,
                shop_id: shop_id.clone(),
            }),
            Err(error) => tracing::warn!(%shop_id, %error, "Skipping shop with unknown config"),
        }
    }
}
