//! Tests for server state, hooks, and the server-side action queue.

use common::models::PieceAction;
use common::protocol::{ActionRequest, ClientMessage, GameError, ServerMessage};
use common::types::{
    ActionId, BoardCoord, DurationMs, KitId, ModeId, PieceId, PieceTypeId, PlayerId, TimestampMs,
};
use server::instance::{GameInstance, JoinRequest};
use server::state::ServerState;
use server::time::now_ms;
use std::sync::Arc;
use tokio::sync::mpsc;

type Inbox = mpsc::Receiver<ServerMessage>;

/// Returns the public instance of `mode`.
fn instance_of(mode: &str) -> Arc<GameInstance> {
    ServerState::new()
        .public_instance(&ModeId::from(mode))
        .expect("mode exists")
}

/// Joins `instance` with `kit` and returns the player id and their inbox.
fn join(instance: &GameInstance, name: &str, kit: &str) -> (PlayerId, Inbox) {
    let (tx, rx) = mpsc::channel(256);
    let (player_id, _) = instance
        .join(JoinRequest {
            name: name.to_string(),
            kit_name: KitId::from(kit),
            tx,
            player_id: None,
            session_secret: None,
        })
        .expect("join succeeds");
    (player_id, rx)
}

/// Collects every message waiting in an inbox.
fn drain(rx: &mut Inbox) -> Vec<ServerMessage> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

/// Collects the action results waiting in an inbox.
fn action_results(rx: &mut Inbox) -> Vec<(ActionId, Result<(), GameError>)> {
    drain(rx)
        .into_iter()
        .filter_map(|message| match message {
            ServerMessage::ActionResult { id, result } => Some((id, result)),
            _ => None,
        })
        .collect()
}

/// Collects the victory messages waiting in an inbox.
fn victory_messages(rx: &mut Inbox) -> Vec<String> {
    drain(rx)
        .into_iter()
        .filter_map(|message| match message {
            ServerMessage::Victory { message, .. } => Some(message),
            _ => None,
        })
        .collect()
}

/// Removes every piece except the listed kings and places each king at its square.
///
/// Kings marked as busy are on a long cooldown; the others can move immediately.
fn keep_only_kings(instance: &GameInstance, kings: &[(PlayerId, [i32; 2], bool)]) -> Vec<PieceId> {
    instance.with_state(|state| {
        let ids: Vec<PieceId> = kings
            .iter()
            .map(|(player_id, _, _)| state.players[player_id].king_id)
            .collect();
        state.pieces.retain(|id, _| ids.contains(id));
        for (id, (_, pos, busy)) in ids.iter().zip(kings) {
            let king = state.pieces.get_mut(id).expect("king exists");
            king.position = BoardCoord::new(pos[0], pos[1]);
            king.last_move_time = if *busy {
                now_ms()
            } else {
                TimestampMs::from_millis(0)
            };
            king.cooldown_ms = if *busy {
                DurationMs::from_millis(60_000)
            } else {
                DurationMs::zero()
            };
        }
        ids
    })
}

/// Makes a piece ready to move again.
fn make_ready(instance: &GameInstance, piece_id: PieceId) {
    instance.with_state(|state| {
        state
            .pieces
            .get_mut(&piece_id)
            .expect("piece exists")
            .last_move_time = TimestampMs::from_millis(0);
    });
}

/// Returns the position of a piece.
fn position_of(instance: &GameInstance, piece_id: PieceId) -> BoardCoord {
    instance.with_state(|state| state.pieces[&piece_id].position)
}

/// Builds a move request.
fn move_to(id: u64, piece_id: PieceId, target: [i32; 2], after: Option<u64>) -> ActionRequest {
    ActionRequest {
        id: ActionId(id),
        piece_id,
        action: PieceAction::Move {
            target: BoardCoord::new(target[0], target[1]),
        },
        after: after.map(ActionId),
    }
}

