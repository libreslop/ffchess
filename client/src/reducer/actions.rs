use crate::reducer::types::{MsgSender, Pmove};
use common::models::{
    GameModeClientConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig,
};
use common::protocol::GameError;
use common::types::{PieceId, PieceTypeId, PlayerId, SessionSecret, ShopId};
use std::collections::HashMap;

pub enum GameAction {
    SetInit {
        player_id: PlayerId,
        session_secret: SessionSecret,
        state: GameState,
        mode: GameModeClientConfig,
        pieces: HashMap<PieceTypeId, PieceConfig>,
        shops: HashMap<ShopId, ShopConfig>,
    },
    UpdateState {
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        removed_pieces: Vec<PieceId>,
        removed_players: Vec<PlayerId>,
        board_size: i32,
    },
    SetError(GameError),
    GameOver {
        final_score: u64,
        kills: u32,
        pieces_captured: u32,
        time_survived_secs: u64,
    },
    AddPmove(Pmove),
    ClearPmQueue(PieceId),
    Tick(MsgSender),
    Pong(u64),
    SetFPS(u32),
    SetDisconnected {
        disconnected: bool,
        is_fatal: bool,
        title: Option<String>,
        msg: Option<String>,
    },
    Reset,
}
