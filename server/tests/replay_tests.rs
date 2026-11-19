//! Tests for finishing matches and recording their replays.

use common::models::{GameModeConfig, Piece, PieceAction, Replay};
use common::protocol::{ActionRequest, ServerMessage};
use common::types::{
    ActionId, BoardCoord, DurationMs, KitId, ModeId, PieceId, PlayerId, ReplayId, TimestampMs,
};
use server::config::ConfigManager;
use server::instance::{GameInstance, JoinRequest, MatchStatus, Spectator};
use server::outbound::{ClientFeed, ClientTx};
use server::replays::ReplayStore;
use server::types::ConnectionId;
use std::time::{Duration, Instant};

/// How long a test waits for the replay writer thread to save a replay.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Returns the shipped config of `mode`.
fn mode_config(mode: &str) -> (GameModeConfig, ConfigManager) {
    let config = ConfigManager::load(&server::paths::config_dir());
    let mode = config.modes[&ModeId::from(mode)].clone();
    (mode, config)
}

/// Creates a private match of `mode` recording into `store`.
fn private_match(
    mode: &GameModeConfig,
    config: &ConfigManager,
    store: &ReplayStore,
) -> GameInstance {
    GameInstance::private_match(
        mode,
        config.catalog.clone(),
        config.global.chat_ttl(),
        Some(store),
    )
}

/// Joins `instance` with the Classic kit and returns the player id and their inbox.
fn join(instance: &GameInstance, name: &str) -> (PlayerId, ClientFeed) {
    let (tx, rx) = ClientTx::channel(512);
    let (player_id, _) = instance
        .join(JoinRequest {
            name: name.to_string(),
            kit_name: KitId::from("Classic"),
            tx,
            player_id: None,
            session_secret: None,
        })
        .expect("join succeeds");
    (player_id, rx)
}

/// Collects every message waiting in an inbox.
fn drain(rx: &mut ClientFeed) -> Vec<ServerMessage> {
    std::iter::from_fn(|| rx.try_next_event()).collect()
}

/// Returns the replay announced by the first [`ServerMessage::MatchFinished`] in `messages`.
fn finished_replay(messages: &[ServerMessage]) -> Option<Option<ReplayId>> {
    messages.iter().find_map(|message| match message {
        ServerMessage::MatchFinished { replay } => Some(*replay),
        _ => None,
    })
}

/// Leaves only the two kings, ready to move, with the second right next to the first.
fn kings_face_off(instance: &GameInstance, victim: PlayerId, winner: PlayerId) -> ActionRequest {
    let winner_king = instance.with_state(|state| {
        let kings = [
            state.players[&victim].king_id,
            state.players[&winner].king_id,
        ];
        state.pieces.retain(|id, _| kings.contains(id));
        for (id, pos) in kings.iter().zip([[1, 1], [0, 0]]) {
            let king = state.pieces.get_mut(id).expect("king exists");
            king.position = BoardCoord::new(pos[0], pos[1]);
            king.last_move_time = TimestampMs::from_millis(0);
            king.cooldown_ms = DurationMs::zero();
        }
        kings[1]
    });
    ActionRequest {
        id: ActionId(1),
        piece_id: winner_king,
        action: PieceAction::Move {
            target: BoardCoord::new(1, 1),
        },
        after: None,
    }
}

