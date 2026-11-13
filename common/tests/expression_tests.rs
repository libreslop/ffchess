//! Tests for expression evaluation and board sizing helpers.

use common::models::{GameModeConfig, NpcLimitConfig};
use common::types::{BoardSize, ExprString, ExprVars};

#[test]
/// Verifies expressions use provided variables when evaluating.
fn evaluate_expression_uses_variables() {
    let expr = ExprString::from("player_count * 2 + 3");
    let vars = ExprVars::new().with("player_count", 4.0);
    assert_eq!(expr.evaluate(&vars), 11.0);
}

#[test]
/// Verifies expression evaluation returns zero on missing variables.
fn evaluate_expression_unknown_vars_returns_zero() {
    let expr = ExprString::from("missing_var + 2");
    assert_eq!(expr.evaluate(&ExprVars::new()), 0.0);
}

#[test]
/// Verifies board size calculations clamp to at least 1 tile.
fn board_size_for_clamps_minimum() {
    let mode = GameModeConfig {
        board_size: ExprString::from("0"),
        ..GameModeConfig::default()
    };
    assert_eq!(mode.board_size_for(0), BoardSize::from(1));
}

#[test]
/// Verifies board size expressions see the player count.
fn board_size_for_uses_player_count() {
    let mode = GameModeConfig {
        board_size: ExprString::from("10 + player_count * 2.5"),
        ..GameModeConfig::default()
    };
    assert_eq!(mode.board_size_for(3), BoardSize::from(17));
}

#[test]
/// Verifies NPC limits never go negative.
fn npc_limit_clamps_negative_results() {
    let limit = NpcLimitConfig {
        piece_id: "pawn".into(),
        max_expr: ExprString::from("player_count - 5"),
    };
    assert_eq!(limit.max_count(2), 0);
    assert_eq!(limit.max_count(8), 3);
}
