//! Tests for core game logic helpers.

use common::logic::{MoveValidationParams, is_valid_move, is_within_board};
use common::models::{Piece, PieceConfig, ShopConfig, ShopGroupConfig, ShopItemConfig};
use common::types::{
    BoardSize, DurationMs, ExprString, PieceId, PieceTypeId, PlayerId, Score, ShopId, TimestampMs,
};
use glam::IVec2;
use std::collections::HashMap;

/// Builds a pawn-like piece config for move validation tests.
///
/// Returns a `PieceConfig` with simple orthogonal moves and diagonal captures.
fn mock_pawn_config() -> PieceConfig {
    PieceConfig {
        id: PieceTypeId::from("pawn"),
        display_name: "Pawn".to_string(),
        char: 'P',
        score_value: Score::from(10),
        cooldown_ms: DurationMs::from_millis(1000),
        move_paths: vec![
            vec![IVec2::new(0, 1)],
            vec![IVec2::new(0, -1)],
            vec![IVec2::new(1, 0)],
            vec![IVec2::new(-1, 0)],
        ],
        capture_paths: vec![
            vec![IVec2::new(1, 1)],
            vec![IVec2::new(1, -1)],
            vec![IVec2::new(-1, 1)],
            vec![IVec2::new(-1, -1)],
        ],
    }
}

#[test]
/// Verifies board boundary checks accept and reject expected positions.
fn test_is_within_board() {
    let size = BoardSize::from(100);
    // Range is -50 to 49
    assert!(is_within_board(IVec2::new(0, 0), size));
    assert!(is_within_board(IVec2::new(-50, -50), size));
    assert!(is_within_board(IVec2::new(49, 49), size));
    assert!(!is_within_board(IVec2::new(-51, 0), size));
    assert!(!is_within_board(IVec2::new(50, 50), size));
}

#[test]
/// Verifies basic pawn-like movement and capture rules.
fn test_pawn_movement() {
    let size = BoardSize::from(100);
    let start = IVec2::new(0, 0);
    let config = mock_pawn_config();
    let pieces = HashMap::new();

    // Multi-directional movement (adjacent only)
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(0, -1),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(0, 1),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(-1, 0),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(1, 0),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));

    // Diagonal movement NOT allowed without capture
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(1, 1),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));

    // Multi-directional captures (diagonal only) - should fail because target is empty
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(1, -1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(-1, -1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(1, 1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(-1, 1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));

    // Add some targets to test captures
    let mut pieces = HashMap::new();
    let target_id = PieceId::new();
    pieces.insert(
        target_id,
        Piece {
            id: target_id,
            owner_id: Some(PlayerId::new()), // Different owner
            piece_type: PieceTypeId::from("pawn"),
            position: IVec2::new(1, 1),
            last_move_time: TimestampMs::from_millis(0),
            cooldown_ms: DurationMs::zero(),
        },
    );
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(1, 1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));

    // Adjacent captures NOT allowed
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &config,
        start,
        end: IVec2::new(0, -1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
}

#[test]
/// Verifies move validation respects blocking pieces along a path.
fn test_path_blocking() {
    let size = BoardSize::from(100);
    let start = IVec2::new(0, 0);
    let mut pieces = HashMap::new();
    let blocker_id = PieceId::new();
    pieces.insert(
        blocker_id,
        Piece {
            id: blocker_id,
            owner_id: Some(PlayerId::new()),
            piece_type: PieceTypeId::from("pawn"),
            position: IVec2::new(0, 1),
            last_move_time: TimestampMs::from_millis(0),
            cooldown_ms: DurationMs::zero(),
        },
    );

    let rook_config = PieceConfig {
        id: PieceTypeId::from("rook"),
        display_name: "Rook".to_string(),
        char: 'R',
        score_value: Score::from(50),
        cooldown_ms: DurationMs::from_millis(3000),
        move_paths: vec![vec![IVec2::new(0, 1), IVec2::new(0, 2), IVec2::new(0, 3)]],
        capture_paths: vec![vec![IVec2::new(0, 1), IVec2::new(0, 2), IVec2::new(0, 3)]],
    };

    // Blocked path
    assert!(!is_valid_move(MoveValidationParams {
        piece_config: &rook_config,
        start,
        end: IVec2::new(0, 2),
        is_capture: false,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
    // Not blocked
    assert!(is_valid_move(MoveValidationParams {
        piece_config: &rook_config,
        start,
        end: IVec2::new(0, 1),
        is_capture: true,
        board_size: size,
        pieces: &pieces,
        moving_owner: None,
    }));
}

#[test]
/// Verifies shop group selection matches piece type or defaults.
fn test_select_shop_group_by_piece_type() {
    let pawn_id = PieceTypeId::from("pawn");
    let default_group = ShopGroupConfig {
        applies_to: vec![],
        items: vec![],
    };
    let pawn_group = ShopGroupConfig {
        applies_to: vec![pawn_id.clone()],
        items: vec![ShopItemConfig {
            display_name: "Pawn Upgrade".to_string(),
            price_expr: ExprString::from("1"),
            replace_with: None,
            add_pieces: vec![],
        }],
    };
    let shop_config = ShopConfig {
        id: ShopId::from("shop"),
        display_name: "Test Shop".to_string(),
        default_uses: 1,
        color: None,
        groups: vec![pawn_group.clone()],
        default_group: default_group.clone(),
    };

    let pawn_piece = Piece {
        id: PieceId::new(),
        owner_id: Some(PlayerId::new()),
        piece_type: pawn_id,
        position: IVec2::new(0, 0),
        last_move_time: TimestampMs::from_millis(0),
        cooldown_ms: DurationMs::zero(),
    };

    let selected = common::logic::select_shop_group(&shop_config, Some(&pawn_piece));
    assert_eq!(selected.items.len(), 1);
    assert_eq!(selected.items[0].display_name, "Pawn Upgrade");

    let selected_default = common::logic::select_shop_group(&shop_config, None);
    assert_eq!(selected_default.items.len(), 0);
}

#[test]
/// Verifies pricing variables include counts for player and piece types.
fn test_build_price_vars() {
    let pawn_id = PieceTypeId::from("pawn");
    let rook_id = PieceTypeId::from("rook");
    let vars = common::logic::build_price_vars(3, vec![(&pawn_id, 2), (&rook_id, 1)]);

    assert_eq!(vars.get("player_piece_count"), Some(&3.0));
    assert_eq!(vars.get("pawn_count"), Some(&2.0));
    assert_eq!(vars.get("rook_count"), Some(&1.0));
}

#[test]
/// Verifies cooldown calculation returns the config-defined value.
fn test_calculate_cooldown_returns_config_value() {
    let config = PieceConfig {
        id: PieceTypeId::from("pawn"),
        display_name: "Pawn".to_string(),
        char: 'P',
        score_value: Score::from(1),
        cooldown_ms: DurationMs::from_millis(1500),
        move_paths: vec![vec![IVec2::new(0, 1)]],
        capture_paths: vec![],
    };

    let cooldown = common::logic::calculate_cooldown(&config, IVec2::new(0, 0), IVec2::new(0, 1));
    assert_eq!(cooldown, DurationMs::from_millis(1500));
}