#[tokio::test]
/// Verifies players spawn with correct kit piece counts.
async fn players_spawn_with_their_kits() {
    let instance = instance_of("ffa");
    let (p1, _rx1) = join(&instance, "P1", "Standard");
    let (p2, _rx2) = join(&instance, "P2", "Tank");

    instance.with_state(|state| {
        assert_eq!(state.players.len(), 2);
        assert_eq!(state.pieces_of(p1).count(), 5);
        assert_eq!(state.pieces_of(p2).count(), 3);
    });
}

#[tokio::test]
/// Verifies capturing a king eliminates the owning player and credits the kill.
async fn king_capture_eliminates_owner() {
    let instance = instance_of("ffa");
    let (p1, _rx1) = join(&instance, "P1", "Standard");
    let (p2, mut rx2) = join(&instance, "P2", "Standard");
    let kings = keep_only_kings(&instance, &[(p1, [1, 1], false), (p2, [0, 0], false)]);

    instance.submit_action(p2, move_to(1, kings[1], [1, 1], None));
    assert_eq!(action_results(&mut rx2), vec![(ActionId(1), Ok(()))]);
    instance.tick();

    instance.with_state(|state| {
        assert!(!state.players.contains_key(&p1));
        assert_eq!(state.pieces_of(p1).count(), 0);
        assert_eq!(state.players[&p2].kills, 1);
    });
}

#[tokio::test]
/// Verifies disconnecting in a duel grants a win message to the remaining player.
async fn duel_leave_grants_victory() {
    let instance = instance_of("blitz");
    let (p1, mut rx1) = join(&instance, "P1", "Classic");
    let (_p2, mut rx2) = join(&instance, "P2", "Classic");

    instance.leave(p1);
    assert!(
        drain(&mut rx1)
            .iter()
            .any(|message| matches!(message, ServerMessage::GameOver(_)))
    );

    instance.tick();
    assert_eq!(
        victory_messages(&mut rx2),
        vec!["Opponent disconnected".to_string()]
    );
}

#[tokio::test]
/// Verifies a king-capture victory is delivered once and no leave victory follows.
async fn duel_capture_victory_has_no_delayed_leave_victory() {
    let instance = instance_of("blitz");
    let (p1, _rx1) = join(&instance, "P1", "Classic");
    let (p2, mut rx2) = join(&instance, "P2", "Classic");
    let kings = keep_only_kings(&instance, &[(p1, [1, 1], false), (p2, [0, 0], false)]);

    instance.submit_action(p2, move_to(1, kings[1], [1, 1], None));
    instance.tick();
    assert_eq!(victory_messages(&mut rx2), vec![String::new()]);

    instance.leave(p1);
    instance.tick();
    assert!(victory_messages(&mut rx2).is_empty());
}

#[tokio::test]
/// Verifies an action on a piece in cooldown waits and runs on a later tick.
async fn queued_move_runs_after_cooldown() {
    let instance = instance_of("blitz");
    let (p1, mut rx) = join(&instance, "P1", "Classic");
    let king = keep_only_kings(&instance, &[(p1, [0, 0], true)])[0];

    instance.submit_action(p1, move_to(1, king, [1, 0], None));
    instance.tick();
    assert_eq!(position_of(&instance, king), BoardCoord::new(0, 0));
    assert!(action_results(&mut rx).is_empty());

    make_ready(&instance, king);
    instance.tick();
    assert_eq!(position_of(&instance, king), BoardCoord::new(1, 0));
    assert_eq!(action_results(&mut rx), vec![(ActionId(1), Ok(()))]);
}

#[tokio::test]
/// Verifies a chain returning to its starting square runs fully and in order.
async fn chained_moves_run_in_order() {
    let instance = instance_of("blitz");
    let (p1, mut rx) = join(&instance, "P1", "Classic");
    let king = keep_only_kings(&instance, &[(p1, [0, 0], true)])[0];

    instance.submit_action(p1, move_to(1, king, [1, 0], None));
    instance.submit_action(p1, move_to(2, king, [0, 0], Some(1)));
    instance.submit_action(p1, move_to(3, king, [0, 1], Some(2)));

    for expected in [[1, 0], [0, 0], [0, 1]] {
        make_ready(&instance, king);
        instance.tick();
        assert_eq!(
            position_of(&instance, king),
            BoardCoord::new(expected[0], expected[1])
        );
    }
    assert_eq!(
        action_results(&mut rx),
        vec![
            (ActionId(1), Ok(())),
            (ActionId(2), Ok(())),
            (ActionId(3), Ok(()))
        ]
    );
}

