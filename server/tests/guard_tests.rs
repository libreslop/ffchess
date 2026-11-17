//! Tests for guards and the request order of ready actions in the server action queue.

use common::models::{GameModeConfig, KitConfig, Piece, PieceAction, PieceConfig};
use common::protocol::{ActionRequest, GameError, ServerMessage};
use common::rules::Catalog;
use common::types::{
    ActionId, BoardCoord, DurationMs, ExprString, KitId, ModeId, PieceId, PieceTypeId, PlayerCount,
    PlayerId,
};
use serde_json::json;
use server::instance::{GameInstance, JoinRequest};
use server::outbound::{ClientFeed, ClientTx};
use server::time::now_ms;
use std::sync::Arc;

/// Steps of a piece that moves and captures one square in any direction.
const ONE_STEP: [[i32; 2]; 8] = [
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
    [1, 1],
    [1, -1],
    [-1, 1],
    [-1, -1],
];

/// Builds a piece type moving and capturing one square in any direction, without cooldown.
fn one_step_piece(id: &str) -> PieceConfig {
    let paths: Vec<Vec<[i32; 2]>> = ONE_STEP.iter().map(|step| vec![*step]).collect();
    serde_json::from_value(json!({
        "id": id,
        "display_name": id,
        "svg_path": format!("{id}.svg"),
        "score_value": 10,
        "cooldown_ms": 0,
        "move_paths": paths,
        "capture_paths": paths,
    }))
    .expect("piece config deserializes")
}

