use crate::instance::GameInstance;
use crate::types::ConnectionId;
use common::protocol::ServerMessage;
use common::types::{ModeId, TimestampMs};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Tracks preview watchers and the active preview target for a matchmaking mode.
pub(in crate::state) struct ModePreviewState {
    pub(super) target_mode_id: Option<ModeId>,
    pub(super) empty_since: Option<TimestampMs>,
    watchers: HashMap<ConnectionId, mpsc::Sender<ServerMessage>>,
    default_only: HashSet<ConnectionId>,
}

impl ModePreviewState {
    pub(super) fn new() -> Self {
        Self {
            target_mode_id: None,
            empty_since: None,
            watchers: HashMap::new(),
            default_only: HashSet::new(),
        }
    }

    pub(super) fn insert_watcher(
        &mut self,
        conn_id: ConnectionId,
        tx: mpsc::Sender<ServerMessage>,
    ) {
        self.watchers.insert(conn_id, tx);
    }

    pub(super) fn remove_watcher(
        &mut self,
        conn_id: ConnectionId,
    ) -> Option<mpsc::Sender<ServerMessage>> {
        self.watchers.remove(&conn_id)
    }

    pub(super) fn has_watcher(&self, conn_id: ConnectionId) -> bool {
        self.watchers.contains_key(&conn_id)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.watchers.is_empty()
    }

    pub(super) fn mark_default_only(&mut self, conn_id: ConnectionId, enabled: bool) {
        if enabled {
            self.default_only.insert(conn_id);
        } else {
            self.default_only.remove(&conn_id);
        }
    }

    pub(super) fn is_default_only(&self, conn_id: ConnectionId) -> bool {
        self.default_only.contains(&conn_id)
    }

    pub(super) fn clear_default_only(&mut self, conn_id: ConnectionId) -> bool {
        self.default_only.remove(&conn_id)
    }

    pub(super) fn snapshot(&self) -> PreviewSnapshot {
        let dynamic_watchers = self
            .watchers
            .iter()
            .filter(|(conn_id, _)| !self.default_only.contains(*conn_id))
            .map(|(conn_id, tx)| (*conn_id, tx.clone()))
            .collect::<HashMap<_, _>>();
        PreviewSnapshot {
            dynamic_watchers,
            target_mode_id: self.target_mode_id.clone(),
            empty_since: self.empty_since,
        }
    }
}

/// Captures the state needed to evaluate a preview switch.
pub(super) struct PreviewSnapshot {
    pub(super) dynamic_watchers: HashMap<ConnectionId, mpsc::Sender<ServerMessage>>,
    pub(super) target_mode_id: Option<ModeId>,
    pub(super) empty_since: Option<TimestampMs>,
}

/// Represents the selected preview target and empty timer state.
pub(super) struct PreviewSelection {
    pub(super) target_mode_id: Option<ModeId>,
    pub(super) empty_since: Option<TimestampMs>,
}

/// Captures the current target state while evaluating preview selection.
pub(super) struct CurrentPreviewState<'a> {
    pub(super) target_mode_id: Option<ModeId>,
    pub(super) empty_since: Option<TimestampMs>,
    pub(super) instance: Option<&'a Arc<GameInstance>>,
}

/// Bundles all inputs required to compute the next preview selection.
pub(super) struct PreviewSelectionInput<'a> {
    pub(super) mode_id: &'a ModeId,
    pub(super) now: TimestampMs,
    pub(super) current: CurrentPreviewState<'a>,
    pub(super) default_instance: Option<&'a Arc<GameInstance>>,
    pub(super) latest_running: Option<&'a Arc<GameInstance>>,
}
