//! Preview boards shown to connections browsing a queue mode.
//!
//! Watchers of a queue mode see the most recently started running match. When that match
//! empties, the preview stays on it for the mode's switch delay before moving on, falling back
//! to the mode's empty public board. Watchers in "default only" mode (picking a kit or waiting
//! in the queue) always see the public board instead.

use super::connection::Connection;
use super::lobby::Lobby;
use crate::instance::GameInstance;
use crate::time::now_ms;
use crate::outbound::ClientTx;
use crate::types::ConnectionId;
use common::types::{ModeId, PlayerCount, TimestampMs};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Connections previewing one queue mode and the board they are shown.
#[derive(Default)]
pub(super) struct PreviewWatchers {
    /// Instance currently shown to dynamic watchers.
    target: Option<ModeId>,
    /// When the shown match lost its last player.
    empty_since: Option<TimestampMs>,
    watchers: HashMap<ConnectionId, ClientTx>,
    /// Watchers pinned to the public board.
    default_only: HashSet<ConnectionId>,
}

/// The board dynamic watchers should see and the state of its empty timer.
#[derive(Debug, Clone, PartialEq)]
struct PreviewChoice {
    target: Option<ModeId>,
    empty_since: Option<TimestampMs>,
}

impl PreviewWatchers {
    /// Returns the watchers that follow running matches.
    fn dynamic_watchers(&self) -> Vec<(ConnectionId, ClientTx)> {
        self.watchers
            .iter()
            .filter(|(conn_id, _)| !self.default_only.contains(*conn_id))
            .map(|(conn_id, tx)| (*conn_id, tx.clone()))
            .collect()
    }

    /// Returns the current choice.
    fn choice(&self) -> PreviewChoice {
        PreviewChoice {
            target: self.target.clone(),
            empty_since: self.empty_since,
        }
    }
}

impl Lobby {
    /// Makes a connection a preview watcher of its queue mode unless it already is one.
    pub(super) fn ensure_preview_watcher(&mut self, conn: &Connection) {
        let watching = self
            .previews
            .get(&conn.mode_id)
            .is_some_and(|preview| preview.watchers.contains_key(&conn.id));
        if !watching {
            self.add_preview_watcher(conn);
        }
    }

    /// Stops previewing for a connection.
    pub(super) fn remove_preview_watcher(&mut self, mode_id: &ModeId, conn_id: ConnectionId) {
        let Some(preview) = self.previews.get_mut(mode_id) else {
            return;
        };
        preview.watchers.remove(&conn_id);
        let was_default = preview.default_only.remove(&conn_id);
        let target = preview.target.clone();
        if preview.watchers.is_empty() {
            self.previews.remove(mode_id);
        }

        if let Some(instance) = target.and_then(|id| self.games.get(&id)) {
            instance.remove_viewer(conn_id);
        }
        if was_default && let Some(instance) = self.public_instance(mode_id) {
            instance.remove_viewer(conn_id);
        }
    }

    /// Pins a watcher to the public board, or releases it to follow running matches.
    ///
    /// Ignored while the connection still competes in a match: clients request previews as
    /// soon as they start joining, which can arrive after the join already started a match.
    /// Pinning takes the connection out of a match it has finished (lost or won).
    pub(super) fn set_preview_default(&mut self, conn: &Connection, enabled: bool) {
        if let Some(binding) = conn.binding.get() {
            if binding.instance.is_competing(binding.player_id) {
                return;
            }
            if enabled {
                conn.binding.detach();
                self.cleanup_private_games();
            }
        }
        self.ensure_preview_watcher(conn);

        let Some(preview) = self.previews.get_mut(&conn.mode_id) else {
            return;
        };
        if preview.default_only.contains(&conn.id) == enabled {
            return;
        }
        if enabled {
            preview.default_only.insert(conn.id);
        } else {
            preview.default_only.remove(&conn.id);
        }
        let target = preview.target.clone();

        let (from, to) = if enabled {
            let current = target.and_then(|id| self.games.get(&id).cloned());
            (current, self.public_instance(&conn.mode_id).cloned())
        } else {
            let to = match target {
                Some(id) => self.games.get(&id).cloned(),
                None => self.select_preview_target(&conn.mode_id),
            };
            (self.public_instance(&conn.mode_id).cloned(), to)
        };
        if let Some(from) = from {
            from.remove_viewer(conn.id);
        }
        if let Some(to) = to {
            self.watch(&to, conn.id, &conn.tx);
            if !enabled {
                self.retarget_preview(&conn.mode_id, &to);
            }
        }
    }

