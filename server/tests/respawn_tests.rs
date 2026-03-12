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
    state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id))
        .await
        .expect("Initial join should succeed");

    // 2. Kill the player
    state.remove_player(player_id).await;

    // 3. Attempt immediate rejoin (should fail)
    let result = state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id))
        .await;

    assert!(result.is_err());
    if let Err(GameError::Custom { title, .. }) = result {
        assert_eq!(title, "RESPAWN COOLDOWN");
    } else {
        panic!("Should have failed with RESPAWN COOLDOWN");
    }

    // 4. Manipulate death timestamp to "bypass" time (for testing)
    {
        let mut deaths = state.death_timestamps.write().await;
        deaths.insert(player_id, 0); // Way in the past
    }

    // 5. Attempt rejoin again (should succeed)
    let result = state
        .add_player("Test".to_string(), KitType::Standard, tx.clone(), Some(player_id))
        .await;

    assert!(result.is_ok());
}