#[tokio::test]
/// Verifies a failing action cancels the rest of its chain, including late arrivals.
async fn failed_action_cancels_its_chain() {
    let instance = instance_of("ffa");
    let (p1, mut rx) = join(&instance, "P1", "Standard");
    let king = instance.with_state(|state| state.players[&p1].king_id);
    let blocker = instance.with_state(|state| {
        state
            .pieces_of(p1)
            .find(|piece| piece.id != king)
            .expect("kit has more pieces")
            .id
    });
    instance.with_state(|state| {
        state.pieces.retain(|id, _| *id == king || *id == blocker);
        let king = state.pieces.get_mut(&king).unwrap();
        king.position = BoardCoord::new(0, 0);
        king.last_move_time = now_ms();
        king.cooldown_ms = DurationMs::from_millis(60_000);
        state.pieces.get_mut(&blocker).unwrap().position = BoardCoord::new(5, 5);
    });

    instance.submit_action(p1, move_to(1, king, [1, 0], None));
    instance.submit_action(p1, move_to(2, king, [2, 0], Some(1)));
    // A friendly piece now occupies the first target.
    instance.with_state(|state| {
        state.pieces.get_mut(&blocker).unwrap().position = BoardCoord::new(1, 0)
    });
    make_ready(&instance, king);
    instance.tick();
    instance.submit_action(p1, move_to(3, king, [0, 1], Some(2)));

    assert_eq!(position_of(&instance, king), BoardCoord::new(0, 0));
    assert_eq!(
        action_results(&mut rx),
        vec![
            (ActionId(1), Err(GameError::TargetFriendly)),
            (ActionId(2), Err(GameError::ActionCancelled)),
            (ActionId(3), Err(GameError::ActionCancelled)),
        ]
    );
}

#[tokio::test]
/// Verifies clearing a piece's actions cancels them before they run.
async fn clear_actions_cancels_queue() {
    let instance = instance_of("blitz");
    let (p1, mut rx) = join(&instance, "P1", "Classic");
    let king = keep_only_kings(&instance, &[(p1, [0, 0], true)])[0];

    instance.submit_action(p1, move_to(1, king, [1, 0], None));
    instance.clear_actions(p1, king);
    make_ready(&instance, king);
    instance.tick();

    assert_eq!(position_of(&instance, king), BoardCoord::new(0, 0));
    assert_eq!(
        action_results(&mut rx),
        vec![(ActionId(1), Err(GameError::ActionCancelled))]
    );
}

#[tokio::test]
/// Verifies players cannot act with pieces they do not own.
async fn actions_on_foreign_pieces_are_rejected() {
    let instance = instance_of("blitz");
    let (p1, mut rx1) = join(&instance, "P1", "Classic");
    let (p2, _rx2) = join(&instance, "P2", "Classic");
    let kings = keep_only_kings(&instance, &[(p1, [0, 0], false), (p2, [3, 3], false)]);

    instance.submit_action(p1, move_to(1, kings[1], [3, 2], None));

    assert_eq!(
        action_results(&mut rx1),
        vec![(ActionId(1), Err(GameError::NotYourPiece))]
    );
    assert_eq!(position_of(&instance, kings[1]), BoardCoord::new(3, 3));
}

#[tokio::test]
/// Verifies joins beyond the mode's player limit are refused.
async fn joins_beyond_max_players_are_refused() {
    let instance = instance_of("blitz");
    let _p1 = join(&instance, "P1", "Classic");
    let _p2 = join(&instance, "P2", "Classic");
    let (tx, _rx) = mpsc::channel(8);

    let third = instance.join(JoinRequest {
        name: "P3".to_string(),
        kit_name: KitId::from("Classic"),
        tx,
        player_id: None,
        session_secret: None,
    });
    assert!(matches!(third, Err(GameError::Custom { .. })));
}

