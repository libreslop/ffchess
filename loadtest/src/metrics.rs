//! Counters and latency samples the bots report, read and reset once per load stage.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// A set of latency samples in milliseconds.
#[derive(Default)]
pub struct Samples(Mutex<Vec<u64>>);

impl Samples {
    /// Records one sample.
    pub fn record(&self, ms: u64) {
        self.0.lock().unwrap().push(ms);
    }

    /// Takes every sample recorded since the last call, sorted.
    fn take(&self) -> Vec<u64> {
        let mut samples = std::mem::take(&mut *self.0.lock().unwrap());
        samples.sort_unstable();
        samples
    }
}

/// Percentiles of one set of samples.
#[derive(Debug, Default, Clone, Copy)]
pub struct Percentiles {
    pub count: usize,
    pub p50: u64,
    pub p95: u64,
}

impl Percentiles {
    /// Summarizes sorted samples.
    fn of(sorted: &[u64]) -> Self {
        let at = |q: f64| {
            sorted
                .get(((sorted.len() as f64 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(0)
        };
        Self {
            count: sorted.len(),
            p50: at(0.5),
            p95: at(0.95),
        }
    }

    /// Formats as `p50/p95`.
    pub fn p50_p95(&self) -> String {
        if self.count == 0 {
            return "-".to_string();
        }
        format!("{}/{}", self.p50, self.p95)
    }
}

/// Monotonic counter.
#[derive(Default)]
pub struct Counter(AtomicU64);

impl Counter {
    /// Adds one.
    pub fn inc(&self) {
        self.add(1);
    }

    /// Adds `n`.
    pub fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    /// Returns the total.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Live gauge.
#[derive(Default)]
pub struct Gauge(AtomicU64);

impl Gauge {
    /// Adds one.
    pub fn up(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Removes one.
    pub fn down(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    /// Returns the value.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Kinds of request a user waits on, each held to the response-time target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// Websocket ping answered by a pong.
    Ping,
    /// The homepage, `GET /`.
    Homepage,
    /// The mode list with player counts, `GET /api/modes`.
    Modes,
    /// Opening a websocket until the first board arrives.
    Connect,
    /// Joining a shared world until the player's board arrives.
    Join,
    /// Asking for the spectatable matches until the list arrives.
    MatchList,
    /// Asking to spectate until the spectated board arrives.
    Spectate,
    /// Downloading a replay.
    Replay,
}

impl RequestKind {
    pub const ALL: [Self; 8] = [
        Self::Ping,
        Self::Homepage,
        Self::Modes,
        Self::Connect,
        Self::Join,
        Self::MatchList,
        Self::Spectate,
        Self::Replay,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::Homepage => "home",
            Self::Modes => "modes",
            Self::Connect => "conn",
            Self::Join => "join",
            Self::MatchList => "list",
            Self::Spectate => "spec",
            Self::Replay => "replay",
        }
    }
}

/// Response times of every request kind.
#[derive(Default)]
pub struct Requests([Samples; 8]);

impl Requests {
    /// Records how long one request took.
    pub fn record(&self, kind: RequestKind, ms: u64) {
        self.0[kind as usize].record(ms);
    }

    /// Takes the samples of every kind recorded since the last call.
    pub fn take(&self) -> Vec<(RequestKind, Vec<u64>)> {
        RequestKind::ALL
            .iter()
            .map(|kind| (*kind, self.0[*kind as usize].take()))
            .collect()
    }
}

/// Everything the bots report.
#[derive(Default)]
pub struct Metrics {
    pub connected: Gauge,
    pub playing: Gauge,
    pub spectating: Gauge,
    pub queued: Gauge,
    pub updates: Counter,
    pub bytes: Counter,
    pub actions_sent: Counter,
    pub actions_failed: Counter,
    pub matches_finished: Counter,
    pub replays_announced: Counter,
    pub replays_fetched: Counter,
    pub replay_bytes: Counter,
    pub replay_failures: Counter,
    pub spectate_sessions: Counter,
    pub disconnects: Counter,
    pub connect_failures: Counter,
    pub http_failures: Counter,
    pub decode_errors: Counter,
    pub server_errors: Counter,
    /// Live boards that went quiet for over a second while the bot sat on them.
    pub update_gaps: Counter,
    pub requests: Requests,
}

/// Counter totals at one moment, to diff stages.
#[derive(Debug, Default, Clone, Copy)]
pub struct Totals {
    pub updates: u64,
    pub bytes: u64,
    pub actions_sent: u64,
    pub actions_failed: u64,
    pub matches_finished: u64,
    pub replays_announced: u64,
    pub replays_fetched: u64,
    pub replay_bytes: u64,
    pub replay_failures: u64,
    pub spectate_sessions: u64,
    pub disconnects: u64,
    pub connect_failures: u64,
    pub http_failures: u64,
    pub decode_errors: u64,
    pub server_errors: u64,
    pub update_gaps: u64,
}

/// Latency summaries of one stage.
#[derive(Debug, Default, Clone)]
pub struct Latencies {
    /// Percentiles of each request kind.
    pub requests: Vec<(RequestKind, Percentiles)>,
    /// Share of all requests answered within the target time.
    pub within_target: f64,
}

impl Metrics {
    /// Returns the counter totals.
    pub fn totals(&self) -> Totals {
        Totals {
            updates: self.updates.get(),
            bytes: self.bytes.get(),
            actions_sent: self.actions_sent.get(),
            actions_failed: self.actions_failed.get(),
            matches_finished: self.matches_finished.get(),
            replays_announced: self.replays_announced.get(),
            replays_fetched: self.replays_fetched.get(),
            replay_bytes: self.replay_bytes.get(),
            replay_failures: self.replay_failures.get(),
            spectate_sessions: self.spectate_sessions.get(),
            disconnects: self.disconnects.get(),
            connect_failures: self.connect_failures.get(),
            http_failures: self.http_failures.get(),
            decode_errors: self.decode_errors.get(),
            server_errors: self.server_errors.get(),
            update_gaps: self.update_gaps.get(),
        }
    }

    /// Takes the latency samples recorded since the last call, judging requests against
    /// `target_ms`.
    pub fn take_latencies(&self, target_ms: u64) -> Latencies {
        let requests = self.requests.take();
        let total: usize = requests.iter().map(|(_, samples)| samples.len()).sum();
        let fast: usize = requests
            .iter()
            .map(|(_, samples)| samples.iter().filter(|ms| **ms <= target_ms).count())
            .sum();
        Latencies {
            requests: requests
                .iter()
                .map(|(kind, samples)| (*kind, Percentiles::of(samples)))
                .collect(),
            within_target: if total == 0 { 1.0 } else { fast as f64 / total as f64 },
        }
    }
}

impl Totals {
    /// Returns the increase from `before` to `self`.
    pub fn since(&self, before: &Totals) -> Totals {
        Totals {
            updates: self.updates - before.updates,
            bytes: self.bytes - before.bytes,
            actions_sent: self.actions_sent - before.actions_sent,
            actions_failed: self.actions_failed - before.actions_failed,
            matches_finished: self.matches_finished - before.matches_finished,
            replays_announced: self.replays_announced - before.replays_announced,
            replays_fetched: self.replays_fetched - before.replays_fetched,
            replay_bytes: self.replay_bytes - before.replay_bytes,
            replay_failures: self.replay_failures - before.replay_failures,
            spectate_sessions: self.spectate_sessions - before.spectate_sessions,
            disconnects: self.disconnects - before.disconnects,
            connect_failures: self.connect_failures - before.connect_failures,
            http_failures: self.http_failures - before.http_failures,
            decode_errors: self.decode_errors - before.decode_errors,
            server_errors: self.server_errors - before.server_errors,
            update_gaps: self.update_gaps - before.update_gaps,
        }
    }
}
