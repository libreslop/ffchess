use crate::models::{GameModeConfig, Piece, PieceConfig};
use crate::types::{PieceId, PlayerId};
use glam::IVec2;
use std::collections::HashMap;

pub fn evaluate_expression(expr: &str, vars: &HashMap<String, f64>) -> f64 {
    let mut context = meval::Context::new();
    for (name, val) in vars {
        context.var(name, *val);
    }
    meval::eval_str_with_context(expr, &context).unwrap_or(0.0)
}

pub fn calculate_board_size(mode: &GameModeConfig, player_count: usize) -> i32 {
    let mut vars = HashMap::new();
    vars.insert("player_count".to_string(), player_count as f64);
    evaluate_expression(&mode.board_size, &vars) as i32
}

pub fn is_within_board(pos: IVec2, board_size: i32) -> bool {
    let half = board_size / 2;
    let limit_pos = (board_size + 1) / 2;
    pos.x >= -half && pos.x < limit_pos && pos.y >= -half && pos.y < limit_pos
}

pub fn is_valid_move(
    piece_config: &PieceConfig,
    start: IVec2,
    end: IVec2,
    is_capture: bool,
    board_size: i32,
    pieces: &HashMap<PieceId, Piece>,
    moving_owner: Option<PlayerId>,
) -> bool {
    if start == end || !is_within_board(end, board_size) {
        return false;
    }

    let diff = end - start;
    let paths = if is_capture {
        &piece_config.capture_paths
    } else {
        &piece_config.move_paths
    };

    let target_piece = pieces.values().find(|p| p.position == end);
    if is_capture {
        match target_piece {
            Some(p) => {
                if p.owner_id == moving_owner {
                    return false; // Cannot capture your own pieces
                }
            }
            None => return false, // Cannot capture an empty square
        }
    } else if target_piece.is_some() {
        return false; // Cannot move to an occupied square
    }

    for path in paths {
        for (i, &step) in path.iter().enumerate() {
            if step == diff {
                // Check if path is blocked (intermediate squares)
                for step in path.iter().take(i) {
                    let intermediate_pos = start + *step;
                    if pieces.values().any(|p| p.position == intermediate_pos) {
                        return false; // Path blocked
                    }
                }
                return true;
            }
        }
    }

    false
}

pub fn calculate_cooldown(piece_config: &PieceConfig, _start: IVec2, _end: IVec2) -> i64 {
    piece_config.cooldown_ms as i64
}
