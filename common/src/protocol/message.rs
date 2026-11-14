use crate::models::{
    GameModeClientConfig, GameState, MatchSummary, Piece, PieceAction, Player, Shop,
};
use crate::protocol::error::GameError;
use crate::rules::Catalog;
use crate::types::{
    ActionId, BoardCoord, BoardSize, ColorHex, KitId, ModeId, PieceId, PlayerCount, PlayerId,
    QueuePosition, Score, SessionSecret, TimestampMs,
};
use serde::{Deserialize, Serialize};

/// Camera target behavior for a victory overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VictoryFocusTarget {
    #[default]
    KeepCurrent,
    BoardPosition(BoardCoord),
}

/// One chat line sent by the server for the active room.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatLine {
    pub sender_name: String,
    pub sender_color: ColorHex,
    pub message: String,
    #[serde(default)]
    pub is_system: bool,
    pub sent_at: TimestampMs,
}

/// A piece action requested by a client.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRequest {
    /// Client-chosen id echoed back in the matching [`ServerMessage::ActionResult`].
    pub id: ActionId,
    pub piece_id: PieceId,
    pub action: PieceAction,
    /// The previous queued action of the same piece this one was planned after.
    ///
    /// When that action failed or was cleared, this one is cancelled as well, because the
    /// client planned it from a position the piece never reached.
    pub after: Option<ActionId>,
}

/// End-of-life statistics of a removed player.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameOverStats {
    pub final_score: Score,
    pub kills: u32,
    pub pieces_captured: u32,
    pub time_survived_secs: u64,
}

/// The board a client asks to spectate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpectateTarget {
    /// The match the connection last played in, or the mode's shared world when it has none.
    Own,
    /// A running private match of a queue mode, by its instance id.
    Match(ModeId),
}

/// Messages sent from the client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join {
        name: String,
        kit_name: KitId,
        player_id: Option<PlayerId>,
        session_secret: Option<SessionSecret>,
    },
    /// Leaves the current match, queue, or spectated board.
    Leave,
    /// Asks for the running matches of a queue mode, answered by [`ServerMessage::MatchList`].
    ListMatches,
    /// Watches a board as a named spectator.
    Spectate {
        name: String,
        target: SpectateTarget,
        /// Stored player id of the client, so it cannot watch a match it plays in elsewhere.
        player_id: Option<PlayerId>,
    },
    QueueAction(ActionRequest),
    ClearActions {
        piece_id: PieceId,
    },
    SetPreviewDefault {
        enabled: bool,
    },
    Chat {
        name_hint: String,
        message: String,
    },
    Ping(u64),
}

/// Messages sent from the server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Init {
        player_id: PlayerId,
        session_secret: SessionSecret,
        move_unlock_at: Option<TimestampMs>,
        state: Box<GameState>,
        mode: Box<GameModeClientConfig>,
        catalog: Box<Catalog>,
        chat_room_key: String,
        chat_history: Vec<ChatLine>,
        /// Names of the named spectators of the board.
        spectators: Vec<String>,
        sync_interval_ms: u32,
    },
    /// Full snapshot of the mutable world state, sent every server tick.
    UpdateState {
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        board_size: BoardSize,
        spectators: Vec<String>,
    },
    /// Outcome of a queued action; every [`ActionRequest`] gets exactly one result.
    ///
    /// Results are sent before the snapshot that first reflects the action.
    ActionResult {
        id: ActionId,
        result: Result<(), GameError>,
    },
    Error(GameError),
    Victory {
        title: String,
        message: String,
        focus_target: VictoryFocusTarget,
    },
    QueueState {
        position_in_queue: QueuePosition,
        queued_players: PlayerCount,
        required_players: PlayerCount,
    },
    GameOver(GameOverStats),
    /// Running matches of the connection's queue mode, newest first.
    MatchList {
        matches: Vec<MatchSummary>,
    },
    /// The spectated match ended, or was already over; the connection no longer spectates it.
    SpectateEnded,
    Chat {
        line: ChatLine,
    },
    Pong(u64, TimestampMs),
}

impl ServerMessage {
    /// Returns true for messages after which earlier snapshots may describe a board the client
    /// no longer looks at.
    pub fn switches_board(&self) -> bool {
        matches!(self, Self::Init { .. } | Self::SpectateEnded)
    }
}
