use common::*;
use glam::IVec2;
use server::state::ServerState;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::test]
async fn test_idle_optimization() {
    let state = Arc::new(ServerState::new());
    let now = chrono::Utc::now().timestamp_millis();

    // Initially, it should be considered "viewed" recently (initialized in new())
    {
        let last_viewed = *state.last_viewed_at.read().await;
        assert!(now - last_viewed < 1000);
    }

    // Set last_viewed_at to 6 seconds ago
    {
        let mut last_viewed = state.last_viewed_at.write().await;
        *last_viewed = now - 6000;
    }

    // Handle tick with no players - should NOT update last_viewed_at
    state.handle_tick().await;
    {
        let last_viewed = *state.last_viewed_at.read().await;
        assert!(now - last_viewed >= 6000);
    }

    // Add a player channel (simulating someone viewing the board)
    let (tx, _rx) = mpsc::unbounded_channel();
    state
        .player_channels
        .write()
        .await
        .insert(Uuid::new_v4(), tx);

    // Handle tick with a player - SHOULD update last_viewed_at
    state.handle_tick().await;
    {
        let last_viewed = *state.last_viewed_at.read().await;
        assert!(chrono::Utc::now().timestamp_millis() - last_viewed < 1000);
    }
}

#[tokio::test]
async fn test_shop_behavior_pawns() {
    let state = Arc::new(ServerState::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let player_id = state
        .add_player("Test".to_string(), KitType::Standard, tx, None)
        .await;

    // Set score high enough
    {
        let mut game = state.game.write().await;
        game.players.get_mut(&player_id).unwrap().score = 1000;
        // Ensure there is a shop at (0,0)
        game.shops.push(Shop {
            position: IVec2::new(0, 0),
            uses_remaining: 1,
            shop_type: ShopType::Spawn,
        });
        // Put a piece on the shop
        let p_id = Uuid::new_v4();
        game.pieces.insert(
            p_id,
            Piece {
                id: p_id,
                owner_id: Some(player_id),
                piece_type: PieceType::Knight,
                position: IVec2::new(0, 0),
                last_move_time: 0,
                cooldown_ms: 0,
            },
        );
    }

    let initial_piece_count = {
        let game = state.game.read().await;
        game.pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count()
    };

    // Buy a pawn
    state
        .handle_shop_buy(player_id, IVec2::new(0, 0), PieceType::Pawn)
        .await
        .expect("Should succeed");

    // Check that we have one more piece
    {
        let game = state.game.read().await;
        let current_count = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();
        assert_eq!(current_count, initial_piece_count + 1);

        // Ensure the original piece is still there and still a Knight
        let piece_on_shop = game
            .pieces
            .values()
            .find(|p| p.position == IVec2::new(0, 0))
            .unwrap();
        assert_eq!(piece_on_shop.piece_type, PieceType::Knight);
    }
}

#[tokio::test]
async fn test_shop_behavior_upgrade() {
    let state = Arc::new(ServerState::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let player_id = state
        .add_player("Test".to_string(), KitType::Standard, tx, None)
        .await;

    let piece_id = Uuid::new_v4();
    {
        let mut game = state.game.write().await;
        game.players.get_mut(&player_id).unwrap().score = 1000;
        game.shops.push(Shop {
            position: IVec2::new(5, 5),
            uses_remaining: 1,
            shop_type: ShopType::Upgrade,
        });
        // Put a Knight on the shop
        game.pieces.insert(
            piece_id,
            Piece {
                id: piece_id,
                owner_id: Some(player_id),
                piece_type: PieceType::Knight,
                position: IVec2::new(5, 5),
                last_move_time: 0,
                cooldown_ms: 0,
            },
        );
    }

    // Upgrade Knight to Queen
    state
        .handle_shop_buy(player_id, IVec2::new(5, 5), PieceType::Queen)
        .await
        .expect("Should succeed");

    {
        let game = state.game.read().await;
        let piece = game.pieces.get(&piece_id).unwrap();
        assert_eq!(piece.piece_type, PieceType::Queen);
        assert_eq!(piece.position, IVec2::new(5, 5));
    }
}

#[tokio::test]
async fn test_king_can_only_recruit_pawns() {
    let state = Arc::new(ServerState::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let player_id = state
        .add_player("Test".to_string(), KitType::Standard, tx, None)
        .await;

    let king_pos = {
        let game = state.game.read().await;
        let player = game.players.get(&player_id).unwrap();
        let king = game.pieces.get(&player.king_id).unwrap();
        king.position
    };

    {
        let mut game = state.game.write().await;
        game.players.get_mut(&player_id).unwrap().score = 1000;
        game.shops.push(Shop {
            position: king_pos,
            uses_remaining: 2,
            shop_type: ShopType::Upgrade,
        });
    }

    // 1. Try to upgrade King - should fail
    let result = state
        .handle_shop_buy(player_id, king_pos, PieceType::Queen)
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), GameError::KingRestrictedShop);

    // 2. Try to recruit Pawn with King - should succeed
    state
        .handle_shop_buy(player_id, king_pos, PieceType::Pawn)
        .await
        .expect("Should succeed");

    {
        let game = state.game.read().await;
        let player = game.players.get(&player_id).unwrap();
        let piece_count = game
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();
        // Standard kit starts with King + 4 pieces = 5. After recruiting one pawn, should be 6.
        assert_eq!(piece_count, 6);

        // Ensure King is still a King
        let king = game.pieces.get(&player.king_id).unwrap();
        assert_eq!(king.piece_type, PieceType::King);
    }
}
