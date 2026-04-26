//! Tests for server state and gameplay interactions.

#[cfg(test)]
mod tests {
    use common::models::Piece;
    use common::protocol::ServerMessage;
    use common::types::{
        BoardCoord, DurationMs, KitId, ModeId, PieceId, PieceTypeId, Score, TimestampMs,
    };
    use glam::IVec2;
    use server::state::ServerState;
    use server::time::now_ms;
    use tokio::sync::mpsc;

    #[tokio::test]
    /// Verifies players spawn with correct kit piece counts.
    async fn test_player_spawn_and_kits() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("ffa"))
            .await
            .expect("FFA game should exist");
        let (tx, _) = mpsc::channel(100);

        let (p1, _) = instance
            .add_player(
                "P1".to_string(),
                KitId::from("Standard"),
                tx.clone(),
                None,
                None,
            )
            .await
            .expect("Initial join should succeed");
        let (p2, _) = instance
            .add_player(
                "P2".to_string(),
                KitId::from("Tank"),
                tx.clone(),
                None,
                None,
            )
            .await
            .expect("Initial join should succeed");

        let game = instance.game.read().await;
        assert_eq!(game.players.len(), 2);

        let p1_pieces: Vec<_> = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(p1))
            .collect();
        // Standard kit: 5 pieces total, including the king.
        assert_eq!(p1_pieces.len(), 5);

        let p2_pieces: Vec<_> = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(p2))
            .collect();
        // Tank kit: "pieces": ["king", "rook", "pawn"]
        assert_eq!(p2_pieces.len(), 3);
    }

    #[tokio::test]
    /// Verifies capturing a king eliminates the owning player.
    async fn test_king_capture_elimination() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("ffa"))
            .await
            .expect("FFA game should exist");
        let (tx1, _rx1) = mpsc::channel(100);
        let (tx2, _rx2) = mpsc::channel(100);

        let (p1_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx1, None, None)
            .await
            .expect("Initial join should succeed");
        let (p2_id, _) = instance
            .add_player("P2".to_string(), KitId::from("Standard"), tx2, None, None)
            .await
            .expect("Initial join should succeed");

        let p1_king_id = {
            let game = instance.game.read().await;
            game.players.get(&p1_id).unwrap().king_id
        };

        // Add a queen for P2 near P1 king
        let p2_queen_id = {
            let mut game = instance.game.write().await;
            let q_id = PieceId::new();
            game.pieces.insert(
                q_id,
                Piece {
                    id: q_id,
                    owner_id: Some(p2_id),
                    piece_type: PieceTypeId::from("queen"),
                    position: BoardCoord(IVec2::new(10, 10)),
                    last_move_time: TimestampMs::from_millis(0),
                    cooldown_ms: DurationMs::zero(),
                },
            );
            q_id
        };

        // Move P1 king to (11, 11) and P2 queen to (10, 10)
        {
            let mut game = instance.game.write().await;
            game.pieces.get_mut(&p1_king_id).unwrap().position = BoardCoord(IVec2::new(11, 11));
            game.pieces.get_mut(&p2_queen_id).unwrap().position = BoardCoord(IVec2::new(10, 10));
        }

        // P2 Queen captures P1 King
        instance
            .handle_move(p2_id, p2_queen_id, BoardCoord(IVec2::new(11, 11)))
            .await
            .unwrap();

        instance.handle_tick().await;

        let game = instance.game.read().await;
        assert!(!game.players.contains_key(&p1_id));
        assert!(game.players.contains_key(&p2_id));

        // P1's pieces should all be removed
        let p1_piece_count = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(p1_id))
            .count();
        assert_eq!(p1_piece_count, 0);

        // P2 should have gained score (King value is 500)
        assert_eq!(game.players.get(&p2_id).unwrap().score, Score::from(500));
    }

    #[tokio::test]
    /// Verifies disconnecting in duel grants a win message to the remaining player.
    async fn test_duel_player_leave_win_hook() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx1, mut rx1) = mpsc::channel(100);
        let (tx2, mut rx2) = mpsc::channel(100);

        let (p1_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx1, None, None)
            .await
            .expect("P1 join should succeed");
        let (_p2_id, _) = instance
            .add_player("P2".to_string(), KitId::from("Standard"), tx2, None, None)
            .await
            .expect("P2 join should succeed");

        instance.remove_player(p1_id).await;

        let p1_msg = rx1.try_recv().expect("Leaver should receive GameOver");
        assert!(matches!(p1_msg, ServerMessage::GameOver { .. }));

        instance.handle_tick().await;

        let winner_msg = rx2
            .try_recv()
            .expect("Remaining player should receive win message");
        match winner_msg {
            ServerMessage::Victory { title, message, .. } => {
                assert_eq!(title, "VICTORY");
                assert_eq!(message, "Opponent disconnected");
            }
            other => panic!("Unexpected winner message: {:?}", other),
        }
    }

    #[tokio::test]
    /// Verifies capture-win hook takes precedence over leave-win hook in duel.
    async fn test_duel_capture_king_win_hook_precedes_leave_hook() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx1, _rx1) = mpsc::channel(100);
        let (tx2, mut rx2) = mpsc::channel(100);

        let (p1_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx1, None, None)
            .await
            .expect("P1 join should succeed");
        let (p2_id, _) = instance
            .add_player("P2".to_string(), KitId::from("Standard"), tx2, None, None)
            .await
            .expect("P2 join should succeed");

        let (_p1_king_id, p2_king_id) = {
            let mut game = instance.game.write().await;
            let p1_king_id = game.players.get(&p1_id).unwrap().king_id;
            let p2_king_id = game.players.get(&p2_id).unwrap().king_id;
            game.pieces
                .retain(|id, _| *id == p1_king_id || *id == p2_king_id);

            let p1_king = game.pieces.get_mut(&p1_king_id).unwrap();
            p1_king.position = BoardCoord(IVec2::new(1, 1));
            p1_king.last_move_time = TimestampMs::from_millis(0);
            p1_king.cooldown_ms = DurationMs::zero();

            let p2_king = game.pieces.get_mut(&p2_king_id).unwrap();
            p2_king.position = BoardCoord(IVec2::new(0, 0));
            p2_king.last_move_time = TimestampMs::from_millis(0);
            p2_king.cooldown_ms = DurationMs::zero();

            (p1_king_id, p2_king_id)
        };

        instance
            .handle_move(p2_id, p2_king_id, BoardCoord(IVec2::new(1, 1)))
            .await
            .expect("Capture should succeed");

        instance.handle_tick().await;

        let winner_msg = rx2.try_recv().expect("Capturer should receive win message");
        match winner_msg {
            ServerMessage::Victory { title, message, .. } => {
                assert_eq!(title, "VICTORY");
                assert_eq!(message, "");
            }
            other => panic!("Unexpected winner message: {:?}", other),
        }
    }

    #[tokio::test]
    /// Verifies king-capture victory does not trigger a delayed leave-based victory message.
    async fn test_duel_capture_king_does_not_emit_delayed_leave_victory() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx1, _rx1) = mpsc::channel(100);
        let (tx2, mut rx2) = mpsc::channel(100);

        let (p1_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx1, None, None)
            .await
            .expect("P1 join should succeed");
        let (p2_id, _) = instance
            .add_player("P2".to_string(), KitId::from("Standard"), tx2, None, None)
            .await
            .expect("P2 join should succeed");

        let p2_king_id = {
            let mut game = instance.game.write().await;
            let p1_king_id = game.players.get(&p1_id).unwrap().king_id;
            let p2_king_id = game.players.get(&p2_id).unwrap().king_id;
            game.pieces
                .retain(|id, _| *id == p1_king_id || *id == p2_king_id);

            let p1_king = game.pieces.get_mut(&p1_king_id).unwrap();
            p1_king.position = BoardCoord(IVec2::new(1, 1));
            p1_king.last_move_time = TimestampMs::from_millis(0);
            p1_king.cooldown_ms = DurationMs::zero();

            let p2_king = game.pieces.get_mut(&p2_king_id).unwrap();
            p2_king.position = BoardCoord(IVec2::new(0, 0));
            p2_king.last_move_time = TimestampMs::from_millis(0);
            p2_king.cooldown_ms = DurationMs::zero();
            p2_king_id
        };

        instance
            .handle_move(p2_id, p2_king_id, BoardCoord(IVec2::new(1, 1)))
            .await
            .expect("Capture should succeed");

        instance.handle_tick().await;

        let mut first_tick_victories = Vec::<String>::new();
        while let Ok(msg) = rx2.try_recv() {
            if let ServerMessage::Victory { message, .. } = msg {
                first_tick_victories.push(message);
            }
        }
        assert_eq!(first_tick_victories, vec!["".to_string()]);

        instance.handle_tick().await;

        let mut second_tick_victories = Vec::<String>::new();
        while let Ok(msg) = rx2.try_recv() {
            if let ServerMessage::Victory { message, .. } = msg {
                second_tick_victories.push(message);
            }
        }
        assert!(
            second_tick_victories.is_empty(),
            "No follow-up victory messages expected, got: {:?}",
            second_tick_victories
        );
    }

    #[tokio::test]
    /// Verifies a move submitted during cooldown is queued and executed on a later tick.
    async fn test_server_side_premove_executes_after_cooldown() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx, _rx) = mpsc::channel(100);

        let (player_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx, None, None)
            .await
            .expect("P1 join should succeed");

        let (king_id, start_position) = {
            let mut game = instance.game.write().await;
            let king_id = game.players.get(&player_id).expect("player exists").king_id;
            game.pieces.retain(|id, _| *id == king_id);

            let king = game.pieces.get_mut(&king_id).expect("king exists");
            king.position = BoardCoord(IVec2::new(0, 0));
            king.last_move_time = now_ms();
            king.cooldown_ms = DurationMs::from_millis(10_000);
            (king_id, king.position)
        };

        let target = BoardCoord(start_position.0 + IVec2::new(1, 0));
        instance
            .handle_move(player_id, king_id, target)
            .await
            .expect("Move request should be accepted and queued");

        {
            let game = instance.game.read().await;
            assert_eq!(
                game.pieces.get(&king_id).expect("king exists").position,
                start_position
            );
        }

        {
            let mut game = instance.game.write().await;
            let king = game.pieces.get_mut(&king_id).expect("king exists");
            king.last_move_time = TimestampMs::from_millis(0);
        }
        instance.handle_tick().await;

        let game = instance.game.read().await;
        assert_eq!(
            game.pieces.get(&king_id).expect("king exists").position,
            target
        );
    }

    #[tokio::test]
    /// Verifies chained premoves are queued server-side and executed in order.
    async fn test_server_side_chained_premoves_execute_in_order() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx, _rx) = mpsc::channel(100);

        let (player_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx, None, None)
            .await
            .expect("P1 join should succeed");

        let king_id = {
            let mut game = instance.game.write().await;
            let king_id = game.players.get(&player_id).expect("player exists").king_id;
            game.pieces.retain(|id, _| *id == king_id);

            let king = game.pieces.get_mut(&king_id).expect("king exists");
            king.position = BoardCoord(IVec2::new(0, 0));
            king.last_move_time = now_ms();
            king.cooldown_ms = DurationMs::from_millis(10_000);
            king_id
        };

        let first_target = BoardCoord(IVec2::new(1, 0));
        let second_target = BoardCoord(IVec2::new(2, 0));

        instance
            .handle_move(player_id, king_id, first_target)
            .await
            .expect("First premove should be queued");
        instance
            .handle_move(player_id, king_id, second_target)
            .await
            .expect("Second premove should be queued");

        {
            let mut game = instance.game.write().await;
            game.pieces
                .get_mut(&king_id)
                .expect("king exists")
                .last_move_time = TimestampMs::from_millis(0);
        }
        instance.handle_tick().await;
        {
            let game = instance.game.read().await;
            assert_eq!(
                game.pieces.get(&king_id).expect("king exists").position,
                first_target
            );
        }

        {
            let mut game = instance.game.write().await;
            game.pieces
                .get_mut(&king_id)
                .expect("king exists")
                .last_move_time = TimestampMs::from_millis(0);
        }
        instance.handle_tick().await;
        {
            let game = instance.game.read().await;
            assert_eq!(
                game.pieces.get(&king_id).expect("king exists").position,
                second_target
            );
        }
    }

    #[tokio::test]
    /// Verifies that premoves can be cleared via ClearPremoves message.
    async fn test_clear_premoves() {
        let state = ServerState::new();
        let instance = state
            .get_joinable_game(&ModeId::from("duel"))
            .await
            .expect("Duel game should exist");
        let (tx, _rx) = mpsc::channel(100);

        let (player_id, _) = instance
            .add_player("P1".to_string(), KitId::from("Standard"), tx, None, None)
            .await
            .expect("P1 join should succeed");

        let king_id = {
            let mut game = instance.game.write().await;
            let king_id = game.players.get(&player_id).expect("player exists").king_id;
            game.pieces.retain(|id, _| *id == king_id);

            let king = game.pieces.get_mut(&king_id).expect("king exists");
            king.position = BoardCoord(IVec2::new(0, 0));
            king.last_move_time = now_ms();
            king.cooldown_ms = DurationMs::from_millis(10_000);
            king_id
        };

        let target = BoardCoord(IVec2::new(1, 0));
        instance
            .handle_move(player_id, king_id, target)
            .await
            .expect("Move should be queued");

        // Clear premoves
        instance.clear_queued_moves(king_id).await;

        // Reset cooldown
        {
            let mut game = instance.game.write().await;
            let king = game.pieces.get_mut(&king_id).expect("king exists");
            king.last_move_time = TimestampMs::from_millis(0);
        }

        instance.handle_tick().await;

        // Should NOT have moved
        let game = instance.game.read().await;
        assert_eq!(
            game.pieces.get(&king_id).expect("king exists").position,
            BoardCoord(IVec2::new(0, 0))
        );
    }
}
