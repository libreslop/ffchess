use common::*;
use glam::IVec2;

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
    
    // Multi-directional movement (adjacent only)
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(0, -1), false, size)); // Up
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(0, 1), false, size)); // Down
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(-1, 0), false, size)); // Left
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(1, 0), false, size)); // Right
    
    // Diagonal movement NOT allowed without capture
    assert!(!is_valid_chess_move(PieceType::Pawn, start, IVec2::new(1, 1), false, size));

    // Multi-directional captures (diagonal only)
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(1, -1), true, size)); // Top-Right
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(-1, -1), true, size)); // Top-Left
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(1, 1), true, size)); // Bottom-Right
    assert!(is_valid_chess_move(PieceType::Pawn, start, IVec2::new(-1, 1), true, size)); // Bottom-Left

    // Adjacent captures NOT allowed
    assert!(!is_valid_chess_move(PieceType::Pawn, start, IVec2::new(0, -1), true, size));
}

#[test]
fn test_knight_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(2, 1), false, size));
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(-2, 1), false, size));
    assert!(is_valid_chess_move(PieceType::Knight, start, IVec2::new(1, 2), false, size));
    assert!(!is_valid_chess_move(PieceType::Knight, start, IVec2::new(1, 1), false, size));
}

#[test]
fn test_queen_movement() {
    let size = 100;
    let start = IVec2::new(0, 0);
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(10, 10), false, size));
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(0, 10), false, size));
    assert!(is_valid_chess_move(PieceType::Queen, start, IVec2::new(-10, 0), false, size));
    assert!(!is_valid_chess_move(PieceType::Queen, start, IVec2::new(1, 2), false, size));
}

#[test]
fn test_cooldown_calculation() {
    let start = IVec2::new(0, 0);
    let end_close = IVec2::new(1, 0);
    let end_far = IVec2::new(10, 0);
    let config = CooldownConfig::default();

    let _cd_pawn_close = calculate_cooldown(PieceType::Pawn, start, end_close, &config);
    let _cd_pawn_far = calculate_cooldown(PieceType::Pawn, start, end_far, &config);

    // Dynamic cooldown check: Bishop base 1200 + 400 * dist
    let cd_bishop_1 = calculate_cooldown(PieceType::Bishop, start, IVec2::new(1, 0), &config);
    let cd_bishop_2 = calculate_cooldown(PieceType::Bishop, start, IVec2::new(2, 0), &config);
    assert!(cd_bishop_2 > cd_bishop_1);
}

#[test]
fn test_upgrade_costs() {
    let cost_0_pieces = get_upgrade_cost(PieceType::Queen, 0);
    let cost_10_pieces = get_upgrade_cost(PieceType::Queen, 10);
    
    assert!(cost_10_pieces > cost_0_pieces);
    assert_eq!(cost_0_pieces, 250);
}