    /// Re-evaluates the preview target of every previewed mode.
    pub(super) fn refresh_previews(&mut self) {
        let mode_ids: Vec<ModeId> = self.previews.keys().cloned().collect();
        for mode_id in mode_ids {
            self.refresh_preview(&mode_id);
        }
    }

    /// Switches the dynamic watchers of a mode to a better match when needed.
    pub(super) fn refresh_preview(&mut self, mode_id: &ModeId) {
        let Some(preview) = self.previews.get(mode_id) else {
            return;
        };
        let watchers = preview.dynamic_watchers();
        if watchers.is_empty() {
            return;
        }
        let current = preview.choice();
        let next = self.next_preview_choice(mode_id, &current, now_ms());
        if next == current {
            return;
        }

        if next.target != current.target {
            if let Some(old) = current.target.as_ref().and_then(|id| self.games.get(id)) {
                for (conn_id, _) in &watchers {
                    old.remove_viewer(*conn_id);
                }
            }
            if let Some(new) = next.target.as_ref().and_then(|id| self.games.get(id)) {
                for (conn_id, tx) in &watchers {
                    self.watch(new, *conn_id, tx);
                }
            }
        }
        if let Some(preview) = self.previews.get_mut(mode_id) {
            preview.target = next.target;
            preview.empty_since = next.empty_since;
        }
    }

    /// Adds a preview watcher and shows it the current preview board.
    fn add_preview_watcher(&mut self, conn: &Connection) {
        let preview = self.previews.entry(conn.mode_id.clone()).or_default();
        preview.watchers.insert(conn.id, conn.tx.clone());
        let target = preview.target.clone();

        let instance = target
            .and_then(|id| self.games.get(&id).cloned())
            .or_else(|| self.select_preview_target(&conn.mode_id));
        if let Some(instance) = instance {
            self.watch(&instance, conn.id, &conn.tx);
            self.retarget_preview(&conn.mode_id, &instance);
        }
    }

    /// Records `instance` as the preview target of a mode, resetting the empty timer on change.
    fn retarget_preview(&mut self, mode_id: &ModeId, instance: &GameInstance) {
        if let Some(preview) = self.previews.get_mut(mode_id)
            && preview.target.as_ref() != Some(instance.mode_id())
        {
            preview.target = Some(instance.mode_id().clone());
            preview.empty_since = None;
        }
    }

    /// Decides which board dynamic watchers should see next.
    fn next_preview_choice(
        &self,
        mode_id: &ModeId,
        current: &PreviewChoice,
        now: TimestampMs,
    ) -> PreviewChoice {
        let fresh = |target: Option<ModeId>| PreviewChoice {
            target,
            empty_since: None,
        };
        let latest = self.latest_running_match(mode_id);
        let default = self.public_instance(mode_id).map(|i| i.mode_id().clone());

        match &current.target {
            Some(target) if target != mode_id => {
                let players = self
                    .games
                    .get(target)
                    .map_or_else(PlayerCount::zero, |instance| instance.player_count());
                if players > PlayerCount::zero() {
                    return fresh(current.target.clone());
                }
                let delay = self
                    .mode(mode_id)
                    .map(|mode| mode.preview_switch_delay_ms)
                    .unwrap_or_default();
                match current.empty_since {
                    None => PreviewChoice {
                        target: current.target.clone(),
                        empty_since: Some(now),
                    },
                    Some(since) if now - since < delay => current.clone(),
                    Some(_) => fresh(latest.map(|i| i.mode_id().clone()).or(default)),
                }
            }
            _ => match latest {
                Some(latest) => fresh(Some(latest.mode_id().clone())),
                None if current.target.is_none() => fresh(default),
                None => fresh(current.target.clone()),
            },
        }
    }

    /// Returns the instance dynamic watchers should start on.
    fn select_preview_target(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        self.latest_running_match(mode_id)
            .or_else(|| self.public_instance(mode_id).cloned())
    }

    /// Returns the most recently started instance of a mode that still has players.
    fn latest_running_match(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        self.instances_of(mode_id)
            .filter(|instance| instance.player_count() > PlayerCount::zero())
            .max_by_key(|instance| instance.last_started_at())
            .cloned()
    }
}
