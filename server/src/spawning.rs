//! Spawn position helpers for pieces and shops.

use common::*;
use glam::IVec2;
use rand::Rng;

/// Check whether a board position is in-bounds and unoccupied by pieces or shops.
///
/// `game` provides state, `pos` is the candidate tile. Returns `true` if free.
pub fn is_free_position(game: &GameState, pos: IVec2) -> bool {
    common::logic::is_within_board(common::BoardCoord(pos), game.board_size)
        && !game.pieces.values().any(|p| p.position == pos)
        && !game.shops.iter().any(|s| s.position == pos)
}

/// Find a nearby open position using a fixed list of offsets.
///
/// `game` provides state and `origin` is the center tile. Returns a free position if found.
pub fn find_adjacent_free_pos(game: &GameState, origin: IVec2) -> Option<IVec2> {
    const OFFSETS: [IVec2; 8] = [
        IVec2::new(1, 0),
        IVec2::new(-1, 0),
        IVec2::new(0, 1),
        IVec2::new(0, -1),
        IVec2::new(1, 1),
        IVec2::new(-1, 1),
        IVec2::new(1, -1),
        IVec2::new(-1, -1),
    ];
    OFFSETS
        .iter()
        .map(|offset| origin + *offset)
        .find(|pos| is_free_position(game, *pos))
}

/// Find a random nearby open position within the provided offset bounds.
///
/// `game` provides state, `origin` is the center tile, `rng` is the RNG to use,
/// `offset_range` bounds random offsets, and `attempts` is the max tries.
/// Returns a free position if found.
pub fn find_random_nearby_free_pos(
    game: &GameState,
    origin: IVec2,
    rng: &mut impl Rng,
    offset_range: std::ops::RangeInclusive<i32>,
    attempts: usize,
) -> Option<IVec2> {
    for _ in 0..attempts {
        let offset = IVec2::new(
            rng.gen_range(offset_range.clone()),
            rng.gen_range(offset_range.clone()),
        );
        let candidate = origin + offset;
        if candidate != origin && is_free_position(game, candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Finds a suitable spawn position with distance-based heuristics.
///
/// `game` provides the current board state. Returns a spawnable tile.
pub fn find_spawn_pos(game: &GameState) -> IVec2 {
    let mut rng = rand::thread_rng();
    let board_size = game.board_size;
    let half = board_size.half();
    let limit = board_size.limit_pos();
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
