//! Spawn position heuristics for pieces and shops.

use common::models::GameState;
use common::types::BoardCoord;
use glam::IVec2;
use rand::Rng;
use std::ops::Range;

/// Distance from the board edge kept clear of spawns when the board is large enough.
const EDGE_MARGIN: i32 = 3;

/// Spawns try to stay at least this far from any piece.
const PIECE_CLEARANCE: f32 = 10.0;

/// Spawns try to stay at least this far from any shop.
const SHOP_CLEARANCE: f32 = 5.0;

/// Random attempts per spawn heuristic.
const SPAWN_ATTEMPTS: usize = 100;

/// Returns the coordinate range spawns are drawn from on each axis.
///
/// Keeps an edge margin when the board is large enough, otherwise uses the whole board.
fn spawn_range(state: &GameState) -> Range<i32> {
    let (low, high) = (-state.board_size.half(), state.board_size.limit_pos());
    if high - low > 2 * EDGE_MARGIN {
        low + EDGE_MARGIN..high - EDGE_MARGIN
    } else {
        low..high
    }
}

/// Picks a uniformly random coordinate inside the spawn range.
fn random_spawn_candidate(state: &GameState, rng: &mut impl Rng) -> BoardCoord {
    let range = spawn_range(state);
    BoardCoord::new(rng.gen_range(range.clone()), rng.gen_range(range))
}

/// Returns true when no piece or shop is closer to `pos` than the spawn clearances.
fn has_clearance(state: &GameState, pos: BoardCoord) -> bool {
    let distance = |other: BoardCoord| (other - pos).as_vec2().length();
    state
        .pieces
        .values()
        .all(|piece| distance(piece.position) >= PIECE_CLEARANCE)
        && state
            .shops
            .iter()
            .all(|shop| distance(shop.position) >= SHOP_CLEARANCE)
}

/// Finds a spawn position, preferring squares far away from pieces and shops.
///
/// Falls back to any free square, and finally to any square in the spawn range.
pub fn find_spawn_pos(state: &GameState, rng: &mut impl Rng) -> BoardCoord {
    let mut candidates = std::iter::repeat_with(|| random_spawn_candidate(state, rng));
    candidates
        .by_ref()
        .take(SPAWN_ATTEMPTS)
        .find(|pos| has_clearance(state, *pos))
        .or_else(|| {
            candidates
                .by_ref()
                .take(SPAWN_ATTEMPTS)
                .find(|pos| state.piece_at(*pos).is_none() && state.shop_at(*pos).is_none())
        })
        .unwrap_or_else(|| random_spawn_candidate(state, rng))
}

/// Finds a random free square within `radius` of `origin`, excluding `origin` itself.
pub fn find_random_nearby_free_pos(
    state: &GameState,
    origin: BoardCoord,
    rng: &mut impl Rng,
    radius: i32,
    attempts: usize,
) -> Option<BoardCoord> {
    std::iter::repeat_with(|| {
        origin
            + IVec2::new(
                rng.gen_range(-radius..=radius),
                rng.gen_range(-radius..=radius),
            )
    })
    .take(attempts)
    .find(|pos| *pos != origin && state.is_free(*pos))
}
