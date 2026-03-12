#[cfg(test)]
mod tests {
    use common::*;
    use server::state::ServerState;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_color_assignment_uniqueness() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();

        let mut colors = std::collections::HashSet::new();
        for i in 0..5 {
            let id = state.add_player(format!("P{}", i), KitType::Standard, tx.clone(), None).await;
            let game = state.game.read().await;
            let player = game.players.get(&id).unwrap();
            assert!(colors.insert(player.color.clone()), "Duplicate color assigned");
        }
    }

    #[tokio::test]
    async fn test_color_persistence_on_rejoin() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        let player_id = Uuid::new_v4();

        // Join first time
        state.add_player("P1".to_string(), KitType::Standard, tx.clone(), Some(player_id)).await;
        let color1 = {
            let game = state.game.read().await;
            game.players.get(&player_id).unwrap().color.clone()
        };

        // Die/Remove
        state.remove_player(player_id).await;

        // Rejoin immediately
        state.add_player("P1".to_string(), KitType::Standard, tx.clone(), Some(player_id)).await;
        let color2 = {
            let game = state.game.read().await;
            game.players.get(&player_id).unwrap().color.clone()
        };

        assert_eq!(color1, color2, "Player did not keep color on immediate rejoin");
    }

    #[tokio::test]
    async fn test_color_release_after_timeout() {
        let state = ServerState::new();
        let (tx, _) = mpsc::unbounded_channel();
        let p1_id = Uuid::new_v4();

        // P1 joins and gets a color (Red is first in PREFERRED_COLORS)
        state.add_player("P1".to_string(), KitType::Standard, tx.clone(), Some(p1_id)).await;
        let p1_color = {
            let game = state.game.read().await;
            game.players.get(&p1_id).unwrap().color.clone()
        };
        assert_eq!(p1_color, "#dc2626"); // First preferred color

        // P1 dies
        state.remove_player(p1_id).await;

        // Manually manipulate ColorManager to simulate 61 seconds passing
        {
            let mut cm = state.color_manager.write().await;
            if let Some(last_active) = cm.color_last_active.get_mut(&p1_color) {
                *last_active -= 61;
            }
        }

        // P2 joins, should be able to take P1's color
        let p2_id = Uuid::new_v4();
        state.add_player("P2".to_string(), KitType::Standard, tx.clone(), Some(p2_id)).await;
        let p2_color = {
            let game = state.game.read().await;
            game.players.get(&p2_id).unwrap().color.clone()
        };

        assert_eq!(p1_color, p2_color, "P2 did not take expired color from P1");
    }
}
