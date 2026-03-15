use crate::models::{GameModeConfig, Piece, PieceConfig, ShopConfig, ShopGroupConfig};
use crate::types::{BoardSize, DurationMs, ExprString, PieceId, PieceTypeId, PlayerId};
use glam::IVec2;
use std::collections::HashMap;

pub fn evaluate_expression(expr: &ExprString, vars: &HashMap<String, f64>) -> f64 {
    let mut context = meval::Context::new();
    for (name, val) in vars {
        context.var(name, *val);
    }
    meval::eval_str_with_context(expr.as_ref(), &context).unwrap_or(0.0)
}

pub fn calculate_board_size(mode: &GameModeConfig, player_count: usize) -> BoardSize {
    let mut vars = HashMap::new();
    vars.insert("player_count".to_string(), player_count as f64);
    let size = evaluate_expression(&mode.board_size, &vars).trunc() as i32;
    BoardSize::from(size)
}

pub fn is_within_board(pos: IVec2, board_size: BoardSize) -> bool {
    let half = board_size.half();
    let limit_pos = board_size.limit_pos();
    pos.x >= -half && pos.x < limit_pos && pos.y >= -half && pos.y < limit_pos
}

/// Inputs for validating a move on the board.
pub struct MoveValidationParams<'a> {
    pub piece_config: &'a PieceConfig,
    pub start: IVec2,
    pub end: IVec2,
    pub is_capture: bool,
    pub board_size: BoardSize,
    pub pieces: &'a HashMap<PieceId, Piece>,
    pub moving_owner: Option<PlayerId>,
}

pub fn is_valid_move(params: MoveValidationParams<'_>) -> bool {
    if params.start == params.end || !is_within_board(params.end, params.board_size) {
        return false;
    }

    let diff = params.end - params.start;
    let paths = if params.is_capture {
        &params.piece_config.capture_paths
    } else {
        &params.piece_config.move_paths
    };

    let target_piece = piece_at(params.pieces, params.end);
    if params.is_capture {
        match target_piece {
            Some(p) => {
                if p.owner_id == params.moving_owner {
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
                    let intermediate_pos = params.start + *step;
                    if piece_at(params.pieces, intermediate_pos).is_some() {
                        return false; // Path blocked
                    }
                }
                return true;
            }
        }
    }

    false
}

pub fn calculate_cooldown(piece_config: &PieceConfig, _start: IVec2, _end: IVec2) -> DurationMs {
    piece_config.cooldown_ms
}

/// Resolve the shop group for a piece standing on the shop.
pub fn select_shop_group<'a>(
    shop_config: &'a ShopConfig,
    piece_on_shop: Option<&Piece>,
) -> &'a ShopGroupConfig {
    if let Some(piece) = piece_on_shop {
        shop_config
            .groups
            .iter()
            .find(|g| g.applies_to.contains(&piece.piece_type))
            .unwrap_or(&shop_config.default_group)
    } else {
        &shop_config.default_group
    }
}

/// Build expression variables for pricing formulas.
pub fn build_price_vars<'a, I>(player_piece_count: usize, piece_counts: I) -> HashMap<String, f64>
where
    I: IntoIterator<Item = (&'a PieceTypeId, usize)>,
{
    let mut vars = HashMap::new();
    vars.insert("player_piece_count".to_string(), player_piece_count as f64);
    for (piece_id, count) in piece_counts {
        vars.insert(format!("{}_count", piece_id.as_ref()), count as f64);
    }
    vars
}

fn piece_at(pieces: &HashMap<PieceId, Piece>, pos: IVec2) -> Option<&Piece> {
    pieces.values().find(|p| p.position == pos)
}
