use yew::prelude::*;
pub use common::*;
use uuid::Uuid;
use glam::IVec2;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub struct Pmove {
    pub piece_id: Uuid,
    pub target: IVec2,
    pub pending: bool,
}

#[derive(Default, Clone, PartialEq)]
pub struct GameStateReducer {
    pub state: GameState,
    pub player_id: Option<Uuid>,
    pub error: Option<String>,
    pub pm_queue: Vec<Pmove>,
    pub last_score: u64,
    pub last_kills: u32,
    pub last_captured: u32,
    pub last_survival_secs: u64,
    pub ping_ms: u64,
    pub fps: u32,
}

pub enum GameAction {
    SetInit { player_id: Uuid, state: GameState },
    UpdateState { 
        players: Vec<Player>, 
        pieces: Vec<Piece>, 
        shops: Vec<Shop>,
        removed_pieces: Vec<Uuid>,
        removed_players: Vec<Uuid>,
    },
    SetError(String),
    GameOver { final_score: u64, kills: u32, pieces_captured: u32, time_survived_secs: u64 },
    AddPmove(Pmove),
    ClearPmQueue(Uuid),
    Tick(MsgSender),
    Pong(u64),
    SetFPS(u32),
}

#[derive(Clone)]
pub struct MsgSender(pub tokio::sync::mpsc::UnboundedSender<ClientMessage>);

impl PartialEq for MsgSender {
    fn eq(&self, _other: &Self) -> bool { true }
}

