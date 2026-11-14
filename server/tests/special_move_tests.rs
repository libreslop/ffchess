//! Tests for special moves through the server action queue.

use common::models::{GameModeConfig, KitConfig, Piece, PieceAction, PieceConfig};
use common::protocol::{ActionRequest, GameError, ServerMessage};
use common::rules::Catalog;
use common::types::{
    ActionId, BoardCoord, DurationMs, ExprString, KitId, ModeId, PieceId, PieceTypeId, PlayerCount,
};
use serde_json::json;
use server::instance::{GameInstance, JoinRequest};
use server::outbound::ClientTx;
use std::sync::Arc;

/// Builds a piece config from JSON, filling in the fields the test does not care about.
fn piece_config(id: &str, mut fields: serde_json::Value) -> PieceConfig {
    let object = fields.as_object_mut().expect("object");
    for (key, value) in [
        ("id", json!(id)),
        ("display_name", json!(id)),
        ("svg_path", json!(format!("{id}.svg"))),
        ("score_value", json!(10)),
        ("cooldown_ms", json!(0)),
        ("move_paths", json!([[[0, 1]]])),
        ("capture_paths", json!([])),
    ] {
        object.entry(key).or_insert(value);
    }
    serde_json::from_value(fields).expect("piece config deserializes")
}

#[test]
/// Verifies a castle runs on the server and cancels the rook's queued premoves.
fn castling_cancels_partner_queue() {
    let king = piece_config(
        "king",
        json!({
            "special_moves": [{
                "type": "partneredmove",
                "target": [2, 0],
                "requirements": [
                    { "at": [1, 0], "condition": "square_empty" },
                    { "at": [2, 0], "condition": "square_empty" }
                ],
                "partners": [{
                    "at": [3, 0],
                    "owner": "self",
                    "types": ["rook"],
                    "action": { "move_to": [1, 0] }
                }]
            }]
        }),
    );
    let catalog = Catalog {
        pieces: [king, piece_config("rook", json!({}))]
            .into_iter()
            .map(|config| (config.id.clone(), config))
            .collect(),
        shops: Default::default(),
    };
    catalog.validate().expect("castling config is valid");
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
    let (tx, mut rx) = ClientTx::channel(100);
    let (player_id, _) = instance
        .join(JoinRequest {
            name: "Tester".to_string(),
            kit_name: KitId::from("basic"),
            tx,
            player_id: None,
            session_secret: None,
        })
        .expect("player joins");

    let (king_id, rook_id) = instance.with_state(|state| {
        let king_id = state.king_of(player_id).expect("king").id;
        state.pieces.retain(|id, _| *id == king_id);
        state.shops.clear();
        state.pieces.get_mut(&king_id).unwrap().position = BoardCoord::new(5, 5);
        // The rook is cooling down, so its premove waits in the queue.
        let mut rook = Piece::new(
            Some(player_id),
            PieceTypeId::from("rook"),
            BoardCoord::new(8, 5),
        );
        rook.cooldown_ms = DurationMs::from_millis(1_000_000_000_000_000);
        let rook_id = rook.id;
        state.pieces.insert(rook_id, rook);
        (king_id, rook_id)
    });

    let request = |id: u64, piece_id: PieceId, target: [i32; 2]| ActionRequest {
        id: ActionId(id),
        piece_id,
        action: PieceAction::Move {
            target: BoardCoord::new(target[0], target[1]),
        },
        after: None,
    };
    instance.submit_action(player_id, request(1, rook_id, [8, 6]));
    instance.submit_action(player_id, request(2, king_id, [7, 5]));

    let results: Vec<_> = std::iter::from_fn(|| rx.try_next_event())
        .filter_map(|message| match message {
            ServerMessage::ActionResult { id, result } => Some((id, result)),
            _ => None,
        })
        .collect();
    assert!(results.contains(&(ActionId(2), Ok(()))), "{results:?}");
    assert!(
        results.contains(&(ActionId(1), Err(GameError::ActionCancelled))),
        "{results:?}"
    );
    instance.with_state(|state| {
        assert_eq!(state.pieces[&king_id].position, BoardCoord::new(7, 5));
        assert_eq!(state.pieces[&rook_id].position, BoardCoord::new(6, 5));
    });
}
