use common::*;
use server::state::ServerState;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::test]
async fn test_respawn_cooldown_enforcement() {
    let state = Arc::new(ServerState::new());
    let player_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel();

    // 1. Join for the first time
    let (_pid, secret) = state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id), None)
        .await
        .expect("Initial join should succeed");

    // 2. Kill the player
    state.remove_player(player_id).await;

    // 3. Attempt immediate rejoin (should fail)
    let result = state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id), Some(secret))
        .await;

    assert!(result.is_err());
    if let Err(GameError::Custom { title, .. }) = result {
        assert_eq!(title, "Respawn cooldown");
    } else {
        panic!("Should have failed with Respawn cooldown");
    }

    // 4. Manipulate death timestamp to "bypass" time (for testing)
    {
        let mut deaths = state.death_timestamps.write().await;
        deaths.insert(player_id, 0); // Way in the past
    }

    // 5. Attempt rejoin again (should succeed)
    let result = state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id), Some(secret))
        .await;

    assert!(result.is_ok());

    // 6. Verify death timestamp is cleared
    {
        let deaths = state.death_timestamps.read().await;
        assert!(!deaths.contains_key(&player_id));
    }
}

#[tokio::test]
async fn test_session_hijack_prevention() {
    let state = Arc::new(ServerState::new());
    let player_id = Uuid::new_v4();
    let (tx, _rx) = mpsc::unbounded_channel::<ServerMessage>();

    // 1. P1 joins
    let _ = state
        .add_player("P1".to_string(), KitType::Standard, tx.clone(), Some(player_id), None)
        .await
        .expect("P1 join should succeed");

    // 2. P2 attempts to hijack with same player_id but no secret (or wrong secret)
    let hijack_result = state
        .add_player("Hijacker".to_string(), KitType::Standard, tx.clone(), Some(player_id), None)
        .await;

    assert!(hijack_result.is_err());
    if let Err(GameError::Custom { title, .. }) = hijack_result {
        assert_eq!(title, "SESSION ERROR");
    } else {
        panic!("Should have failed with SESSION ERROR");
    }

    let wrong_secret = Uuid::new_v4();
    let hijack_result_2 = state
        .add_player("Hijacker".to_string(), KitType::Standard, tx.clone(), Some(player_id), Some(wrong_secret))
        .await;

    assert!(hijack_result_2.is_err());
}
