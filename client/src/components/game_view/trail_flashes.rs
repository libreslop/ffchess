//! Flashing of premove trails that disappear because their actions failed.

use crate::canvas::FailedTrail;
use crate::premoves::{FailedMove, PremoveLine, Projection};

/// One failed trail, and the squares its failed moves tried to move into, flashing since
/// `started_at`.
struct Flash {
    lines: Vec<PremoveLine>,
    failed_moves: Vec<FailedMove>,
    started_at: f64,
}

/// Tracks the drawn premove lines and flashes the ones that vanish because they failed.
pub(super) struct TrailFlashes {
    /// Number of dark-then-colored flashes.
    flashes: u32,
    /// Duration of each dark or colored phase, in milliseconds.
    phase_ms: f64,
    last_lines: Vec<PremoveLine>,
    active: Vec<Flash>,
}

impl TrailFlashes {
    /// Creates a tracker flashing `flashes` times, each phase lasting `phase_ms`.
    pub(super) fn new(flashes: u32, phase_ms: u32) -> Self {
        Self {
            flashes,
            phase_ms: phase_ms as f64,
            last_lines: Vec::new(),
            active: Vec::new(),
        }
    }

    /// Starts flashing the lines drawn for the previous projection that `projection` no
    /// longer draws because their actions failed, along with the targets of the actions that
    /// failed themselves.
    ///
    /// Lines that vanish because their actions ran, were cleared, or are hidden while their
    /// piece is dragged do not flash.
    pub(super) fn observe(&mut self, projection: &Projection, now: f64) {
        let failed: Vec<PremoveLine> = self
            .last_lines
            .iter()
            .filter(|line| {
                projection.failed.contains_key(&line.action) && !projection.lines.contains(line)
            })
            .copied()
            .collect();
        if !failed.is_empty() {
            let mut failed_moves: Vec<FailedMove> = Vec::new();
            for failed_move in failed
                .iter()
                .filter_map(|line| projection.failed.get(&line.action).copied().flatten())
            {
                if !failed_moves.contains(&failed_move) {
                    failed_moves.push(failed_move);
                }
            }
            self.active.push(Flash {
                lines: failed,
                failed_moves,
                started_at: now,
            });
        }
        self.last_lines = projection.lines.clone();
    }

    /// Returns the trails flashing at `now`, dropping finished flashes.
    ///
    /// Each flash starts dark and alternates with the trail's color every phase.
    pub(super) fn trails_at(&mut self, now: f64) -> Vec<FailedTrail> {
        let duration = 2.0 * self.flashes as f64 * self.phase_ms;
        self.active
            .retain(|flash| now - flash.started_at < duration);
        self.active
            .iter()
            .map(|flash| FailedTrail {
                lines: flash.lines.clone(),
                failed_moves: flash.failed_moves.clone(),
                dark: (((now - flash.started_at) / self.phase_ms) as u64).is_multiple_of(2),
            })
            .collect()
    }
}
