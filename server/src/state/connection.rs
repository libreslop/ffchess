//! Identity of one websocket connection as seen by the lobby.

use crate::types::{ClientTx, ConnectionId};
use common::protocol::{GameError, ServerMessage};
use common::types::ModeId;

/// A websocket connection opened for one public mode.
pub struct Connection {
    pub(super) id: ConnectionId,
    pub(super) mode_id: ModeId,
    pub(super) tx: ClientTx,
    /// True when the mode matches players through a queue instead of a shared world.
    pub(super) is_queue_mode: bool,
}

impl Connection {
    /// Sends a message to this connection, dropping it if the connection is saturated.
    pub(super) fn send(&self, message: ServerMessage) {
        let _ = self.tx.try_send(message);
    }

    /// Sends an error to this connection.
    pub(super) fn send_error(&self, error: GameError) {
        self.send(ServerMessage::Error(error));
    }
}
