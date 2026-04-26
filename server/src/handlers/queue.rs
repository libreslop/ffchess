use crate::state::ServerState;
use common::protocol::ServerMessage;
use common::types::{ModeId, QueuePosition};
use std::sync::Arc;

/// Broadcasts queue position/state updates to all queued players for a mode.
pub(super) async fn broadcast_queue_state(state: &Arc<ServerState>, mode_id: &ModeId) {
    let Some((required_players, entries)) = state.queue_snapshot(mode_id).await else {
        return;
    };

    let queued_players = common::types::PlayerCount::new(entries.len() as u32);
    for (idx, entry) in entries.iter().enumerate() {
        let _ = entry.tx().try_send(ServerMessage::QueueState {
            position_in_queue: QueuePosition::new((idx + 1) as u32),
            queued_players,
            required_players,
        });
    }
}
