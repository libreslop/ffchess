#[cfg(test)]
mod tests {
    use server::state::ServerState;
    use common::*;
    use glam::IVec2;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_capture_records_removal() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        let p1_id = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        let p2_id = state.add_player("P2".to_string(), KitType::Standard, tx.clone()).await;
        
        let p1_pawn_id = {
            let game = state.game.read().await;
            *game.pieces.iter().find(|(_, p)| p.owner_id == Some(p1_id) && p.piece_type == PieceType::Pawn).unwrap().0
        };
        
        let p2_rook_id = {
            let mut game = state.game.write().await;
            let r_id = Uuid::new_v4();
            game.pieces.insert(r_id, Piece {
                id: r_id,
                owner_id: Some(p2_id),
                piece_type: PieceType::Rook,
                position: IVec2::new(10, 10),
                last_move_time: 0,
                cooldown_ms: 0,
            });
            r_id
        };

        // Set positions for capture
        {
            let mut game = state.game.write().await;
            game.pieces.get_mut(&p1_pawn_id).unwrap().position = IVec2::new(10, 5);
            game.pieces.get_mut(&p2_rook_id).unwrap().position = IVec2::new(10, 10);
        }

        // P2 Rook captures P1 Pawn
        state.handle_move(p2_id, p2_rook_id, IVec2::new(10, 5)).await.unwrap();
        
        // Check if removal is recorded
        let removed = state.removed_pieces.read().await;
        assert!(removed.contains(&p1_pawn_id));
    }

    #[tokio::test]
    async fn test_player_disconnect_records_removal() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        let p1_id = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        
        let p1_pieces: Vec<Uuid> = {
            let game = state.game.read().await;
            game.pieces.iter()
                .filter(|(_, p)| p.owner_id == Some(p1_id))
                .map(|(id, _)| *id)
                .collect()
        };
        
        state.remove_player(p1_id).await;
        
        let removed_pieces = state.removed_pieces.read().await;
        let removed_players = state.removed_players.read().await;
        
        assert!(removed_players.contains(&p1_id));
        for id in p1_pieces {
            assert!(removed_pieces.contains(&id));
        }
    }

    #[tokio::test]
    async fn test_handle_tick_clears_buffers() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        let p1_id = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        state.remove_player(p1_id).await;
        
        // Before tick, buffers should be full
        {
            assert!(!state.removed_pieces.read().await.is_empty());
            assert!(!state.removed_players.read().await.is_empty());
        }

        state.handle_tick().await;

        // After tick, buffers should be empty
        {
            assert!(state.removed_pieces.read().await.is_empty());
            assert!(state.removed_players.read().await.is_empty());
        }
    }

    #[tokio::test]
    async fn test_npc_capture_records_removal() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        
        // Add player and NPC
        let p1_id = state.add_player("P1".to_string(), KitType::Standard, tx.clone()).await;
        
        let p1_pawn_id = {
            let game = state.game.read().await;
            *game.pieces.iter().find(|(_, p)| p.owner_id == Some(p1_id) && p.piece_type == PieceType::Pawn).unwrap().0
        };

        let npc_id = Uuid::new_v4();
        {
            let mut game = state.game.write().await;
            // Place player King at (5,5) so NPC is visible and aggressive
            let king_id = game.players.get(&p1_id).unwrap().king_id;
            game.pieces.get_mut(&king_id).unwrap().position = IVec2::new(5, 5);
            
            game.pieces.insert(npc_id, Piece {
                id: npc_id,
                owner_id: None,
                piece_type: PieceType::Rook,
                position: IVec2::new(5, 10),
                last_move_time: 0,
                cooldown_ms: 0,
            });
            // Move player pawn to (5, 8) - rook at (5, 10) can capture it
            game.pieces.get_mut(&p1_pawn_id).unwrap().position = IVec2::new(5, 8);
        }

        // Trick tick_npcs into capturing p1_pawn
        // NPCs move every 2s in Tick, and prioritize captures if visible.
        // We need to set last_move_time to way back.
        {
            let mut game = state.game.write().await;
            game.pieces.get_mut(&npc_id).unwrap().last_move_time = 0;
        }

        state.tick_npcs().await;
        
        let removed = state.removed_pieces.read().await;
        assert!(removed.contains(&p1_pawn_id));
    }
}
