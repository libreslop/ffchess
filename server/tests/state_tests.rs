//! Tests for server state and gameplay interactions.

#[cfg(test)]
mod tests {
    use common::models::Piece;
    use common::types::{DurationMs, KitId, ModeId, PieceId, PieceTypeId, Score, TimestampMs};
    use glam::IVec2;
    use server::state::ServerState;
    use tokio::sync::mpsc;

    #[tokio::test]
    /// Verifies players spawn with correct kit piece counts.
    async fn test_player_spawn_and_kits() {
        let state = ServerState::new();
        let instance = state
            .get_game(&ModeId::from("ffa"))
            .await
            .expect("FFA game should exist");
        let (tx, _) = mpsc::unbounded_channel();

        let (p1, _p1_secret) = instance
            .add_player(
                "P1".to_string(),
                KitId::from("Standard"),
                tx.clone(),
                None,
                None,
            )
            .await
            .expect("Initial join should succeed");
        let (p2, _p2_secret) = instance
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
            .get_game(&ModeId::from("ffa"))
            .await
            .expect("FFA game should exist");
        let (tx, _) = mpsc::unbounded_channel();

        let (p1_id, _p1_secret) = instance
            .add_player(
                "P1".to_string(),
                KitId::from("Standard"),
                tx.clone(),
                None,
                None,
            )
            .await
            .expect("Initial join should succeed");
        let (p2_id, _p2_secret) = instance
            .add_player(
                "P2".to_string(),
                KitId::from("Standard"),
                tx.clone(),
                None,
                None,
            )
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
                    position: IVec2::new(10, 10),
                    last_move_time: TimestampMs::from_millis(0),
                    cooldown_ms: DurationMs::zero(),
                },
            );
            q_id
        };

        // Move P1 king to (11, 11) and P2 queen to (10, 10)
        {
            let mut game = instance.game.write().await;
            game.pieces.get_mut(&p1_king_id).unwrap().position = IVec2::new(11, 11);
            game.pieces.get_mut(&p2_queen_id).unwrap().position = IVec2::new(10, 10);
        }

        // P2 Queen captures P1 King
        instance
            .handle_move(p2_id, p2_queen_id, IVec2::new(11, 11))
            .await
            .unwrap();

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
}
