//! Tests for expression evaluation and board sizing helpers.

use common::models::{GameModeConfig, NpcLimitConfig};
use common::types::{BoardSize, ExprString, ExprVars, MoveCount};

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

#[test]
/// Verifies conditions compare integer counts exactly and combine with boolean operators.
fn conditions_compare_counts() {
    let vars = ExprVars::new()
        .with("self_moves", MoveCount(0))
        .with("partner_moves", MoveCount(2));
    assert!(ExprString::from("self_moves == 0").holds(&vars));
    assert!(ExprString::from("self_moves == 0 && (partner_moves < 3 || false)").holds(&vars));
    assert!(!ExprString::from("!(partner_moves >= 2)").holds(&vars));
}

#[test]
/// Verifies malformed or non-boolean conditions never hold.
fn broken_conditions_do_not_hold() {
    let vars = ExprVars::new().with("self_moves", MoveCount(0));
    assert!(!ExprString::from("self_moves ==").holds(&vars));
    assert!(!ExprString::from("missing == 0").holds(&vars));
    assert!(!ExprString::from("self_moves + 1").holds(&vars));
}

#[test]
/// Verifies condition checks catch malformed, misspelled, and non-boolean conditions.
fn condition_check_reports_mistakes() {
    let sample = ExprVars::new().with("self_moves", MoveCount(0));
    assert!(
        ExprString::from("self_moves == 0")
            .check_condition(&sample)
            .is_ok()
    );
    for broken in [
        "self_moves ==",
        "self_move == 0",
        "self_moves + 1",
        "foo(self_moves)",
    ] {
        assert!(
            ExprString::from(broken).check_condition(&sample).is_err(),
            "{broken}"
        );
    }
}

#[test]
/// Verifies the FFA board size formula grows with the player count.
fn ffa_board_size_formula() {
    let mode = GameModeConfig {
        board_size: ExprString::from("max(34, 25 + math::sqrt(floor(player_count / 3)) * 15)"),
        ..GameModeConfig::default()
    };
    assert_eq!(mode.board_size_for(0), BoardSize::from(34));
    assert_eq!(mode.board_size_for(12), BoardSize::from(55));
}
