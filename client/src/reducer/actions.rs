//! Reducer actions and payloads for client state updates.

use crate::reducer::types::{MsgSender, QueueStatus};
use common::models::{
    GameModeClientConfig, GameState, MatchSummary, Piece, PieceAction, Player, Shop,
};
use common::protocol::{ChatLine, GameError, GameOverStats, VictoryFocusTarget};
use common::rules::Catalog;
use common::types::{
    ActionId, BoardCoord, BoardSize, PieceId, PlayerId, SessionSecret, TimestampMs,
};

/// Initial game snapshot payload delivered on connect.
pub struct InitPayload {
    pub player_id: PlayerId,
    pub session_secret: SessionSecret,
    pub move_unlock_at: Option<TimestampMs>,
    pub state: GameState,
    pub mode: GameModeClientConfig,
    pub catalog: Catalog,
    pub chat_room_key: String,
    pub chat_history: Vec<ChatLine>,
    pub spectators: Vec<String>,
}

/// Per-tick world snapshot payload.
pub struct UpdateStatePayload {
    pub players: Vec<Player>,
    pub pieces: Vec<Piece>,
    pub shops: Vec<Shop>,
    pub board_size: BoardSize,
    pub spectators: Vec<String>,
}

/// Content of an overlay telling the player about a lost or refused connection.
pub struct DisconnectNotice {
    pub disconnected: bool,
    pub is_fatal: bool,
    pub title: Option<String>,
    pub msg: Option<String>,
}

impl DisconnectNotice {
    /// Clears every disconnect overlay.
    pub fn none() -> Self {
        Self {
            disconnected: false,
            is_fatal: false,
            title: None,
            msg: None,
        }
    }

    /// Shows the generic, recoverable "connection lost" overlay.
    pub fn connection_lost() -> Self {
        Self {
            disconnected: true,
            ..Self::none()
        }
    }

    /// Shows a fatal banner with a custom title and message.
    pub fn fatal(title: String, msg: String) -> Self {
        Self {
            disconnected: true,
            is_fatal: true,
            title: Some(title),
            msg: Some(msg),
        }
    }
}

/// Actions that drive the client reducer state machine.
pub enum GameAction {
    SetInit(Box<InitPayload>),
    SetQueueStatus(QueueStatus),
    UpdateState(Box<UpdateStatePayload>),
    /// Queues a piece action locally and sends it to the server.
    ///
    /// Sending happens inside the reducer so the request id and chain predecessor always come
    /// from the latest queue, even for several actions issued before a re-render.
    QueueAction {
        piece_id: PieceId,
        action: PieceAction,
        tx: MsgSender,
    },
    /// Drops every queued action of a piece, locally and on the server.
    ClearActions {
        piece_id: PieceId,
        tx: MsgSender,
    },
    ActionResult {
        id: ActionId,
        result: Result<(), GameError>,
        culprits: Vec<BoardCoord>,
    },
    PushChatLine(ChatLine),
    PruneExpiredChat {
        now: TimestampMs,
        ttl_ms: u32,
    },
    SetError(GameError),
    /// Hides the error notice with the given serial if it is still shown.
    ClearError(u32),
    SetVictory {
        title: String,
        msg: String,
        focus_target: VictoryFocusTarget,
    },
    GameOver(GameOverStats),
    Pong(u64, TimestampMs),
    SetFPS(u32),
    SetDisconnected(DisconnectNotice),
    /// A new socket delivered its first message; the server holds no queue entry for it yet.
    ConnectionEstablished,
    ResetForRejoin,
    Reset,
    /// Leaves the local session to watch a board; the server answers with its Init.
    BeginSpectate,
    /// The spectated match ended.
    SpectateEnded,
    SetMatchList(Vec<MatchSummary>),
}
