//! Tests for spawn position helpers.

use common::models::{GameState, Piece, Shop};
use common::types::{BoardCoord, BoardSize, DurationMs, PieceId, PieceTypeId, PlayerId, ShopId, TimestampMs};
use glam::IVec2;
use rand::SeedableRng;
use rand::rngs::StdRng;
use server::spawning::{find_adjacent_free_pos, find_random_nearby_free_pos, is_free_position};

/// Builds an empty game state with a specific board size.
///
/// `board_size` sets the board dimension. Returns a `GameState`.
fn empty_state(board_size: i32) -> GameState {
    GameState {
        board_size: BoardSize::from(board_size),
        ..Default::default()
    }
}

#[test]
/// Verifies free-position checks consider bounds and occupancy.
fn test_is_free_position_checks_bounds_and_occupancy() {
    let mut state = empty_state(10);
    let pos = IVec2::new(1, 1);

    assert!(is_free_position(&state, pos));
    assert!(!is_free_position(&state, IVec2::new(100, 100)));

    let piece_id = PieceId::new();
    state.pieces.insert(
        piece_id,
        Piece {
            id: piece_id,
            owner_id: Some(PlayerId::new()),
            piece_type: PieceTypeId::from("pawn"),
            position: BoardCoord(pos),
            last_move_time: TimestampMs::from_millis(0),
            cooldown_ms: DurationMs::zero(),
        },
    );
    assert!(!is_free_position(&state, pos));

    state.pieces.clear();
    state.shops.push(Shop {
        position: BoardCoord(pos),
        uses_remaining: 1,
        shop_id: ShopId::from("shop"),
    });
    assert!(!is_free_position(&state, pos));
}

#[test]
/// Verifies adjacent free position lookup returns the first open offset.
fn test_find_adjacent_free_pos_returns_first_open_slot() {
    let mut state = empty_state(10);
    let origin = IVec2::new(0, 0);

    // Block all adjacent positions except (1, 0) which is checked first.
    let blocked = [
        IVec2::new(-1, 0),
        IVec2::new(0, 1),
        IVec2::new(0, -1),
        IVec2::new(1, 1),
        IVec2::new(-1, 1),
        IVec2::new(1, -1),
        IVec2::new(-1, -1),
    ];

    for pos in blocked {
        let piece_id = PieceId::new();
        state.pieces.insert(
            piece_id,
            Piece {
                id: piece_id,
                owner_id: Some(PlayerId::new()),
                piece_type: PieceTypeId::from("pawn"),
                position: BoardCoord(pos),
                last_move_time: TimestampMs::from_millis(0),
                cooldown_ms: DurationMs::zero(),
            },
        );
    }

    let found = find_adjacent_free_pos(&state, origin);
    assert_eq!(found, Some(IVec2::new(1, 0)));
}

#[test]
/// Verifies random nearby position selection works with fixed offsets.
fn test_find_random_nearby_free_pos_handles_fixed_offset() {
    let state = empty_state(10);
    let origin = IVec2::new(0, 0);
    let mut rng = StdRng::seed_from_u64(42);

    let found = find_random_nearby_free_pos(&state, origin, &mut rng, 1..=1, 3);
    assert_eq!(found, Some(IVec2::new(1, 1)));
}

#[test]
/// Verifies random nearby position returns none when no valid offsets exist.
fn test_find_random_nearby_free_pos_returns_none_when_no_offsets() {
    let state = empty_state(10);
    let origin = IVec2::new(0, 0);
    let mut rng = StdRng::seed_from_u64(7);

    let found = find_random_nearby_free_pos(&state, origin, &mut rng, 0..=0, 5);
    assert_eq!(found, None);
}
