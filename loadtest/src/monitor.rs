//! Resource usage of the server process and the machine, read from `/proc`.

use std::time::Instant;

/// One reading of the server's resources.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sample {
    pub rss_mb: u64,
    /// Memory of the process that was moved to swap.
    pub swap_mb: u64,
    pub threads: u64,
    /// Memory the machine can still hand out without swapping.
    pub available_mb: u64,
    /// CPU time the process used so far, in clock ticks.
    cpu_ticks: u64,
}

/// Watches one server process.
pub struct ServerProcess {
    pid: u32,
    last: Option<(Instant, Sample)>,
    previous: Option<(Instant, Sample)>,
}

/// Clock ticks per second of `/proc/<pid>/stat` times on Linux.
const TICKS_PER_SEC: f64 = 100.0;

impl ServerProcess {
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            last: None,
            previous: None,
        }
    }

    /// Reads the current resources, or `None` once the process is gone.
    pub fn sample(&mut self) -> Option<Sample> {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.pid)).ok()?;
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", self.pid)).ok()?;
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb = |text: &str, key: &str| {
            text.lines()
                .find(|line| line.starts_with(key))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
        };
        // Fields after the parenthesized command name; utime and stime are the 14th and 15th.
        let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
        let tick = |index: usize| {
            fields
                .get(index)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let sample = Sample {
            rss_mb: kb(&status, "VmRSS:") / 1024,
            swap_mb: kb(&status, "VmSwap:") / 1024,
            threads: kb(&status, "Threads:"),
            available_mb: kb(&meminfo, "MemAvailable:") / 1024,
            cpu_ticks: tick(11) + tick(12),
        };
        self.previous = self.last.take().or(self.previous);
        self.last = Some((Instant::now(), sample));
        Some(sample)
    }

    /// Returns the latest reading.
    pub fn last(&self) -> Option<Sample> {
        self.last.map(|(_, sample)| sample)
    }

    /// Returns the CPU use between the last two readings, in percent of one core.
    pub fn cpu_percent(&self) -> f64 {
        match (self.previous, self.last) {
            (Some((t0, s0)), Some((t1, s1))) => {
                let secs = (t1 - t0).as_secs_f64().max(0.001);
                (s1.cpu_ticks.saturating_sub(s0.cpu_ticks)) as f64 / TICKS_PER_SEC / secs * 100.0
            }
            _ => 0.0,
        }
    }
}
