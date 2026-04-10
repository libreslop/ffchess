//! Tests for shop purchase behavior in server instances.

use common::logic::evaluate_expression;
use common::models::{
    GameModeConfig, KitConfig, NpcLimitConfig, PieceConfig, Shop, ShopConfig, ShopCountConfig,
    ShopGroupConfig, ShopItemConfig,
};
use common::types::{
    DurationMs, ExprString, KitId, ModeId, PieceTypeId, PlayerCount, Score, ShopId, TimestampMs,
};
use glam::IVec2;
use server::instance::GameInstance;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Builds a minimal piece config for testing.
///
/// `id` is the piece id and `score` is the reward value.
/// Returns a `PieceConfig`.
fn make_piece_config(id: &str, score: u64) -> PieceConfig {
    PieceConfig {
        id: PieceTypeId::from(id),
        display_name: id.to_string(),
        svg_path: format!("assets/pieces/{}.svg", id),
        score_value: Score::from(score),
        cooldown_ms: DurationMs::from_millis(500),
        move_paths: vec![vec![IVec2::new(1, 0)]],
        capture_paths: vec![vec![IVec2::new(1, 0)]],
    }
}

#[tokio::test]
/// Verifies shop purchases deduct score and spawn pieces.
async fn shop_purchase_deducts_score_and_adds_piece() {
    let mut piece_configs = HashMap::new();
    piece_configs.insert(PieceTypeId::from("king"), make_piece_config("king", 500));
    piece_configs.insert(PieceTypeId::from("pawn"), make_piece_config("pawn", 10));

    let shop_id = ShopId::from("test_shop");
    let shop_config = ShopConfig {
        id: shop_id.clone(),
        display_name: "Test Shop".to_string(),
        default_uses: 1,
        color: None,
        groups: vec![],
        default_group: ShopGroupConfig {
            applies_to: vec![],
            items: vec![ShopItemConfig {
                display_name: "Hire Pawn".to_string(),
                price_expr: ExprString::from("10"),
                replace_with: None,
                add_pieces: vec![PieceTypeId::from("pawn")],
            }],
        },
    };

    let mode_config = GameModeConfig {
        id: ModeId::from("test"),
        display_name: "Test".to_string(),
        max_players: PlayerCount::new(8),
        queue_players: PlayerCount::zero(),
        preview_switch_delay_ms: DurationMs::from_millis(5000),
        board_size: ExprString::from("20"),
        camera_pan_limit: ExprString::from("10"),
        fog_of_war_radius: Some(ExprString::from("10")),
        respawn_cooldown_ms: DurationMs::zero(),
        npc_limits: vec![NpcLimitConfig {
            piece_id: PieceTypeId::from("pawn"),
            max_expr: ExprString::from("0"),
        }],
        shop_counts: vec![ShopCountConfig {
            shop_id: shop_id.clone(),
            count: 0,
        }],
        kits: vec![KitConfig {
            name: KitId::from("basic"),
            description: "Basic".to_string(),
            pieces: vec![PieceTypeId::from("king")],
        }],
        hooks: vec![],
    };

    let instance = GameInstance::new(
        mode_config.clone(),
        mode_config.id.clone(),
        Arc::new(piece_configs),
        Arc::new(HashMap::from([(shop_id.clone(), shop_config.clone())])),
    );

    let (tx, _) = mpsc::unbounded_channel();
    let (player_id, _) = instance
        .add_player("Tester".to_string(), KitId::from("basic"), tx, None, None)
        .await
        .expect("player joins");

    let shop_pos = {
        let mut game = instance.game.write().await;
        let king_id = game.players.get(&player_id).unwrap().king_id;
        let king_pos = game.pieces.get(&king_id).unwrap().position;

        game.players.get_mut(&player_id).unwrap().score = Score::from(100);

        game.shops.push(Shop {
            position: king_pos,
            uses_remaining: shop_config.default_uses,
            shop_id: shop_id.clone(),
        });
        king_pos
    };

    instance
        .handle_shop_buy(player_id, shop_pos, 0)
        .await
        .expect("shop buy succeeds");

    let game = instance.game.read().await;
    let player = game.players.get(&player_id).unwrap();
    let price = Score::from(evaluate_expression(&ExprString::from("10"), &HashMap::new()) as u64);
    assert_eq!(player.score, Score::from(100) - price);

    let player_piece_count = game
        .pieces
        .values()
        .filter(|p| p.owner_id == Some(player_id))
        .count();
    assert_eq!(player_piece_count, 2);

    // Ensure cooldown types are wired for new pieces
    let added_piece = game
        .pieces
        .values()
        .find(|p| p.owner_id == Some(player_id) && p.piece_type == PieceTypeId::from("pawn"))
        .unwrap();
    assert_eq!(added_piece.last_move_time, TimestampMs::from_millis(0));
    assert_eq!(added_piece.cooldown_ms, DurationMs::zero());
}