/// Starts an instance whose players join with a lone king, far from the board centre.
fn instance() -> GameInstance {
    let catalog = Catalog {
        pieces: [one_step_piece("king"), one_step_piece("man")]
            .into_iter()
            .map(|config| (config.id.clone(), config))
            .collect(),
        shops: Default::default(),
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
    instance.with_state(|state| state.shops.clear());
    instance
}

/// Joins `instance`, moving the player's king to `king_at`.
fn join(instance: &GameInstance, name: &str, king_at: [i32; 2]) -> (PlayerId, ClientFeed) {
    let (tx, rx) = ClientTx::channel(256);
    let (player_id, _) = instance
        .join(JoinRequest {
            name: name.to_string(),
            kit_name: KitId::from("basic"),
            tx,
            player_id: None,
            session_secret: None,
        })
        .expect("player joins");
    instance.with_state(|state| {
        let king_id = state.players[&player_id].king_id;
        state
            .pieces
            .retain(|id, piece| *id == king_id || piece.owner_id != Some(player_id));
        state.pieces.get_mut(&king_id).expect("king").position =
            BoardCoord::new(king_at[0], king_at[1]);
    });
    (player_id, rx)
}

/// Places a one-step piece of `owner` on `at`; a busy piece is on a long cooldown.
fn place(instance: &GameInstance, owner: PlayerId, at: [i32; 2], busy: bool) -> PieceId {
    instance.with_state(|state| {
        let mut piece = Piece::new(
            Some(owner),
            PieceTypeId::from("man"),
            BoardCoord::new(at[0], at[1]),
        );
        if busy {
            piece.last_move_time = now_ms();
            piece.cooldown_ms = DurationMs::from_millis(60_000);
        }
        let id = piece.id;
        state.pieces.insert(id, piece);
        id
    })
}

/// Builds a request for `action`, planned after the action `after`.
fn request(id: u64, piece_id: PieceId, action: PieceAction, after: Option<u64>) -> ActionRequest {
    ActionRequest {
        id: ActionId(id),
        piece_id,
        action,
        after: after.map(ActionId),
    }
}

/// Builds a move to `target`.
fn move_to(target: [i32; 2]) -> PieceAction {
    PieceAction::Move {
        target: BoardCoord::new(target[0], target[1]),
    }
}

/// Builds a guard of `target`.
fn guard(target: [i32; 2]) -> PieceAction {
    PieceAction::Guard {
        target: BoardCoord::new(target[0], target[1]),
    }
}

/// Collects the action results waiting in an inbox.
fn action_results(rx: &mut ClientFeed) -> Vec<(ActionId, Result<(), GameError>)> {
    std::iter::from_fn(|| rx.try_next_event())
        .filter_map(|message| match message {
            ServerMessage::ActionResult { id, result, .. } => Some((id, result)),
            _ => None,
        })
        .collect()
}

/// Returns the position of a piece, or `None` once it left the board.
fn position_of(instance: &GameInstance, piece_id: PieceId) -> Option<BoardCoord> {
    instance.with_state(|state| state.pieces.get(&piece_id).map(|piece| piece.position))
}

#[test]
/// Verifies guards wait while their piece stands, and the earliest requested guard takes the
/// square back at once when it is captured, while the other keeps guarding.
fn guards_recapture_in_request_order() {
    let instance = instance();
    let (defender, mut defender_rx) = join(&instance, "Defender", [-9, -9]);
    let (attacker, _attacker_rx) = join(&instance, "Attacker", [9, 9]);
    let guarded = place(&instance, defender, [0, 0], false);
    let below = place(&instance, defender, [0, -1], false);
    let above = place(&instance, defender, [0, 1], false);
    let raider = place(&instance, attacker, [1, 0], false);

    instance.submit_action(defender, request(1, above, guard([0, 0]), None));
    instance.submit_action(defender, request(2, below, guard([0, 0]), None));
    instance.tick();
    assert_eq!(action_results(&mut defender_rx), vec![]);
    assert_eq!(position_of(&instance, above), Some(BoardCoord::new(0, 1)));

    instance.submit_action(attacker, request(1, raider, move_to([0, 0]), None));
    assert_eq!(position_of(&instance, guarded), None);
    assert_eq!(position_of(&instance, raider), None);
    assert_eq!(position_of(&instance, above), Some(BoardCoord::new(0, 0)));
    assert_eq!(position_of(&instance, below), Some(BoardCoord::new(0, -1)));
    assert_eq!(
        action_results(&mut defender_rx),
        vec![(ActionId(1), Ok(()))]
    );
}

#[test]
/// Verifies a guard fails at once, even while cooling down, when the guarded piece leaves
/// the square empty instead of being captured.
fn guard_fails_when_square_is_left_empty() {
    let instance = instance();
    let (player, mut rx) = join(&instance, "Player", [-9, -9]);
    let guarded = place(&instance, player, [0, 0], false);
    let guarding = place(&instance, player, [1, 0], true);

    instance.submit_action(player, request(1, guarding, guard([0, 0]), None));
    instance.submit_action(player, request(1, guarded, move_to([-1, 0]), None));

    assert_eq!(
        action_results(&mut rx),
        vec![
            (ActionId(1), Ok(())),
            (ActionId(1), Err(GameError::InvalidMove))
        ]
    );
    assert_eq!(
        position_of(&instance, guarding),
        Some(BoardCoord::new(1, 0))
    );
}

#[test]
/// Verifies an action queued after a standing guard supersedes it.
fn later_action_supersedes_guard() {
    let instance = instance();
    let (player, mut rx) = join(&instance, "Player", [-9, -9]);
    let guarded = place(&instance, player, [0, 0], false);
    let guarding = place(&instance, player, [1, 0], false);

    instance.submit_action(player, request(1, guarding, guard([0, 0]), None));
    instance.submit_action(player, request(2, guarding, move_to([2, 0]), Some(1)));

    assert_eq!(
        action_results(&mut rx),
        vec![(ActionId(1), Ok(())), (ActionId(2), Ok(()))]
    );
    assert_eq!(
        position_of(&instance, guarding),
        Some(BoardCoord::new(2, 0))
    );
    assert_eq!(position_of(&instance, guarded), Some(BoardCoord::new(0, 0)));
}

#[test]
/// Verifies premoves of different pieces that become ready in the same tick run in the order
/// they were requested.
fn ready_premoves_run_in_request_order() {
    // Piece queues are kept in a hash map, so several boards cover several iteration orders.
    for _ in 0..16 {
        let instance = instance();
        let (player, mut rx) = join(&instance, "Player", [-9, -9]);
        let first = place(&instance, player, [-1, 0], true);
        let second = place(&instance, player, [1, 0], true);

        instance.submit_action(player, request(1, second, move_to([0, 0]), None));
        instance.submit_action(player, request(2, first, move_to([0, 0]), None));
        instance.with_state(|state| {
            for piece_id in [first, second] {
                state.pieces.get_mut(&piece_id).expect("piece").cooldown_ms = DurationMs::zero();
            }
        });
        instance.tick();

        assert_eq!(position_of(&instance, second), Some(BoardCoord::new(0, 0)));
        assert_eq!(position_of(&instance, first), Some(BoardCoord::new(-1, 0)));
        assert_eq!(
            action_results(&mut rx),
            vec![
                (ActionId(1), Ok(())),
                (ActionId(2), Err(GameError::TargetFriendly))
            ]
        );
    }
}
