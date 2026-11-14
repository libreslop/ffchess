//! Identity of one websocket connection as seen by the lobby.

use super::bindings::BindingSlot;
use crate::outbound::ClientTx;
use crate::types::ConnectionId;
use common::protocol::{ActionRequest, GameError, ServerMessage};
use common::types::{ModeId, PieceId};

/// A websocket connection opened for one public mode.
///
/// Clones are handles to the same connection.
#[derive(Clone)]
pub struct Connection {
    pub(super) id: ConnectionId,
    pub(super) mode_id: ModeId,
    pub(super) tx: ClientTx,
    /// True when the mode matches players through a queue instead of a shared world.
    pub(super) is_queue_mode: bool,
    pub(super) binding: BindingSlot,
}

impl Connection {
    /// Sends a message to this connection.
    pub(super) fn send(&self, message: ServerMessage) {
        self.tx.send(message);
    }

    /// Sends an error to this connection.
    pub(super) fn send_error(&self, error: GameError) {
        self.send(ServerMessage::Error(error));
    }

    /// Queues or runs a piece action for the player this connection controls.
    pub(super) fn submit_action(&self, request: ActionRequest) {
        if let Some(binding) = self.binding.get() {
            binding.instance.submit_action(binding.player_id, request);
        }
    }

    /// Cancels the queued actions of a piece of the player this connection controls.
    pub(super) fn clear_actions(&self, piece_id: PieceId) {
        if let Some(binding) = self.binding.get() {
            binding.instance.clear_actions(binding.player_id, piece_id);
        }
    }
}
