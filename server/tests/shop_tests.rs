//! Tests for shop purchases through the action queue.

use common::models::{
    GameModeConfig, KitConfig, PieceAction, PieceConfig, Shop, ShopConfig, ShopGroupConfig,
    ShopItemConfig, ShopItemIndex,
};
use common::protocol::{ActionRequest, ServerMessage};
use common::rules::Catalog;
use common::types::{
    ActionId, DurationMs, ExprString, KitId, ModeId, PieceTypeId, PlayerCount, Score, ShopId,
    TimestampMs,
};
use glam::IVec2;
use server::instance::{GameInstance, JoinRequest};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Builds a minimal piece config for testing.
fn make_piece_config(id: &str, score: u64) -> PieceConfig {
    PieceConfig {
        id: PieceTypeId::from(id),
        display_name: id.to_string(),
        svg_path: format!("{id}.svg"),
        score_value: Score::from(score),
        cooldown_ms: DurationMs::from_millis(500),
        move_paths: vec![vec![IVec2::new(1, 0)]],
        capture_paths: vec![vec![IVec2::new(1, 0)]],
        special_moves: vec![],
    }
}

#[test]
/// Verifies shop purchases deduct score, spawn pieces, and report success.
fn shop_purchase_deducts_score_and_adds_piece() {
    let shop_id = ShopId::from("test_shop");
    let shop_config = ShopConfig {
        id: shop_id.clone(),
        display_name: "Test Shop".to_string(),
        default_uses: 1,
        color: None,
        focus_color: None,
        auto_upgrade_single_item: false,
        groups: vec![],
        default_group: Some(ShopGroupConfig {
            applies_to: vec![],
            items: vec![ShopItemConfig {
                display_name: "Hire Pawn".to_string(),
                price_expr: Some(ExprString::from("10")),
                replace_with: None,
                add_pieces: vec![PieceTypeId::from("pawn")],
            }],
        }),
    };
    let catalog = Catalog {
        pieces: [
            make_piece_config("king", 500),
            make_piece_config("pawn", 10),
        ]
        .into_iter()
        .map(|config| (config.id.clone(), config))
        .collect(),
        shops: [(shop_id.clone(), shop_config)].into_iter().collect(),
    };
    let mode = GameModeConfig {
        id: ModeId::from("test"),
        display_name: "Test".to_string(),
        max_players: PlayerCount::new(8),
        board_size: ExprString::from("20"),
        kits: vec![KitConfig {
            name: KitId::from("basic"),
            description: "Basic".to_string(),
            pieces: vec![PieceTypeId::from("king")],
        }],
        ..GameModeConfig::default()
    };
    let instance = GameInstance::public(mode, Arc::new(catalog), DurationMs::from_millis(10_000));

    let (tx, mut rx) = mpsc::channel(100);
    let (player_id, _) = instance
        .join(JoinRequest {
            name: "Tester".to_string(),
            kit_name: KitId::from("basic"),
            tx,
            player_id: None,
            session_secret: None,
        })
        .expect("player joins");

    let king_id = instance.with_state(|state| {
        let king = state.king_of(player_id).expect("king").clone();
        state.players.get_mut(&player_id).unwrap().score = Score::from(100);
        state.shops.push(Shop {
            position: king.position,
            uses_remaining: 1,
            shop_id: shop_id.clone(),
        });
        king.id
    });

    instance.submit_action(
        player_id,
        ActionRequest {
            id: ActionId(1),
            piece_id: king_id,
            action: PieceAction::Buy {
                item: ShopItemIndex(0),
            },
            after: None,
        },
    );

    let result = std::iter::from_fn(|| rx.try_recv().ok()).find_map(|message| match message {
        ServerMessage::ActionResult { id, result } => Some((id, result)),
        _ => None,
    });
    assert_eq!(result, Some((ActionId(1), Ok(()))));

    instance.with_state(|state| {
        assert_eq!(state.players[&player_id].score, Score::from(90));
        let pieces: Vec<_> = state.pieces_of(player_id).collect();
        assert_eq!(pieces.len(), 2);
        let pawn = pieces
            .iter()
            .find(|piece| piece.piece_type == PieceTypeId::from("pawn"))
            .expect("pawn bought");
        assert_eq!(pawn.last_move_time, TimestampMs::from_millis(0));
        assert_eq!(pawn.cooldown_ms, DurationMs::zero());
        // The depleted shop was replaced by a fresh one elsewhere.
        assert_eq!(state.shops.len(), 1);
        assert_eq!(state.shops[0].uses_remaining, 1);
    });
}