impl Reducible for GameStateReducer {
    type Action = GameAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            GameAction::SetInit { player_id, state } => {
                next.player_id = Some(player_id);
                next.state = state;
                next.pm_queue.clear();
                next.error = None;
                if let Some(p) = next.state.players.get(&player_id) {
                    next.last_score = p.score;
                }
            }
            GameAction::UpdateState { players, pieces, shops, removed_pieces, removed_players } => {
                next.error = None;
                for p in players { 
                    if next.player_id == Some(p.id) {
                        next.last_score = p.score;
                    }
                    next.state.players.insert(p.id, p); 
                }
                for p in pieces { 
                    // Robust cleanup: remove the match AND any preceding moves for this piece
                    if let Some(match_idx) = next.pm_queue.iter().rposition(|pm| pm.piece_id == p.id && pm.target == p.position) {
                        let mut i = 0;
                        let mut threshold = match_idx;
                        while i <= threshold {
                            if next.pm_queue[i].piece_id == p.id {
                                next.pm_queue.remove(i);
                                if threshold == 0 { break; }
                                threshold -= 1;
                            } else {
                                i += 1;
                            }
                        }
                    }
                    next.state.pieces.insert(p.id, p); 
                }
                next.state.shops = shops;
                for id in removed_pieces { 
                    next.state.pieces.remove(&id); 
                    next.pm_queue.retain(|pm| pm.piece_id != id);
                }
                for id in removed_players { 
                    next.state.players.remove(&id); 
                }
            }
            GameAction::SetError(e) => {
                next.error = Some(e.clone());
                if e.contains("cooldown") {
                    for pm in next.pm_queue.iter_mut() {
                        pm.pending = false;
                    }
                } else {
                    next.pm_queue.clear();
                }
            }
            GameAction::GameOver { final_score, kills, pieces_captured, time_survived_secs } => {
                next.last_score = final_score;
                next.last_kills = kills;
                next.last_captured = pieces_captured;
                next.last_survival_secs = time_survived_secs;
            }
            GameAction::AddPmove(pm) => {
                next.pm_queue.push(pm);
            }
            GameAction::ClearPmQueue(piece_id) => {
                if piece_id == Uuid::nil() {
                    next.pm_queue.clear();
                } else {
                    next.pm_queue.retain(|pm| pm.piece_id != piece_id);
                }
            }
            GameAction::Tick(tx) => {
                let now = chrono::Utc::now().timestamp_millis();
                let mut processed_pieces = std::collections::HashSet::new();
                for pm in next.pm_queue.iter_mut() {
                    if processed_pieces.contains(&pm.piece_id) || pm.pending {
                        processed_pieces.insert(pm.piece_id);
                        continue;
                    }
                    if let Some(piece) = next.state.pieces.get(&pm.piece_id) {
                        if now >= piece.last_move_time + piece.cooldown_ms + 50 {
                            let _ = tx.0.send(ClientMessage::MovePiece { 
                                piece_id: pm.piece_id, 
                                target: pm.target 
                            });
                            pm.pending = true;
                            processed_pieces.insert(pm.piece_id);
                        }
                    }
                }
            }
            GameAction::Pong(t) => {
                let now = js_sys::Date::now() as u64;
                if now >= t {
                    next.ping_ms = now - t;
                }
            }
            GameAction::SetFPS(fps) => {
                next.fps = fps;
            }
        }
        next.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use tokio::sync::mpsc;

    fn setup() -> Rc<GameStateReducer> {
        let mut reducer = GameStateReducer::default();
        reducer.state.board_size = 30;
        Rc::new(reducer)
    }

    #[test]
    fn test_premove_execution() {
        let reducer = setup();
        let piece_id = Uuid::new_v4();
        let piece = Piece {
            id: piece_id,
            owner_id: Some(Uuid::new_v4()),
            piece_type: PieceType::Rook,
            position: IVec2::new(0, 0),
            last_move_time: chrono::Utc::now().timestamp_millis() - 5000,
            cooldown_ms: 1000,
        };
        
        let mut next = (*reducer).clone();
        next.state.pieces.insert(piece_id, piece);
        let reducer = Rc::new(next);

        let target = IVec2::new(0, 5);
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target,
            pending: false,
        }));

        assert_eq!(reducer.pm_queue.len(), 1);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));

        let msg = rx.try_recv().expect("Should have sent a move message");
        if let ClientMessage::MovePiece { piece_id: pid, target: t } = msg {
            assert_eq!(pid, piece_id);
            assert_eq!(t, target);
        } else {
            panic!("Wrong message type");
        }

        assert!(reducer.pm_queue[0].pending);

        let mut confirmed_piece = reducer.state.pieces.get(&piece_id).unwrap().clone();
        confirmed_piece.position = target;
        let reducer = reducer.reduce(GameAction::UpdateState {
            players: vec![],
            pieces: vec![confirmed_piece],
            shops: vec![],
            removed_pieces: vec![],
            removed_players: vec![],
        });

        assert_eq!(reducer.pm_queue.len(), 0);
    }

    #[test]
    fn test_multi_premove_chain() {
        let reducer = setup();
        let piece_id = Uuid::new_v4();
        let piece = Piece {
            id: piece_id,
            owner_id: Some(Uuid::new_v4()),
            piece_type: PieceType::Rook,
            position: IVec2::new(0, 0),
            last_move_time: chrono::Utc::now().timestamp_millis(),
            cooldown_ms: 1000,
        };
        
        let mut next = (*reducer).clone();
        next.state.pieces.insert(piece_id, piece);
        let reducer = Rc::new(next);

        let reducer = reducer.reduce(GameAction::AddPmove(Pmove { piece_id, target: IVec2::new(0, 1), pending: false }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove { piece_id, target: IVec2::new(0, 2), pending: false }));

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _reducer_after_tick = reducer.clone().reduce(GameAction::Tick(MsgSender(tx)));
        assert!(rx.try_recv().is_err());

        let mut next = (*reducer).clone();
        next.state.pieces.get_mut(&piece_id).unwrap().last_move_time -= 2000;
        let reducer = Rc::new(next);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));
        let msg = rx.try_recv().expect("Should send move 1");
        assert!(matches!(msg, ClientMessage::MovePiece { target, .. } if target == IVec2::new(0, 1)));

        let mut p1 = reducer.state.pieces.get(&piece_id).unwrap().clone();
        p1.position = IVec2::new(0, 1);
        p1.last_move_time = chrono::Utc::now().timestamp_millis();
        let reducer = reducer.reduce(GameAction::UpdateState {
            players: vec![],
            pieces: vec![p1],
            shops: vec![],
            removed_pieces: vec![],
            removed_players: vec![],
        });

        assert_eq!(reducer.pm_queue.len(), 1);

        let mut next = (*reducer).clone();
        next.state.pieces.get_mut(&piece_id).unwrap().last_move_time -= 2000;
        let reducer = Rc::new(next);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));
        let msg = rx.try_recv().expect("Should send move 2");
        assert!(matches!(msg, ClientMessage::MovePiece { target, .. } if target == IVec2::new(0, 2)));
    }

    #[test]
    fn test_aggressive_cleanup() {
        let reducer = setup();
        let piece_id = Uuid::new_v4();
        let piece = Piece {
            id: piece_id,
            owner_id: Some(Uuid::new_v4()),
            piece_type: PieceType::Rook,
            position: IVec2::new(0, 0),
            last_move_time: 0,
            cooldown_ms: 0,
        };
        
        let mut next = (*reducer).clone();
        next.state.pieces.insert(piece_id, piece);
        let reducer = Rc::new(next);

        // Queue: (0,1), (0,2), (0,3)
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove { piece_id, target: IVec2::new(0, 1), pending: false }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove { piece_id, target: IVec2::new(0, 2), pending: false }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove { piece_id, target: IVec2::new(0, 3), pending: false }));

        assert_eq!(reducer.pm_queue.len(), 3);

        // Server confirms (0,2) directly (maybe (0,1) update was missed)
        let mut p_at_2 = reducer.state.pieces.get(&piece_id).unwrap().clone();
        p_at_2.position = IVec2::new(0, 2);
        let reducer = reducer.reduce(GameAction::UpdateState {
            players: vec![],
            pieces: vec![p_at_2],
            shops: vec![],
            removed_pieces: vec![],
            removed_players: vec![],
        });

        // Should have removed BOTH (0,1) and (0,2)
        assert_eq!(reducer.pm_queue.len(), 1);
        assert_eq!(reducer.pm_queue[0].target, IVec2::new(0, 3));
    }
}
