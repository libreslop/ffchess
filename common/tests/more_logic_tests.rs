use common::*;
use glam::IVec2;
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn test_calculate_board_size() {
    assert_eq!(calculate_board_size(0), 40);
    assert_eq!(calculate_board_size(3), 40);
    assert_eq!(calculate_board_size(10), 80); // 25 + sqrt(10) * 17.5 = 25 + 3.16 * 17.5 = 25 + 55.3 = 80.3
    assert_eq!(calculate_board_size(100), 200); // 25 + sqrt(100) * 17.5 = 25 + 10 * 17.5 = 25 + 175 = 200
    assert_eq!(calculate_board_size(200), 200); // Clamped
}

#[test]
fn test_king_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    assert!(is_valid_chess_move(PieceType::King, start, IVec2::new(1, 1), false, size));
    assert!(is_valid_chess_move(PieceType::King, start, IVec2::new(0, 1), false, size));
    assert!(is_valid_chess_move(PieceType::King, start, IVec2::new(-1, 0), false, size));
    assert!(!is_valid_chess_move(PieceType::King, start, IVec2::new(2, 0), false, size));
    assert!(!is_valid_chess_move(PieceType::King, start, IVec2::new(1, 2), false, size));
}

#[test]
fn test_rook_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    assert!(is_valid_chess_move(PieceType::Rook, start, IVec2::new(10, 0), false, size));
    assert!(is_valid_chess_move(PieceType::Rook, start, IVec2::new(0, -10), false, size));
    assert!(!is_valid_chess_move(PieceType::Rook, start, IVec2::new(1, 1), false, size));
}

#[test]
fn test_bishop_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    assert!(is_valid_chess_move(PieceType::Bishop, start, IVec2::new(10, 10), false, size));
    assert!(is_valid_chess_move(PieceType::Bishop, start, IVec2::new(-5, 5), false, size));
    assert!(!is_valid_chess_move(PieceType::Bishop, start, IVec2::new(10, 0), false, size));
}

#[test]
fn test_is_move_blocked() {
    let mut pieces = HashMap::new();
    let p1_id = Uuid::new_v4();
    pieces.insert(p1_id, Piece {
        id: p1_id,
        owner_id: None,
        piece_type: PieceType::Pawn,
        position: IVec2::new(5, 0),
        last_move_time: 0,
        cooldown_ms: 0,
    });

    // Horizontal block
    assert!(is_move_blocked(IVec2::new(0, 0), IVec2::new(10, 0), &pieces));
    assert!(!is_move_blocked(IVec2::new(0, 0), IVec2::new(4, 0), &pieces));

    // Vertical block
    let p2_id = Uuid::new_v4();
    pieces.insert(p2_id, Piece {
        id: p2_id,
        owner_id: None,
        piece_type: PieceType::Pawn,
        position: IVec2::new(0, 5),
        last_move_time: 0,
        cooldown_ms: 0,
    });
    assert!(is_move_blocked(IVec2::new(0, 0), IVec2::new(0, 10), &pieces));

    // Diagonal block
    let p3_id = Uuid::new_v4();
    pieces.insert(p3_id, Piece {
        id: p3_id,
        owner_id: None,
        piece_type: PieceType::Pawn,
        position: IVec2::new(5, 5),
        last_move_time: 0,
        cooldown_ms: 0,
    });
    assert!(is_move_blocked(IVec2::new(0, 0), IVec2::new(10, 10), &pieces));
}

#[test]
fn test_piece_values() {
    assert_eq!(get_piece_value(PieceType::Pawn), 10);
    assert_eq!(get_piece_value(PieceType::Knight), 30);
    assert_eq!(get_piece_value(PieceType::Bishop), 30);
    assert_eq!(get_piece_value(PieceType::Rook), 50);
    assert_eq!(get_piece_value(PieceType::Queen), 90);
    assert_eq!(get_piece_value(PieceType::King), 500);
}
