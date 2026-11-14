//! Tests for the outbound lanes of a connection and the cleanup of user text.

use common::protocol::{ChatLine, ClientMessage, GameError, ServerMessage, SpectateTarget};
use common::types::{ColorHex, KitId, ModeId, PlayerId, TimestampMs};
use server::outbound::{ClientFeed, ClientTx, EncodedUpdate};
use server::state::ServerState;
use server::text::UserText;

/// Builds a chat event carrying `message`.
fn chat(message: &str) -> ServerMessage {
    ServerMessage::Chat {
        line: ChatLine {
            sender_name: "P1".to_string(),
            sender_color: ColorHex::from("#ffffff"),
            message: message.to_string(),
            is_system: false,
            sent_at: TimestampMs::from_millis(0),
        },
    }
}

/// Encodes a pong standing in for a board snapshot.
fn snapshot(marker: u64) -> EncodedUpdate {
    EncodedUpdate::encode(&ServerMessage::Pong(marker, TimestampMs::from_millis(0)))
        .expect("encodes")
}

/// Collects every event waiting in a feed.
fn drain(feed: &mut ClientFeed) -> Vec<ServerMessage> {
    std::iter::from_fn(|| feed.try_next_event()).collect()
}

#[tokio::test]
/// Verifies only the newest snapshot is kept, and that it goes out after queued events.
async fn feed_keeps_newest_snapshot_after_events() {
    let (tx, mut feed) = ClientTx::channel(8);
    tx.publish(&snapshot(1));
    tx.send(chat("hello"));
    tx.publish(&snapshot(2));

    let batch = feed.next_batch().await.expect("connection open");
    assert_eq!(batch.len(), 2);
    assert!(batch[0].contains("hello"));
    assert!(batch[1].contains("[2,"));
}

#[tokio::test]
/// Verifies a snapshot read alongside a board switch is dropped.
async fn feed_drops_snapshot_before_board_switch() {
    let (tx, mut feed) = ClientTx::channel(8);
    tx.publish(&snapshot(1));
    tx.send(ServerMessage::SpectateEnded);

    let batch = feed.next_batch().await.expect("connection open");
    assert_eq!(batch, vec!["\"SpectateEnded\"".to_string()]);
}

#[tokio::test]
/// Verifies a connection whose event queue overflows is closed instead of losing events.
async fn feed_closes_on_event_overflow() {
    let (tx, mut feed) = ClientTx::channel(1);
    tx.send(chat("first"));
    tx.send(chat("second"));
    assert!(feed.next_batch().await.is_none());
}

#[test]
/// Verifies names lose control and invisible characters and keep single spaces.
fn names_drop_invisible_characters() {
    assert_eq!("  Bob\u{200B}\u{202E} \t Smith\u{0007} ".clean_name(), "Bob Smith");
    assert_eq!("\u{2800}\u{3164}".clean_name(), "");
    assert_eq!("محمد".clean_name(), "محمد");
}

#[test]
/// Verifies chat messages lose bidirectional controls but keep emoji sequences.
fn messages_drop_bidi_controls() {
    assert_eq!("\u{202E}olleh\u{2066}".clean_message(), "olleh");
    assert_eq!("👨\u{200D}👩".clean_message(), "👨\u{200D}👩");
}

/// Opens a connection to the shared world of `mode`.
fn connect(state: &ServerState, mode: &str) -> (server::state::Connection, ClientFeed) {
    let (tx, feed) = ClientTx::channel(512);
    let conn = state
        .open_connection(ModeId::from(mode), tx)
        .expect("mode exists");
    (conn, feed)
}

/// Returns the player id of the first Init carrying one.
fn joined_player(messages: &[ServerMessage]) -> Option<PlayerId> {
    messages.iter().find_map(|message| match message {
        ServerMessage::Init { player_id, .. } if !player_id.is_nil() => Some(*player_id),
        _ => None,
    })
}

#[tokio::test]
/// Verifies a client cannot spectate a game it plays in from another tab.
async fn spectating_own_active_game_is_refused() {
    let state = ServerState::new();
    let (player, mut player_feed) = connect(&state, "ffa");
    state.handle_message(
        &player,
        ClientMessage::Join {
            name: "P1".to_string(),
            kit_name: KitId::from("Standard"),
            player_id: None,
            session_secret: None,
        },
    );
    let player_id = joined_player(&drain(&mut player_feed)).expect("joined");

    let (watcher, mut watcher_feed) = connect(&state, "ffa");
    drain(&mut watcher_feed);
    state.handle_message(
        &watcher,
        ClientMessage::Spectate {
            name: "P1".to_string(),
            target: SpectateTarget::Own,
            player_id: Some(player_id),
        },
    );
    let replies = drain(&mut watcher_feed);
    assert!(
        replies
            .iter()
            .any(|message| matches!(message, ServerMessage::Error(GameError::Custom { .. })))
    );
    let instance = state
        .public_instance(&ModeId::from("ffa"))
        .expect("mode exists");
    assert_eq!(
        instance.match_summary().spectators,
        common::types::PlayerCount::zero()
    );
}

#[tokio::test]
/// Verifies joining a game ends spectating it from another tab.
async fn joining_ends_spectating_in_another_tab() {
    let state = ServerState::new();
    let player_id = PlayerId::new();
    let (watcher, mut watcher_feed) = connect(&state, "ffa");
    state.handle_message(
        &watcher,
        ClientMessage::Spectate {
            name: "P1".to_string(),
            target: SpectateTarget::Own,
            player_id: Some(player_id),
        },
    );
    drain(&mut watcher_feed);

    let (player, _player_feed) = connect(&state, "ffa");
    state.handle_message(
        &player,
        ClientMessage::Join {
            name: "P1".to_string(),
            kit_name: KitId::from("Standard"),
            player_id: Some(player_id),
            session_secret: None,
        },
    );
    assert!(
        drain(&mut watcher_feed)
            .iter()
            .any(|message| matches!(message, ServerMessage::SpectateEnded))
    );
}