#[tokio::test]
/// Verifies rejoining with a wrong session secret is refused.
async fn rejoin_requires_matching_secret() {
    let instance = instance_of("arena");
    let (p1, _rx) = join(&instance, "P1", "Standard");
    instance.detach(p1);
    let (tx, _rx2) = mpsc::channel(8);

    let rejoin = instance.join(JoinRequest {
        name: "P1".to_string(),
        kit_name: KitId::from("Standard"),
        tx,
        player_id: Some(p1),
        session_secret: Some(common::types::SessionSecret::new()),
    });
    assert_eq!(rejoin.err(), Some(GameError::InvalidSession));
}

#[tokio::test]
/// Verifies a preview request arriving right after a queue join does not pull the player out of
/// the match that join started.
async fn preview_request_after_matchmaking_keeps_player_in_match() {
    let state = ServerState::new();
    let mode = ModeId::from("blitz");
    let mut inboxes = Vec::new();
    for name in ["P1", "P2"] {
        let (tx, rx) = mpsc::channel(512);
        let conn = state
            .open_connection(mode.clone(), tx)
            .expect("mode exists");
        state.handle_message(
            &conn,
            ClientMessage::Join {
                name: name.to_string(),
                kit_name: KitId::from("Classic"),
                player_id: None,
                session_secret: None,
            },
        );
        state.handle_message(&conn, ClientMessage::SetPreviewDefault { enabled: true });
        inboxes.push((conn, rx));
    }
    state.tick();

    let summary = state
        .mode_summaries()
        .into_iter()
        .find(|summary| summary.id == mode)
        .expect("blitz listed");
    assert_eq!(summary.players, common::types::PlayerCount::new(2));
    for (_, rx) in &mut inboxes {
        assert!(
            !drain(rx)
                .iter()
                .any(|message| matches!(message, ServerMessage::Victory { .. })),
            "nobody may win by the opponent leaving"
        );
    }
}

#[tokio::test]
/// Verifies the blitz king castles with the shipped config, but not onto an attacked square.
async fn blitz_king_castles_unless_attacked() {
    let instance = instance_of("blitz");
    let (p1, mut rx) = join(&instance, "P1", "Classic");
    let (p2, _rx2) = join(&instance, "P2", "Classic");
    let rook_type = PieceTypeId::from("blitz_rook");
    let [king, rook, attacker] = instance.with_state(|state| {
        let rook_of = |player| {
            state
                .pieces_of(player)
                .find(|piece| piece.piece_type == rook_type)
                .expect("kit has a rook")
                .id
        };
        let ids = [state.players[&p1].king_id, rook_of(p1), rook_of(p2)];
        state.pieces.retain(|id, _| ids.contains(id));
        for (id, pos) in ids.iter().zip([[0, 3], [3, 3], [2, -4]]) {
            let piece = state.pieces.get_mut(id).expect("piece exists");
            piece.position = BoardCoord::new(pos[0], pos[1]);
            piece.last_move_time = TimestampMs::from_millis(0);
        }
        ids
    });

    // The enemy rook attacks the square the king lands on.
    instance.submit_action(p1, move_to(1, king, [2, 3], None));
    assert_eq!(
        action_results(&mut rx),
        vec![(ActionId(1), Err(GameError::InvalidMove))]
    );

    // Attacking only the square the king crosses, where the rook lands, is allowed.
    instance.with_state(|state| {
        state
            .pieces
            .get_mut(&attacker)
            .expect("attacker exists")
            .position = BoardCoord::new(1, -4);
    });
    instance.submit_action(p1, move_to(2, king, [2, 3], None));
    assert_eq!(action_results(&mut rx), vec![(ActionId(2), Ok(()))]);
    assert_eq!(position_of(&instance, king), BoardCoord::new(2, 3));
    assert_eq!(position_of(&instance, rook), BoardCoord::new(1, 3));
    // Castling is the king's move, so the rook can move again right away.
    instance.with_state(|state| assert!(state.pieces[&rook].is_ready(now_ms())));
}
