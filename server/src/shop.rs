use crate::npc::spawning::spawn_random_shop;
use crate::state::ServerState;
use common::*;
use glam::IVec2;
use uuid::Uuid;

impl ServerState {
    pub async fn handle_shop_buy(
        &self,
        player_id: Uuid,
        shop_pos: IVec2,
        piece_type: PieceType,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;

        // 1. Find the player's piece on the shop square
        let piece_id = game
            .pieces
            .values()
            .find(|p| p.position == shop_pos && p.owner_id == Some(player_id))
            .map(|p| p.id)
            .ok_or(GameError::NoPieceOnShop)?;

        let is_king = game
            .pieces
            .get(&piece_id)
            .map(|p| p.piece_type == PieceType::King)
            .unwrap_or(false);

        if is_king && piece_type != PieceType::Pawn {
            return Err(GameError::KingRestrictedShop);
        }

        // 2. Cost calculation
        let player_piece_count = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();
        let cost = get_upgrade_cost(piece_type, player_piece_count);
        let shop_idx = game
            .shops
            .iter()
            .position(|s| s.position == shop_pos)
            .ok_or(GameError::ShopNotFound)?;

        if game.shops[shop_idx].uses_remaining == 0 {
            return Err(GameError::ShopDepleted);
        }

        // 3. Score check
        let player = game
            .players
            .get_mut(&player_id)
            .ok_or(GameError::PlayerNotFound)?;
        if player.score < cost {
            return Err(GameError::InsufficientScore {
                needed: cost,
                have: player.score,
            });
        }

        // 4. Execution
        player.score -= cost;
        game.shops[shop_idx].uses_remaining -= 1;

        if piece_type == PieceType::Pawn {
            // Spawn logic: Find nearest free square
            let board_size = game.board_size;
            let mut spawn_pos = None;

            // Search in expanding rings
            'outer: for r in 1..5 {
                for dx in -r..=r {
                    let dx: i32 = dx;
                    for dy in -r..=r {
                        let dy: i32 = dy;
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        let p = shop_pos + IVec2::new(dx, dy);
                        if is_within_board(p, board_size)
                            && !game.pieces.values().any(|pc| pc.position == p)
                        {
                            spawn_pos = Some(p);
                            break 'outer;
                        }
                    }
                }
            }

            let final_spawn_pos = spawn_pos.ok_or(GameError::NoSpaceNearby)?;
            let new_id = Uuid::new_v4();
            game.pieces.insert(
                new_id,
                Piece {
                    id: new_id,
                    owner_id: Some(player_id),
                    piece_type: PieceType::Pawn,
                    position: final_spawn_pos,
                    last_move_time: 0,
                    cooldown_ms: 0,
                },
            );
        } else {
            // Upgrade logic: Replace current piece
            if let Some(p) = game.pieces.get_mut(&piece_id) {
                p.piece_type = piece_type;
            }
        }

        // 5. Cleanup depleted shop
        if game.shops[shop_idx].uses_remaining == 0 {
            game.shops.remove(shop_idx);
            spawn_random_shop(&mut game);
        }

        Ok(())
    }
}
