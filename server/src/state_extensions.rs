use common::*;
use crate::state::ServerState;
use uuid::Uuid;
use glam::IVec2;

impl ServerState {
    pub async fn handle_shop_buy(&self, player_id: Uuid, shop_pos: IVec2, piece_type: PieceType) -> Result<(), String> {
        let mut game = self.game.write().await;
        
        // 1. Check piece on shop (immutable)
        let piece_id = game.pieces.values()
            .find(|p| p.position == shop_pos && p.owner_id == Some(player_id))
            .map(|p| p.id)
            .ok_or("No piece on shop square")?;

        // 2. Get info (immutable)
        let player_piece_count = game.pieces.values().filter(|p| p.owner_id == Some(player_id)).count();
        let cost = get_upgrade_cost(piece_type, player_piece_count);
        let shop_idx = game.shops.iter().position(|s| s.position == shop_pos).ok_or("Shop not found")?;
        let shop_type = game.shops[shop_idx].shop_type.clone();

        if game.shops[shop_idx].uses_remaining == 0 {
            return Err("Shop is depleted".to_string());
        }

        // 3. Mutate
        let player = game.players.get_mut(&player_id).ok_or("Player not found")?;
        if player.score < cost {
            return Err(format!("Insufficient score. Need {}, have {}", cost, player.score));
        }

        player.score -= cost;
        game.shops[shop_idx].uses_remaining -= 1;

        match shop_type {
            ShopType::Upgrade => {
                if let Some(p) = game.pieces.get_mut(&piece_id) {
                    p.piece_type = piece_type;
                }
            }
            ShopType::Spawn => {
                let board_size = game.board_size;
                let new_piece_id = Uuid::new_v4();
                let neighbors = [
                    IVec2::new(1, 0), IVec2::new(-1, 0), IVec2::new(0, 1), IVec2::new(0, -1)
                ];
                let mut spawn_pos = shop_pos;
                for n in neighbors {
                    let p = shop_pos + n;
                    if is_within_board(p, board_size) && !game.pieces.values().any(|pc| pc.position == p) {
                        spawn_pos = p;
                        break;
                    }
                }
                
                game.pieces.insert(new_piece_id, Piece {
                    id: new_piece_id,
                    owner_id: Some(player_id),
                    piece_type,
                    position: spawn_pos,
                    last_move_time: 0,
                    cooldown_ms: 0,
                });
            }
        }

        if game.shops[shop_idx].uses_remaining == 0 {
            game.shops.remove(shop_idx);
            Self::spawn_random_shop(&mut game);
        }

        Ok(())
    }
}
