#[cfg(test)]
mod tests {
    use server::state::ServerState;
    use common::*;
    use glam::IVec2;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_player_spawn_and_kits() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        let p1 = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        let p2 = state.add_player("P2".to_string(), KitType::Tank, tx.clone()).await;
        
        let game = state.game.read().await;
        assert_eq!(game.players.len(), 2);
        
        let p1_pieces: Vec<_> = game.pieces.values().filter(|p| p.owner_id == Some(p1)).collect();
        // Standard kit: 1 King + 4 pieces = 5 pieces total
        assert_eq!(p1_pieces.len(), 5);
        
        let p2_pieces: Vec<_> = game.pieces.values().filter(|p| p.owner_id == Some(p2)).collect();
        // Tank kit: 1 King + 1 piece = 2 pieces total
        assert_eq!(p2_pieces.len(), 2);
    }

    #[tokio::test]
    async fn test_king_capture_elimination() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        let p1_id = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        let p2_id = state.add_player("P2".to_string(), KitType::Standard, tx.clone()).await;
        
        let p1_king_id = {
            let game = state.game.read().await;
            game.players.get(&p1_id).unwrap().king_id
        };
        
        let p2_queen_id = {
            let mut game = state.game.write().await;
            let q_id = Uuid::new_v4();
            game.pieces.insert(q_id, Piece {
                id: q_id,
                owner_id: Some(p2_id),
                piece_type: PieceType::Queen,
                position: IVec2::new(10, 10),
                last_move_time: 0,
                cooldown_ms: 0,
            });
            q_id
        };

        // Move P1 king to (11, 11) and P2 queen to (10, 10)
        {
            let mut game = state.game.write().await;
            game.pieces.get_mut(&p1_king_id).unwrap().position = IVec2::new(11, 11);
            game.pieces.get_mut(&p2_queen_id).unwrap().position = IVec2::new(10, 10);
        }

        // P2 Queen captures P1 King
        state.handle_move(p2_id, p2_queen_id, IVec2::new(11, 11)).await.unwrap();
        
        let game = state.game.read().await;
        assert!(!game.players.contains_key(&p1_id));
        assert!(game.players.contains_key(&p2_id));
        
        // P1's pieces should all be removed
        let p1_piece_count = game.pieces.values().filter(|p| p.owner_id == Some(p1_id)).count();
        assert_eq!(p1_piece_count, 0);
        
        // P2 should have gained score (King value is 500)
        assert_eq!(game.players.get(&p2_id).unwrap().score, 500);
    }
}
