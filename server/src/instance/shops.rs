use super::GameInstance;
use common::models::Shop;
use common::protocol::GameError;
use common::types::{PieceId, PlayerId};
use glam::IVec2;

impl GameInstance {
    pub async fn handle_shop_buy(
        &self,
        player_id: PlayerId,
        shop_pos: IVec2,
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

        let group = if let Some(ref p) = player_piece_on_shop {
            shop_config
                .groups
                .iter()
                .find(|g| g.applies_to.contains(&p.piece_type))
                .unwrap_or(&shop_config.default_group)
        } else {
            &shop_config.default_group
        };

        let item = group
            .items
            .get(item_index)
            .ok_or(GameError::Internal("Invalid shop item index".to_string()))?;

        // Evaluate price
        let mut vars = std::collections::HashMap::new();
        vars.insert(
            "player_piece_count".to_string(),
            game.pieces
                .values()
                .filter(|p| p.owner_id == Some(player_id))
                .count() as f64,
        );
        // Add specific piece counts
        for p_id in self.piece_configs.keys() {
            let count = game
                .pieces
                .values()
                .filter(|p| p.owner_id == Some(player_id) && &p.piece_type == p_id)
                .count();
            vars.insert(format!("{}_count", p_id.as_ref()), count as f64);
        }

        let price = common::logic::evaluate_expression(&item.price_expr, &vars) as u64;

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
                .map(|c| c.cooldown_ms as i64)
                .unwrap_or(1000);
        }

        for add_type in &item.add_pieces {
            let p_id = PieceId::new();
            let mut p_pos = shop_pos;
            // Find nearby space
            let neighbors = [
                IVec2::new(1, 0),
                IVec2::new(-1, 0),
                IVec2::new(0, 1),
                IVec2::new(0, -1),
                IVec2::new(1, 1),
                IVec2::new(-1, 1),
                IVec2::new(1, -1),
                IVec2::new(-1, -1),
            ];
            let mut found = false;
            for offset in neighbors {
                let candidate = shop_pos + offset;
                if common::logic::is_within_board(candidate, game.board_size)
                    && !game.pieces.values().any(|p| p.position == candidate)
                    && !game.shops.iter().any(|s| s.position == candidate)
                {
                    p_pos = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(GameError::NoSpaceNearby);
            }

            game.pieces.insert(
                p_id,
                common::models::Piece {
                    id: p_id,
                    owner_id: Some(player_id),
                    piece_type: add_type.clone(),
                    position: p_pos,
                    last_move_time: 0,
                    cooldown_ms: 0,
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
                position: new_pos,
                uses_remaining: shop_config.default_uses,
                shop_id: shop_id.clone(),
            });
        }

        Ok(())
    }

    pub async fn spawn_initial_shops(&self) {
        let mut game = self.game.write().await;
        for shop_count in &self.mode_config.shop_counts {
            for _ in 0..shop_count.count {
                let pos = crate::spawning::find_spawn_pos(&game);
                let shop_config = self.shop_configs.get(&shop_count.shop_id).unwrap();
                game.shops.push(Shop {
                    position: pos,
                    uses_remaining: shop_config.default_uses,
                    shop_id: shop_count.shop_id.clone(),
                });
            }
        }
    }
}
