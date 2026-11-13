//! Tests for spawn position helpers.

use common::models::{GameState, Piece, Shop};
use common::types::{BoardCoord, BoardSize, PieceTypeId, ShopId};
use rand::SeedableRng;
use rand::rngs::StdRng;
use server::spawning::{find_random_nearby_free_pos, find_spawn_pos};

/// Builds an empty game state with a specific board size.
fn empty_state(board_size: i32) -> GameState {
    GameState {
        board_size: BoardSize::from(board_size),
        ..Default::default()
    }
}

/// Places an NPC pawn at `pos`.
fn add_pawn(state: &mut GameState, pos: BoardCoord) {
    let piece = Piece::new(None, PieceTypeId::from("pawn"), pos);
    state.pieces.insert(piece.id, piece);
}

#[test]
/// Verifies free-position checks consider bounds, pieces, and shops.
fn is_free_checks_bounds_and_occupancy() {
    let mut state = empty_state(10);
    let pos = BoardCoord::new(1, 1);

    assert!(state.is_free(pos));
    assert!(!state.is_free(BoardCoord::new(100, 100)));

    add_pawn(&mut state, pos);
    assert!(!state.is_free(pos));

    let shop_pos = BoardCoord::new(2, 2);
    state.shops.push(Shop {
        position: shop_pos,
        uses_remaining: 1,
        shop_id: ShopId::from("shop"),
    });
    assert!(!state.is_free(shop_pos));
}

#[test]
/// Verifies adjacent placement returns the first open neighbour in a stable order.
fn adjacent_free_returns_first_open_slot() {
    let mut state = empty_state(10);
    let origin = BoardCoord::new(0, 0);
    add_pawn(&mut state, BoardCoord::new(1, 0));

    assert_eq!(state.adjacent_free(origin), Some(BoardCoord::new(-1, 0)));
}

#[test]
/// Verifies nearby placement with a zero radius never finds a square other than the origin.
fn nearby_free_pos_with_zero_radius_finds_nothing() {
    let state = empty_state(10);
    let mut rng = StdRng::seed_from_u64(7);
    assert_eq!(
        find_random_nearby_free_pos(&state, BoardCoord::new(0, 0), &mut rng, 0, 5),
        None
    );
}

#[test]
/// Verifies nearby placement stays within the requested radius.
fn nearby_free_pos_stays_within_radius() {
    let state = empty_state(10);
    let mut rng = StdRng::seed_from_u64(11);
    let origin = BoardCoord::new(0, 0);
    let pos = find_random_nearby_free_pos(&state, origin, &mut rng, 1, 50).expect("free square");
    assert!(pos != origin && pos.chebyshev_distance(origin) <= 1);
}

#[test]
/// Verifies spawning works on boards too small for the edge margin.
fn spawn_pos_on_tiny_boards_stays_on_board() {
    let mut rng = StdRng::seed_from_u64(3);
    for size in 1..8 {
        let state = empty_state(size);
        let pos = find_spawn_pos(&state, &mut rng);
        assert!(state.board_size.contains(pos), "size {size} gave {pos}");
    }
}
