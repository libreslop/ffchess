pub mod actions;
pub mod handlers;
pub mod types;

pub use actions::*;
use handlers::*;
use std::rc::Rc;
pub use types::*;
use uuid::Uuid;
use yew::prelude::*;

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
                next.disconnected = false;
                if let Some(p) = next.state.players.get(&player_id) {
                    next.last_score = p.score;
                }
            }
            GameAction::UpdateState {
                players,
                pieces,
                shops,
                removed_pieces,
                removed_players,
                board_size,
            } => {
                handle_update_state(
                    &mut next,
                    players,
                    pieces,
                    shops,
                    removed_pieces,
                    removed_players,
                    board_size,
                );
            }
            GameAction::SetError(e) => {
                next.error = Some(e.clone());
                if matches!(e, GameError::OnCooldown) {
                    for pm in next.pm_queue.iter_mut() {
                        pm.pending = false;
                    }
                } else {
                    for pm in next.pm_queue.iter().rev() {
                        if pm.pending
                            && let Some(p) = next.state.pieces.get_mut(&pm.piece_id)
                        {
                            p.last_move_time = pm.old_last_move_time;
                            p.cooldown_ms = pm.old_cooldown_ms;
                        }
                    }
                    next.pm_queue.clear();
                }
            }
            GameAction::GameOver {
                final_score,
                kills,
                pieces_captured,
                time_survived_secs,
            } => {
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
                    if let Some(piece) = next.state.pieces.get(&pm.piece_id)
                        && now >= piece.last_move_time + piece.cooldown_ms + 50
                    {
                        let _ = tx.0.send(ClientMessage::MovePiece {
                            piece_id: pm.piece_id,
                            target: pm.target,
                        });
                        pm.pending = true;
                        processed_pieces.insert(pm.piece_id);

                        if let Some(p) = next.state.pieces.get_mut(&pm.piece_id) {
                            pm.old_last_move_time = p.last_move_time;
                            pm.old_cooldown_ms = p.cooldown_ms;
                            p.cooldown_ms = calculate_cooldown(
                                p.piece_type,
                                p.position,
                                pm.target,
                                &next.state.cooldown_config,
                            );
                            p.last_move_time = now;
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
            GameAction::SetDisconnected(d) => {
                next.disconnected = d;
            }
        }
        next.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::*;
    use glam::IVec2;
    use std::rc::Rc;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn setup() -> Rc<GameStateReducer> {
        let mut reducer = GameStateReducer::default();
        reducer.state.board_size = 25;
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
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));

        assert_eq!(reducer.pm_queue.len(), 1);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));

        let msg = rx.try_recv().expect("Should have sent a move message");
        if let ClientMessage::MovePiece {
            piece_id: pid,
            target: t,
        } = msg
        {
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
            board_size: 25,
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

        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target: IVec2::new(0, 1),
            pending: false,
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target: IVec2::new(0, 2),
            pending: false,
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _reducer_after_tick = reducer.clone().reduce(GameAction::Tick(MsgSender(tx)));
        assert!(rx.try_recv().is_err());

        let mut next = (*reducer).clone();
        next.state.pieces.get_mut(&piece_id).unwrap().last_move_time -= 2000;
        let reducer = Rc::new(next);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));
        let msg = rx.try_recv().expect("Should send move 1");
        assert!(
            matches!(msg, ClientMessage::MovePiece { target, .. } if target == IVec2::new(0, 1))
        );

        let mut p1 = reducer.state.pieces.get(&piece_id).unwrap().clone();
        p1.position = IVec2::new(0, 1);
        p1.last_move_time = chrono::Utc::now().timestamp_millis();
        let reducer = reducer.reduce(GameAction::UpdateState {
            players: vec![],
            pieces: vec![p1],
            shops: vec![],
            removed_pieces: vec![],
            removed_players: vec![],
            board_size: 25,
        });

        assert_eq!(reducer.pm_queue.len(), 1);

        let mut next = (*reducer).clone();
        next.state.pieces.get_mut(&piece_id).unwrap().last_move_time -= 2000;
        let reducer = Rc::new(next);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _reducer = reducer.reduce(GameAction::Tick(MsgSender(tx)));
        let msg = rx.try_recv().expect("Should send move 2");
        assert!(
            matches!(msg, ClientMessage::MovePiece { target, .. } if target == IVec2::new(0, 2))
        );
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
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target: IVec2::new(0, 1),
            pending: false,
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target: IVec2::new(0, 2),
            pending: false,
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));
        let reducer = reducer.reduce(GameAction::AddPmove(Pmove {
            piece_id,
            target: IVec2::new(0, 3),
            pending: false,
            old_last_move_time: 0,
            old_cooldown_ms: 0,
        }));

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
            board_size: 25,
        });

        // Should have removed BOTH (0,1) and (0,2)
        assert_eq!(reducer.pm_queue.len(), 1);
        assert_eq!(reducer.pm_queue[0].target, IVec2::new(0, 3));
    }
}
