//! Shop purchase and spawn logic for a game instance.

use super::GameInstance;
use common::models::Shop;
use common::protocol::GameError;
use common::types::{BoardCoord, DurationMs, PieceId, PlayerId, Score, TimestampMs};

impl GameInstance {
    /// Processes a purchase at a shop and applies its effects.
    ///
    /// `player_id` is the buyer, `shop_pos` is the shop tile, `item_index` selects the item.
    /// Returns `Ok(())` on success or a `GameError` on failure.
    pub async fn handle_shop_buy(
        &self,
        player_id: PlayerId,
        shop_pos: BoardCoord,
        item_index: usize,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;
        let (shop_id, shop_index) = game
            .shops
            .iter()
            .enumerate()
            .find(|(_, s)| s.position == shop_pos)
            .map(|(i, s)| (s.shop_id.clone(), i))
            .ok_or(GameError::ShopNotFound)?;

        let shop_config = self
            .shop_configs
            .get(&shop_id)
            .ok_or_else(|| GameError::Internal("Shop config not found".to_string()))?;

        let player_piece_on_shop = game
            .pieces
            .values()
            .find(|p| p.position == shop_pos && p.owner_id == Some(player_id))
            .cloned();

        let group = common::logic::select_shop_group(shop_config, player_piece_on_shop.as_ref());

        let item = group
            .items
            .get(item_index)
            .ok_or(GameError::Internal("Invalid shop item index".to_string()))?;

        // Evaluate price
        let player_piece_count = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();
        let vars = common::logic::build_price_vars(
            player_piece_count,
            self.piece_configs.keys().map(|p_id| {
                let count = game
                    .pieces
                    .values()
                    .filter(|p| p.owner_id == Some(player_id) && &p.piece_type == p_id)
                    .count();
                (p_id, count)
            }),
        );

        let price = Score::from(common::logic::evaluate_expression(&item.price_expr, &vars) as u64);

        let player = game
            .players
            .get_mut(&player_id)
            .ok_or(GameError::PlayerNotFound)?;
        if player.score < price {
            return Err(GameError::InsufficientScore {
                needed: price,
                have: player.score,
            });
        }

        // Deduct score
        player.score -= price;

        // Apply item
        if let Some(ref replace_type) = item.replace_with
            && let Some(p) = player_piece_on_shop
            && let Some(piece) = game.pieces.get_mut(&p.id)
        {
            piece.piece_type = replace_type.clone();
            piece.cooldown_ms = self
                .piece_configs
                .get(replace_type)
                .map(|c| c.cooldown_ms)
                .unwrap_or_else(|| DurationMs::from_millis(1000));
        }

        for add_type in &item.add_pieces {
            let p_id = PieceId::new();
            let Some(p_pos) = crate::spawning::find_adjacent_free_pos(&game, shop_pos.into())
            else {
                return Err(GameError::NoSpaceNearby);
            };

            game.pieces.insert(
                p_id,
                common::models::Piece {
                    id: p_id,
                    owner_id: Some(player_id),
                    piece_type: add_type.clone(),
                    position: common::BoardCoord(p_pos),
                    last_move_time: TimestampMs::from_millis(0),
                    cooldown_ms: DurationMs::zero(),
                },
            );
        }

        // Deplete shop
        game.shops[shop_index].uses_remaining -= 1;
        if game.shops[shop_index].uses_remaining == 0 {
            game.shops.remove(shop_index);
            // Spawn a new one elsewhere
            let new_pos = crate::spawning::find_spawn_pos(&game);
            game.shops.push(Shop {
                position: common::BoardCoord(new_pos),
                uses_remaining: shop_config.default_uses,
                shop_id: shop_id.clone(),
            });
        }

        Ok(())
    }

    /// Spawns the initial set of shops defined by the mode configuration.
    ///
    /// Returns nothing; shops are inserted into the game state.
    pub async fn spawn_initial_shops(&self) {
        let mut game = self.game.write().await;
        for shop_count in &self.mode_config.shop_counts {
            for _ in 0..shop_count.count {
                let pos = crate::spawning::find_spawn_pos(&game);
                let shop_config = self.shop_configs.get(&shop_count.shop_id).unwrap();
                game.shops.push(Shop {
                    position: common::BoardCoord(pos),
                    uses_remaining: shop_config.default_uses,
                    shop_id: shop_count.shop_id.clone(),
                });
            }
        }
    }
}
