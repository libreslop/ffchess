use common::logic::{is_valid_move, is_within_board};
use common::models::{Piece, PieceConfig};
use glam::IVec2;
use std::collections::HashMap;
use uuid::Uuid;

fn mock_pawn_config() -> PieceConfig {
    PieceConfig {
        id: "pawn".to_string(),
        display_name: "Pawn".to_string(),
        char: 'P',
        score_value: 10,
        cooldown_ms: 1000,
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
fn test_is_within_board() {
    let size = 100;
    // Range is -50 to 49
    assert!(is_within_board(IVec2::new(0, 0), size));
    assert!(is_within_board(IVec2::new(-50, -50), size));
    assert!(is_within_board(IVec2::new(49, 49), size));
    assert!(!is_within_board(IVec2::new(-51, 0), size));
    assert!(!is_within_board(IVec2::new(50, 50), size));
}

#[test]
fn test_pawn_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    let config = mock_pawn_config();
    let pieces = HashMap::new();

    // Multi-directional movement (adjacent only)
    assert!(is_valid_move(
        &config,
        start,
        IVec2::new(0, -1),
        false,
        size,
        &pieces,
        None
    ));
    assert!(is_valid_move(
        &config,
        start,
        IVec2::new(0, 1),
        false,
        size,
        &pieces,
        None
    ));
    assert!(is_valid_move(
        &config,
        start,
        IVec2::new(-1, 0),
        false,
        size,
        &pieces,
        None
    ));
    assert!(is_valid_move(
        &config,
        start,
        IVec2::new(1, 0),
        false,
        size,
        &pieces,
        None
    ));

    // Diagonal movement NOT allowed without capture
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(1, 1),
        false,
        size,
        &pieces,
        None
    ));

    // Multi-directional captures (diagonal only) - should fail because target is empty
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(1, -1),
        true,
        size,
        &pieces,
        None
    ));
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(-1, -1),
        true,
        size,
        &pieces,
        None
    ));
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(1, 1),
        true,
        size,
        &pieces,
        None
    ));
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(-1, 1),
        true,
        size,
        &pieces,
        None
    ));

    // Add some targets to test captures
    let mut pieces = HashMap::new();
    let target_id = Uuid::new_v4();
    pieces.insert(
        target_id,
        Piece {
            id: target_id,
            owner_id: Some(Uuid::new_v4()), // Different owner
            piece_type: "pawn".to_string(),
            position: IVec2::new(1, 1),
            last_move_time: 0,
            cooldown_ms: 0,
        },
    );
    assert!(is_valid_move(
        &config,
        start,
        IVec2::new(1, 1),
        true,
        size,
        &pieces,
        None
    ));

    // Adjacent captures NOT allowed
    assert!(!is_valid_move(
        &config,
        start,
        IVec2::new(0, -1),
        true,
        size,
        &pieces,
        None
    ));
}

#[test]
fn test_path_blocking() {
    let size = 100;
    let start = IVec2::new(0, 0);
    let mut pieces = HashMap::new();
    let blocker_id = Uuid::new_v4();
    pieces.insert(
        blocker_id,
        Piece {
            id: blocker_id,
            owner_id: Some(Uuid::new_v4()),
            piece_type: "pawn".to_string(),
            position: IVec2::new(0, 1),
            last_move_time: 0,
            cooldown_ms: 0,
        },
    );

    let mut rook_config = PieceConfig {
        id: "rook".to_string(),
        display_name: "Rook".to_string(),
        char: 'R',
        score_value: 50,
        cooldown_ms: 3000,
        move_paths: vec![vec![IVec2::new(0, 1), IVec2::new(0, 2), IVec2::new(0, 3)]],
        capture_paths: vec![vec![IVec2::new(0, 1), IVec2::new(0, 2), IVec2::new(0, 3)]],
    };

    // Blocked path
    assert!(!is_valid_move(
        &rook_config,
        start,
        IVec2::new(0, 2),
        false,
        size,
        &pieces,
        None
    ));
    // Not blocked
    assert!(is_valid_move(
        &rook_config,
        start,
        IVec2::new(0, 1),
        true,
        size,
        &pieces,
        None
    ));
}