/// Waits for the writer thread to save a replay, then decodes it.
fn wait_for_replay(store: &ReplayStore, id: ReplayId) -> Replay {
    let started = Instant::now();
    loop {
        if let Some(json) = store.finished_replay(id) {
            return serde_json::from_str(&json).expect("replay decodes");
        }
        assert!(started.elapsed() < WRITE_TIMEOUT, "replay was never saved");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
/// Verifies a won duel finishes the match, tells both players, and saves a viewable replay.
async fn won_duel_finishes_with_replay() {
    let (mode, config) = mode_config("bullet");
    let store = ReplayStore::in_memory();
    let instance = private_match(&mode, &config, &store);
    let (p1, mut rx1) = join(&instance, "P1");
    let (p2, mut rx2) = join(&instance, "P2");
    instance.tick();
    assert_eq!(instance.status(), MatchStatus::Running);

    instance.submit_action(p2, kings_face_off(&instance, p1, p2));
    instance.tick();

    assert!(instance.is_finished());
    let replay_id = finished_replay(&drain(&mut rx2))
        .flatten()
        .expect("winner learns the replay");
    assert_eq!(
        finished_replay(&drain(&mut rx1)),
        Some(Some(replay_id)),
        "the loser still watches the board"
    );

    let replay = wait_for_replay(&store, replay_id);
    let keyframes = replay.keyframes();
    let first = keyframes.first().expect("the start is recorded");
    let last = keyframes.last().expect("the end is recorded");
    assert_eq!(first.state.players.len(), 2);
    assert_eq!(first.state.pieces.len(), 32);
    assert_eq!(last.state.players.len(), 1);
    assert!(last.state.players.contains_key(&p2));
    assert!(last.at <= replay.ended_at);
}

#[tokio::test]
/// Verifies spectators learn the match finished before they are released.
async fn spectators_are_released_after_the_finish() {
    let (mode, config) = mode_config("bullet");
    let store = ReplayStore::in_memory();
    let instance = private_match(&mode, &config, &store);
    let (p1, _rx1) = join(&instance, "P1");
    let (p2, _rx2) = join(&instance, "P2");
    let (tx, mut spectator) = ClientTx::channel(512);
    let conn_id = ConnectionId::new();
    instance.add_spectator(
        conn_id,
        tx,
        Spectator {
            name: "Watcher".to_string(),
            player_id: None,
        },
    );

    instance.submit_action(p2, kings_face_off(&instance, p1, p2));
    instance.tick();

    let messages = drain(&mut spectator);
    let finished = messages
        .iter()
        .position(|message| matches!(message, ServerMessage::MatchFinished { replay: Some(_) }))
        .expect("spectator learns the replay");
    let ended = messages
        .iter()
        .position(|message| matches!(message, ServerMessage::SpectateEnded))
        .expect("spectator is released");
    assert!(finished < ended);
    assert!(!instance.has_viewer(conn_id));
}

#[tokio::test]
/// Verifies a match of a mode without replays finishes without recording one.
async fn match_without_replays_announces_none() {
    let (mut mode, config) = mode_config("bullet");
    mode.allow_replays = false;
    let store = ReplayStore::in_memory();
    let instance = private_match(&mode, &config, &store);
    let (p1, _rx1) = join(&instance, "P1");
    let (p2, mut rx2) = join(&instance, "P2");

    instance.submit_action(p2, kings_face_off(&instance, p1, p2));
    instance.tick();

    assert_eq!(finished_replay(&drain(&mut rx2)), Some(None));
}

#[tokio::test]
/// Verifies a match stays finished, and stays out of reach, once it finished.
async fn finished_match_stays_finished() {
    let (mode, config) = mode_config("bullet");
    let store = ReplayStore::in_memory();
    let instance = private_match(&mode, &config, &store);
    let (p1, _rx1) = join(&instance, "P1");
    let (p2, mut rx2) = join(&instance, "P2");
    instance.submit_action(p2, kings_face_off(&instance, p1, p2));
    instance.tick();
    let MatchStatus::Finished { at } = instance.status() else {
        panic!("match finished");
    };
    drain(&mut rx2);

    instance.leave(p2);
    instance.tick();
    assert_eq!(instance.status(), MatchStatus::Finished { at });
    assert_eq!(
        finished_replay(&drain(&mut rx2)),
        None,
        "finishes only once"
    );
}

#[tokio::test]
/// Verifies a capture and the guard taking the square back within one tick stay two moves in
/// the replay, recorded at the same moment.
async fn guard_recapture_is_recorded_as_its_own_move() {
    let (mode, config) = mode_config("bullet");
    let store = ReplayStore::in_memory();
    let instance = private_match(&mode, &config, &store);
    let (p1, _rx1) = join(&instance, "P1");
    let (p2, mut rx2) = join(&instance, "P2");
    instance.tick();

    // The first player's king guards one of their pieces, which the second player's queen
    // takes, out of reach of the second player's king.
    let ready = |piece_ids: &[PieceId]| {
        instance.with_state(|state| {
            for id in piece_ids {
                let piece = state.pieces.get_mut(id).expect("piece exists");
                piece.last_move_time = TimestampMs::from_millis(0);
                piece.cooldown_ms = DurationMs::zero();
            }
        })
    };
    let (guard, attacker, loser_king) = instance.with_state(|state| {
        let (guard, loser_king) = (state.players[&p1].king_id, state.players[&p2].king_id);
        let find = |owner, is_wanted: &dyn Fn(&Piece) -> bool| {
            state
                .pieces
                .values()
                .find(|piece| piece.owner_id == Some(owner) && is_wanted(piece))
                .expect("piece exists")
                .id
        };
        let guarded = find(p1, &|piece| piece.id != guard);
        let attacker = find(p2, &|piece| piece.piece_type.as_ref().contains("queen"));
        let kept = [guard, guarded, attacker, loser_king];
        state.pieces.retain(|id, _| kept.contains(id));
        for (id, pos) in kept.iter().zip([[0, 0], [1, 1], [3, 3], [6, 0]]) {
            let piece = state.pieces.get_mut(id).expect("piece exists");
            piece.position = BoardCoord::new(pos[0], pos[1]);
        }
        (guard, attacker, loser_king)
    });
    ready(&[guard, attacker]);
    let request = |id, piece_id, action| ActionRequest {
        id: ActionId(id),
        piece_id,
        action,
        after: None,
    };
    let square = BoardCoord::new(1, 1);
    instance.submit_action(p1, request(1, guard, PieceAction::Guard { target: square }));
    instance.submit_action(
        p2,
        request(1, attacker, PieceAction::Move { target: square }),
    );
    instance.tick();
    // Taking the other king, brought next to the guard, finishes the match, saving its replay.
    let king_square = BoardCoord::new(2, 1);
    instance.with_state(|state| {
        state
            .pieces
            .get_mut(&loser_king)
            .expect("king exists")
            .position = king_square;
    });
    ready(&[guard]);
    instance.submit_action(
        p1,
        request(
            2,
            guard,
            PieceAction::Move {
                target: king_square,
            },
        ),
    );
    instance.tick();

    let replay_id = finished_replay(&drain(&mut rx2))
        .flatten()
        .expect("the match was recorded");
    let keyframes = wait_for_replay(&store, replay_id).keyframes();
    let standing_on_square = |piece_id| {
        keyframes.iter().position(|keyframe| {
            keyframe
                .state
                .pieces
                .get(&piece_id)
                .map(|piece| piece.position)
                == Some(square)
        })
    };
    let captured = standing_on_square(attacker).expect("the capture is its own frame");
    let taken_back = standing_on_square(guard).expect("the guard takes the square back");
    assert_eq!(taken_back, captured + 1);
    assert_eq!(keyframes[captured].at, keyframes[taken_back].at);
}
