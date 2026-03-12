use common::*;
use glam::IVec2;
use rand::Rng;
use uuid::Uuid;

pub fn spawn_npcs(game: &mut GameState) {
    let board_size = game.board_size;
    let half = board_size / 2;
    let limit = (board_size + 1) / 2;
    let target_npc_count = (board_size * board_size / 250).clamp(20, 200) as usize;

    let mut rng = rand::thread_rng();
    let current_npc_count = game
        .pieces
        .values()
        .filter(|p| p.owner_id.is_none())
        .count();

    if current_npc_count < target_npc_count {
        let id = Uuid::new_v4();
        let pos = IVec2::new(rng.gen_range(-half..limit), rng.gen_range(-half..limit));

        // Don't spawn NPC too close to any player
        let too_close = game
            .pieces
            .values()
            .any(|p| p.owner_id.is_some() && (p.position - pos).abs().max_element() <= 10);

        if !too_close {
            let p_type = match rng.gen_range(0..100) {
                0..=75 => PieceType::Pawn,
                76..=88 => PieceType::Knight,
                89..=96 => PieceType::Bishop,
                97..=99 => PieceType::Rook,
                _ => PieceType::Queen,
            };

            game.pieces.insert(
                id,
                Piece {
                    id,
                    owner_id: None,
                    piece_type: p_type,
                    position: pos,
                    last_move_time: 0,
                    cooldown_ms: 0,
                },
            );
        }
    }
}

pub fn spawn_initial_shops(game: &mut GameState) {
    for _ in 0..10 {
        spawn_random_shop(game);
    }
}

pub fn spawn_random_shop(game: &mut GameState) {
    let board_size = game.board_size;
    let mut rng = rand::thread_rng();
    let half = board_size / 2;
    let limit = (board_size + 1) / 2;
    game.shops.push(Shop {
        position: IVec2::new(rng.gen_range(-half..limit), rng.gen_range(-half..limit)),
        uses_remaining: 1, // Shops are now single-use
        shop_type: if rng.gen_bool(0.5) {
            ShopType::Spawn
        } else {
            ShopType::Upgrade
        },
    });
}
