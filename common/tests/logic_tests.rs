use common::*;
use glam::IVec2;

#[test]
fn test_is_within_board() {
    let size = 100;
    assert!(is_within_board(IVec2::new(0, 0), size));
    assert!(is_within_board(IVec2::new(99, 99), size));
    assert!(!is_within_board(IVec2::new(-1, 0), size));
    assert!(!is_within_board(IVec2::new(100, 100), size));
}

#[test]
fn test_pawn_movement() {
    let size = 100;
    let start = IVec2::new(50, 50);
    
    // Multi-directional movement (adjacent only)
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(50, 49), false, size)); // Up
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(50, 51), false, size)); // Down
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(49, 50), false, size)); // Left
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(51, 50), false, size)); // Right
    
    // Diagonal movement NOT allowed without capture
    assert!(!is_valid_chess_move(PieceType::Pawn, start, IVec2::new(51, 51), false, size));

    // Multi-directional captures (diagonal only)
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(51, 49), true, size)); // Top-Right
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(49, 49), true, size)); // Top-Left
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(51, 51), true, size)); // Bottom-Right
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(49, 51), true, size)); // Bottom-Left

    // Adjacent captures NOT allowed
    assert!(!is_valid_chess_move(PieceType::Pawn, start, IVec2::new(50, 49), true, size));
}

#[test]
fn test_knight_movement() {
    let size = 100;
    let start = IVec2::new(50, 50);
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(52, 51), false, size));
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(48, 51), false, size));
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(51, 52), false, size));
    assert!(!is_valid_chess_move(PieceType::Knight, start, IVec2::new(51, 51), false, size));
}

#[test]
fn test_queen_movement() {
    let size = 100;
    let start = IVec2::new(50, 50);
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(60, 60), false, size));
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(50, 60), false, size));
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(40, 50), false, size));
    assert!(!is_valid_chess_move(PieceType::Queen, start, IVec2::new(51, 52), false, size));
}

#[test]
fn test_cooldown_calculation() {
    let start = IVec2::new(0, 0);
    let end_close = IVec2::new(1, 0);
    let end_far = IVec2::new(10, 0);

    let _cd_pawn_close = calculate_cooldown(PieceType::Pawn, start, end_close);
    let _cd_pawn_far = calculate_cooldown(PieceType::Pawn, start, end_far);

    // Dynamic cooldown check: Bishop base 1200 + 400 * dist
    let cd_bishop_1 = calculate_cooldown(PieceType::Bishop, start, IVec2::new(1, 0));
    let cd_bishop_2 = calculate_cooldown(PieceType::Bishop, start, IVec2::new(2, 0));
    assert!(cd_bishop_2 > cd_bishop_1);
}

#[test]
fn test_upgrade_costs() {
    let cost_0_pieces = get_upgrade_cost(PieceType::Queen, 0);
    let cost_10_pieces = get_upgrade_cost(PieceType::Queen, 10);
    
    assert!(cost_10_pieces > cost_0_pieces);
    assert_eq!(cost_0_pieces, 250);
}
