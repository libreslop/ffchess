use common::*;
use glam::IVec2;
use rand::Rng;

pub fn find_spawn_pos(game: &GameState) -> IVec2 {
    let mut rng = rand::thread_rng();
    let board_size = game.board_size;
    let half = board_size / 2;
    let limit = (board_size + 1) / 2;
    let margin = 3;

    for _ in 0..100 {
        let pos = IVec2::new(
            rng.gen_range(-half + margin..limit - margin),
            rng.gen_range(-half + margin..limit - margin),
        );
        let mut occupied = false;

        for piece in game.pieces.values() {
            if (piece.position - pos).as_vec2().length() < 10.0 {
                occupied = true;
                break;
            }
        }

        if !occupied {
            for shop in &game.shops {
                if (shop.position - pos).as_vec2().length() < 5.0 {
                    occupied = true;
                    break;
                }
            }
        }

        if !occupied {
            return pos;
        }
    }

    for _ in 0..100 {
        let pos = IVec2::new(
            rng.gen_range(-half + margin..limit - margin),
            rng.gen_range(-half + margin..limit - margin),
        );
        if !game.pieces.values().any(|p| p.position == pos)
            && !game.shops.iter().any(|s| s.position == pos)
        {
            return pos;
        }
    }

    IVec2::new(
        rng.gen_range(-half + margin..limit - margin),
        rng.gen_range(-half + margin..limit - margin),
    )
}
