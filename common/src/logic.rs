//! Shared rule helpers for move validation, board sizing, and shop pricing.

use crate::models::{GameModeConfig, Piece, PieceConfig, ShopConfig, ShopGroupConfig};
use crate::types::{BoardSize, DurationMs, ExprString, PieceId, PieceTypeId, PlayerId};
use glam::IVec2;
use std::collections::HashMap;

/// Evaluates a numeric expression string with runtime variables.
///
/// `expr` is the expression to evaluate, `vars` provides variable bindings.
/// Returns the computed value or `0.0` on evaluation errors.
pub fn evaluate_expression(expr: &ExprString, vars: &HashMap<String, f64>) -> f64 {
    let mut context = meval::Context::new();
    for (name, val) in vars {
        context.var(name, *val);
    }
    meval::eval_str_with_context(expr.as_ref(), &context).unwrap_or(0.0)
}

/// Computes the board size for a mode given the current player count.
///
/// `mode` supplies the expression, `player_count` is the active player total.
/// Returns a clamped `BoardSize`.
pub fn calculate_board_size(mode: &GameModeConfig, player_count: usize) -> BoardSize {
    let mut vars = HashMap::new();
    vars.insert("player_count".to_string(), player_count as f64);
    let size = evaluate_expression(&mode.board_size, &vars).trunc() as i32;
    BoardSize::from(size)
}

/// Checks if a board position is inside the bounds of a square board.
///
/// `pos` is the tile coordinate, `board_size` is the board dimension.
/// Returns `true` when the position is within the valid board range.
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

/// Validates a move using piece paths, occupancy, and board bounds.
///
/// `params` carries the move details and board state to validate.
/// Returns `true` if the move is legal under the piece rules.
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

/// Calculates the movement cooldown for a piece.
///
/// `piece_config` provides the cooldown value.
/// Returns the cooldown duration in milliseconds.
pub fn calculate_cooldown(piece_config: &PieceConfig) -> DurationMs {
    piece_config.cooldown_ms
}

/// Resolve the shop group for a piece standing on the shop.
///
/// `shop_config` is the full shop config and `piece_on_shop` is the piece present.
/// Returns the matching group or the default group when none applies.
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
///
/// `player_piece_count` is the player's piece count and `piece_counts` yields per-type counts.
/// Returns a map of variables for evaluation.
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

/// Finds the piece located at a specific board position.
///
/// `pieces` is the active piece map and `pos` is the tile coordinate.
/// Returns the piece reference when present.
fn piece_at(pieces: &HashMap<PieceId, Piece>, pos: IVec2) -> Option<&Piece> {
    pieces.values().find(|p| p.position == pos)
}
