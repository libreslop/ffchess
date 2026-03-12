use crate::models::{CooldownConfig, Piece, PieceType};
use glam::IVec2;
use std::collections::HashMap;
use uuid::Uuid;

pub fn calculate_board_size(player_count: usize) -> i32 {
    // (starts at 40x40, scales with player count using a square-root formula up to 200x200).
    // For the first 3 players, stay at 40.
    if player_count <= 3 {
        return 40;
    }
    // Hit ~200 at 100 players: 25 + sqrt(100) * 17.5 = 200
    // We keep the original scaling but clamp to 40.
    (25.0 + (player_count as f32).sqrt() * 17.5).clamp(40.0, 200.0) as i32
}

pub fn is_within_board(pos: IVec2, board_size: i32) -> bool {
    let half = board_size / 2;
    let limit_pos = (board_size + 1) / 2;
    pos.x >= -half && pos.x < limit_pos && pos.y >= -half && pos.y < limit_pos
}

pub fn get_piece_base_cooldown(piece_type: PieceType) -> i64 {
    match piece_type {
        PieceType::Pawn => 1000,
        PieceType::Knight => 2000,
        PieceType::Bishop => 2500,
        PieceType::Rook => 3000,
        PieceType::Queen => 5000,
        PieceType::King => 4000,
    }
}

pub fn calculate_cooldown(
    piece_type: PieceType,
    start: IVec2,
    end: IVec2,
    config: &CooldownConfig,
) -> i64 {
    let distance = (end - start).as_vec2().length() as f64;

    match piece_type {
        PieceType::Pawn => config.pawn_base,
        PieceType::Knight => config.knight_base,
        PieceType::King => config.king_base,
        PieceType::Bishop => config.bishop_base + (config.bishop_dist * distance) as i64,
        PieceType::Rook => config.rook_base + (config.rook_dist * distance) as i64,
        PieceType::Queen => config.queen_base + (config.queen_dist * distance) as i64,
    }
}

pub fn is_valid_chess_move(
    piece_type: PieceType,
    start: IVec2,
    end: IVec2,
    is_capture: bool,
    board_size: i32,
) -> bool {
    if start == end || !is_within_board(end, board_size) {
        return false;
    }

    let diff = end - start;
    let abs_diff = diff.abs();

    match piece_type {
        PieceType::King => abs_diff.x <= 1 && abs_diff.y <= 1,
        PieceType::Queen => (abs_diff.x == abs_diff.y) || (diff.x == 0 || diff.y == 0),
        PieceType::Rook => diff.x == 0 || diff.y == 0,
        PieceType::Bishop => abs_diff.x == abs_diff.y,
        PieceType::Knight => {
            (abs_diff.x == 1 && abs_diff.y == 2) || (abs_diff.x == 2 && abs_diff.y == 1)
        }
        PieceType::Pawn => {
            if is_capture {
                // Pawns capture in any of the 4 diagonal directions
                abs_diff.x == 1 && abs_diff.y == 1
            } else {
                // Pawns move in any of the 4 adjacent directions
                (abs_diff.x == 1 && abs_diff.y == 0) || (abs_diff.x == 0 && abs_diff.y == 1)
            }
        }
    }
}

pub fn is_move_blocked(start: IVec2, end: IVec2, pieces: &HashMap<Uuid, Piece>) -> bool {
    let diff = end - start;
    if diff.x != 0 && diff.y != 0 && diff.x.abs() != diff.y.abs() {
        // Not horizontal, vertical, or diagonal - cannot check blocking this way
        return false;
    }
    let step = IVec2::new(diff.x.signum(), diff.y.signum());
    let mut current = start + step;
    while current != end {
        if pieces.values().any(|p| p.position == current) {
            return true;
        }
        current += step;
    }
    false
}

pub fn get_piece_value(piece_type: PieceType) -> u64 {
    match piece_type {
        PieceType::Pawn => 10,
        PieceType::Knight => 30,
        PieceType::Bishop => 30,
        PieceType::Rook => 50,
        PieceType::Queen => 90,
        PieceType::King => 500,
    }
}

pub fn get_upgrade_cost(piece_type: PieceType, current_piece_count: usize) -> u64 {
    let base_cost = match piece_type {
        PieceType::Pawn => 10,
        PieceType::Knight => 50,
        PieceType::Bishop => 50,
        PieceType::Rook => 100,
        PieceType::Queen => 250,
        _ => 0,
    };
    // Scaling cost based on piece count
    let multiplier = 1.0 + (current_piece_count as f32 * 0.1);
    (base_cost as f32 * multiplier) as u64
}
