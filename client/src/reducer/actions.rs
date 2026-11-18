//! Reducer actions and payloads for client state updates.

use crate::reducer::types::{DisconnectNotice, MsgSender, QueueStatus};
use common::models::{
    CameraCue, GameModeClientConfig, GameState, MatchSummary, Piece, PieceAction, Player, Shop,
};
use common::protocol::{ChatLine, GameError, GameOverStats};
use common::rules::Catalog;
use common::types::{
    ActionId, BoardCoord, BoardSize, PieceId, PlayerId, ReplayId, SessionSecret, TimestampMs,
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
    },
    /// Camera move a hook made for the local player.
    SetCamera(CameraCue),
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
    /// The match on screen finished, recorded as the given replay if its mode allows replays.
    MatchFinished(Option<ReplayId>),
    SetMatchList(Vec<MatchSummary>),
    /// Shows the start of a replay, watched like a spectator watches a live board.
    LoadReplay(Box<InitPayload>),
    /// Shows the board of a replay at one moment.
    ShowReplayFrame(Box<GameState>),
}
