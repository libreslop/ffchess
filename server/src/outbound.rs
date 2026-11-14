//! Outbound path from the lobby and game instances to one websocket connection.
//!
//! Each connection has two lanes. Board snapshots go through a watch channel that only keeps
//! the newest one, so a client that reads slowly gets a lower frame rate instead of a growing
//! backlog. Every other message is an event that must not be lost, such as an action result or
//! a chat line. Events wait in a small bounded queue, and a client that lets it fill up is
//! disconnected instead of silently missing one.

use common::protocol::ServerMessage;
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Notify, mpsc, watch};

/// A per-tick board snapshot, encoded once and shared by every connection watching the board.
#[derive(Clone)]
pub struct EncodedUpdate(Arc<str>);

impl EncodedUpdate {
    /// Encodes a snapshot message as the JSON text sent to clients.
    pub fn encode(message: &ServerMessage) -> Option<Self> {
        serde_json::to_string(message)
            .ok()
            .map(|json| Self(json.into()))
    }
}

/// Sending side of a connection's outbound lanes, held by the lobby and instances.
#[derive(Clone)]
pub struct ClientTx {
    events: mpsc::Sender<ServerMessage>,
    snapshots: watch::Sender<Option<EncodedUpdate>>,
    /// Signalled when the event queue overflows, telling the writer to close the connection.
    overflowed: Arc<Notify>,
}

impl ClientTx {
    /// Creates the lanes of one connection, allowing `event_capacity` unsent events.
    pub fn channel(event_capacity: usize) -> (Self, ClientFeed) {
        let (events_tx, events_rx) = mpsc::channel(event_capacity.max(1));
        let (snapshots_tx, snapshots_rx) = watch::channel(None);
        let overflowed = Arc::new(Notify::new());
        let tx = Self {
            events: events_tx,
            snapshots: snapshots_tx,
            overflowed: overflowed.clone(),
        };
        let feed = ClientFeed {
            events: events_rx,
            snapshots: snapshots_rx,
            overflowed,
        };
        (tx, feed)
    }

    /// Queues an event, closing the connection when its queue is full.
    pub fn send(&self, message: ServerMessage) {
        if let Err(TrySendError::Full(_)) = self.events.try_send(message) {
            self.overflowed.notify_one();
        }
    }

    /// Replaces the snapshot waiting to be written with `update`.
    pub fn publish(&self, update: &EncodedUpdate) {
        self.snapshots.send_replace(Some(update.clone()));
    }

    /// Returns true once the connection's writer is gone.
    pub fn is_closed(&self) -> bool {
        self.events.is_closed()
    }
}

/// Receiving side of a connection's outbound lanes, drained by its socket writer.
pub struct ClientFeed {
    events: mpsc::Receiver<ServerMessage>,
    snapshots: watch::Receiver<Option<EncodedUpdate>>,
    overflowed: Arc<Notify>,
}

impl ClientFeed {
    /// Waits for outbound traffic and returns it as JSON texts in the order they must be
    /// written.
    ///
    /// Returns `None` once the connection must close: its event queue overflowed, or every
    /// sender is gone.
    pub async fn next_batch(&mut self) -> Option<Vec<String>> {
        let mut events = Vec::new();
        let mut fresh_snapshot = false;
        tokio::select! {
            biased;
            () = self.overflowed.notified() => return None,
            changed = self.snapshots.changed() => {
                changed.ok()?;
                fresh_snapshot = true;
            }
            event = self.events.recv() => events.push(event?),
        }

        // The snapshot is read before the events are drained. Instances queue action results
        // before publishing the snapshot that reflects them, so every result this snapshot
        // reflects is already in the queue and goes out first.
        fresh_snapshot |= self.snapshots.has_changed().unwrap_or(false);
        let snapshot = fresh_snapshot
            .then(|| self.snapshots.borrow_and_update().clone())
            .flatten();
        events.extend(std::iter::from_fn(|| self.try_next_event()));

        // A snapshot read before a board switch may show the board being left, and the Init
        // of the new board carries its full state anyway.
        let snapshot = snapshot.filter(|_| !events.iter().any(ServerMessage::switches_board));
        let texts = events
            .iter()
            .filter_map(|event| serde_json::to_string(event).ok())
            .chain(snapshot.map(|update| update.0.to_string()))
            .collect();
        Some(texts)
    }

    /// Takes the next queued event without waiting.
    pub fn try_next_event(&mut self) -> Option<ServerMessage> {
        self.events.try_recv().ok()
    }
}
