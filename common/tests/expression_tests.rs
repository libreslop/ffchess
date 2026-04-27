//! Tests for expression evaluation and board sizing helpers.

use common::logic::{calculate_board_size, evaluate_expression};
use common::models::{
    GameModeConfig, JoinCameraCenterConfig, KitConfig, NpcLimitConfig, ShopCountConfig,
};
use common::types::{BoardSize, DurationMs, ExprString, KitId, ModeId, PlayerCount};

#[test]
/// Verifies expressions use provided variables when evaluating.
fn evaluate_expression_uses_variables() {
    let expr = ExprString::from("player_count * 2 + 3");
    let mut vars = std::collections::HashMap::new();
    vars.insert("player_count".to_string(), 4.0);
    let result = evaluate_expression(&expr, &vars);
    assert_eq!(result, 11.0);
}

#[test]
/// Verifies expression evaluation returns zero on missing variables.
fn evaluate_expression_unknown_vars_returns_zero() {
    let expr = ExprString::from("missing_var + 2");
    let vars = std::collections::HashMap::new();
    let result = evaluate_expression(&expr, &vars);
    assert_eq!(result, 0.0);
}

#[test]
/// Verifies board size calculations clamp to at least 1 tile.
fn calculate_board_size_clamps_minimum() {
    let mode = GameModeConfig {
        id: ModeId::from("test"),
        display_name: "Test".to_string(),
        max_players: PlayerCount::new(8),
        queue_players: PlayerCount::zero(),
        preview_switch_delay_ms: DurationMs::from_millis(5000),
        board_size: ExprString::from("0"),
        camera_pan_limit: ExprString::from("10"),
        fog_of_war_radius: Some(ExprString::from("10")),
        show_scoreboard: true,
        join_camera_center: JoinCameraCenterConfig::Piece {
            piece_id: "king".into(),
        },
        disable_screen_panning: false,
        respawn_cooldown_ms: DurationMs::zero(),
        npc_limits: vec![NpcLimitConfig {
            piece_id: "pawn".into(),
            max_expr: ExprString::from("0"),
        }],
        shop_counts: vec![ShopCountConfig {
            shop_id: "shop".into(),
            count: 0,
        }],
        fixed_shops: vec![],
        kits: vec![KitConfig {
            name: KitId::from("basic"),
            description: "Basic".to_string(),
            pieces: vec![],
        }],
        queue_layout: None,
        hooks: vec![],
    };

    let size = calculate_board_size(&mode, 0);
    assert_eq!(size, BoardSize::from(1));
}
