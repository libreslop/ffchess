use crate::reducer::types::{MsgSender, Pmove};
use common::*;
use uuid::Uuid;

pub enum GameAction {
    SetInit {
        player_id: Uuid,
        state: GameState,
    },
    UpdateState {
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        removed_pieces: Vec<Uuid>,
        removed_players: Vec<Uuid>,
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
    ClearPmQueue(Uuid),
    Tick(MsgSender),
    Pong(u64),
    SetFPS(u32),
    SetDisconnected(bool),
}
