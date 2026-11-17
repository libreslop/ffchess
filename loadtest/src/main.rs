//! Load tester for the ffchess server: headless bots playing, spectating, and watching
//! replays, ramped up while the server's memory and CPU are watched.

mod bot;
mod http;
mod metrics;
mod monitor;

use bot::{Role, Target, Tempo, run_bot};
use clap::{Parser, Subcommand};
use metrics::Metrics;
use monitor::ServerProcess;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;

#[derive(Parser)]
struct Cli {
    /// `host:port` of the server.
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs a fixed number of bots of one role until interrupted.
    Bots {
        /// One of: ffa, arena, bullet, blitz, spectate-bullet, spectate-blitz.
        role: String,
        #[arg(long, default_value_t = 1)]
        count: usize,
        /// Seconds before a match player resigns, doubled at most at random.
        #[arg(long, default_value_t = 60)]
        resign_secs: u64,
    },
    /// Adds bots in steps, reporting server resources per step, until a limit is reached.
    Ramp {
        /// Server process id to watch.
        #[arg(long)]
        pid: u32,
        /// Bots added per step.
        #[arg(long, default_value_t = 100)]
        step: usize,
        /// Seconds each step is held.
        #[arg(long, default_value_t = 30)]
        hold_secs: u64,
        #[arg(long, default_value_t = 10_000)]
        max_bots: usize,
        /// Stops once the server's resident memory exceeds this.
        #[arg(long, default_value_t = 3_500)]
        max_rss_mb: u64,
        /// Stops once the machine has less memory than this available, to stay out of swap.
        #[arg(long, default_value_t = 3_000)]
        min_available_mb: u64,
        /// Requests are on time when answered within this many milliseconds.
        #[arg(long, default_value_t = 1_000)]
        target_ms: u64,
        /// Stops once fewer than this share of requests in a step are on time.
        #[arg(long, default_value_t = 0.95)]
        min_on_time: f64,
    },
}

/// Parses a role name.
fn role(name: &str) -> Role {
    match name {
        "ffa" => Role::WorldPlayer("ffa"),
        "arena" => Role::WorldPlayer("arena"),
        "bullet" => Role::MatchPlayer("bullet"),
        "blitz" => Role::MatchPlayer("blitz"),
        "spectate-bullet" => Role::MatchSpectator("bullet"),
        "spectate-blitz" => Role::MatchSpectator("blitz"),
        "spectate-ffa" => Role::WorldSpectator("ffa"),
        "browse-bullet" => Role::MatchBrowser("bullet"),
        "visit" => Role::Visitor("ffa"),
        other => panic!("unknown role {other}"),
    }
}

/// Role of the `index`th bot of a ramp: the shared worlds fill up first, then users split
/// into match players (40%), match spectators (10%), shared-world spectators (15%), match
/// browsers (10%), and homepage visitors (25%).
fn ramp_role(index: usize) -> Role {
    match index {
        0..20 => Role::WorldPlayer("ffa"),
        20..30 => Role::WorldPlayer("arena"),
        _ => match index % 20 {
            0..6 => Role::MatchPlayer("bullet"),
            6..8 => Role::MatchPlayer("blitz"),
            8 => Role::MatchSpectator("bullet"),
            9 => Role::MatchSpectator("blitz"),
            10 | 11 => Role::WorldSpectator("ffa"),
            12 => Role::WorldSpectator("arena"),
            13 => Role::MatchBrowser("bullet"),
            14 => Role::MatchBrowser("blitz"),
            15 | 16 => Role::Visitor("ffa"),
            17 => Role::Visitor("arena"),
            18 => Role::Visitor("bullet"),
            _ => Role::Visitor("blitz"),
        },
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let target = Target { addr: cli.addr };
    let metrics = Arc::new(Metrics::default());
    let (stop_tx, stop_rx) = watch::channel(false);
    match cli.command {
        Command::Bots {
            role: name,
            count,
            resign_secs,
        } => {
            let tempo = Tempo {
                action_every: Duration::from_millis(700),
                resign_after: Duration::from_secs(resign_secs),
                replay_watch_chance: 1.0,
            };
            for index in 0..count {
                tokio::spawn(run_bot(
                    index,
                    role(&name),
                    target.clone(),
                    tempo,
                    metrics.clone(),
                    stop_rx.clone(),
                ));
            }
            let mut report = tokio::time::interval(Duration::from_secs(5));
            loop {
                report.tick().await;
                let totals = metrics.totals();
                println!(
                    "connected={} playing={} queued={} spectating={} actions={} matches={} replays={} errors={}",
                    metrics.connected.get(),
                    metrics.playing.get(),
                    metrics.queued.get(),
                    metrics.spectating.get(),
                    totals.actions_sent,
                    totals.matches_finished,
                    totals.replays_fetched,
                    totals.server_errors,
                );
            }
        }
        Command::Ramp {
            pid,
            step,
            hold_secs,
            max_bots,
            max_rss_mb,
            min_available_mb,
            target_ms,
            min_on_time,
        } => {
            let tempo = Tempo {
                action_every: Duration::from_millis(700),
                resign_after: Duration::from_secs(45),
                replay_watch_chance: 0.5,
            };
            ramp(RampPlan {
                target,
                tempo,
                server: ServerProcess::new(pid),
                step,
                hold: Duration::from_secs(hold_secs),
                max_bots,
                max_rss_mb,
                min_available_mb,
                target_ms,
                min_on_time,
                metrics,
                stop: stop_rx,
            })
            .await;
            let _ = stop_tx.send(true);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

/// Everything a ramp needs.
struct RampPlan {
    target: Target,
    tempo: Tempo,
    server: ServerProcess,
    step: usize,
    hold: Duration,
    max_bots: usize,
    max_rss_mb: u64,
    min_available_mb: u64,
    target_ms: u64,
    min_on_time: f64,
    metrics: Arc<Metrics>,
    stop: watch::Receiver<bool>,
}

/// Memory milestones reported once each.
const MILESTONES_MB: [u64; 4] = [512, 1024, 2048, 3072];

/// Ramps bots up step by step, printing one report row per step.
async fn ramp(plan: RampPlan) {
    let RampPlan {
        target,
        tempo,
        mut server,
        step,
        hold,
        max_bots,
        max_rss_mb,
        min_available_mb,
        target_ms,
        min_on_time,
        metrics,
        stop,
    } = plan;
    let kinds: String = metrics::RequestKind::ALL
        .iter()
        .map(|kind| format!(" {:>11}", kind.label()))
        .collect();
    println!(
        "{:>6} {:>5} {:>5} {:>5} {:>5} | {:>7} {:>6} {:>4} | {:>6} |{kinds} | {:>7} {:>6} | {:>5} {:>5} | {:>4} {:>4} {:>4} {:>4} {:>5}",
        "bots", "conn", "play", "queue", "spec", "rss_mb", "cpu%", "thr", "ontime",
        "upd/s/c", "MB/s", "match", "rplay", "disc", "cfail", "hfail", "err", "gap",
    );
    println!("(request columns are p50/p95 in ms; ontime is the share answered within {target_ms} ms)");
    let started = Instant::now();
    let mut spawned = 0;
    let mut milestones = MILESTONES_MB.iter().peekable();
    let mut notes: Vec<String> = Vec::new();
    server.sample();
    loop {
        // Bots of this step join spread over the first few seconds, not all at once.
        let target_bots = (spawned + step).min(max_bots);
        let spread = Duration::from_secs(5).min(hold / 2);
        let gap = spread / (target_bots - spawned).max(1) as u32;
        let step_started = Instant::now();
        while spawned < target_bots {
            tokio::spawn(run_bot(
                spawned,
                ramp_role(spawned),
                target.clone(),
                tempo,
                metrics.clone(),
                stop.clone(),
            ));
            spawned += 1;
            tokio::time::sleep(gap).await;
        }
        // Discards latencies of the joining burst so the row shows the held load.
        tokio::time::sleep(Duration::from_secs(3)).await;
        metrics.take_latencies(target_ms);
        let measured_from = Instant::now();
        let base = metrics.totals();
        let mut stop_reason = None;
        while step_started.elapsed() < hold {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let sample = server.sample();
            let Some(sample) = sample else {
                stop_reason = Some("server process exited".to_string());
                break;
            };
            while let Some(&&milestone) = milestones.peek() {
                if sample.rss_mb < milestone {
                    break;
                }
                let note = format!(
                    "milestone: server RSS crossed {milestone} MB at {} bots ({} connected), t={}s",
                    spawned,
                    metrics.connected.get(),
                    started.elapsed().as_secs()
                );
                println!("{note}");
                notes.push(note);
                milestones.next();
            }
            if sample.swap_mb > 0 {
                stop_reason = Some(format!("server has {} MB in swap", sample.swap_mb));
            } else if sample.rss_mb > max_rss_mb {
                stop_reason = Some(format!("server RSS {} MB over the limit", sample.rss_mb));
            } else if sample.available_mb < min_available_mb {
                stop_reason = Some(format!(
                    "machine has only {} MB available",
                    sample.available_mb
                ));
            }
            if stop_reason.is_some() {
                break;
            }
        }
        let secs = measured_from.elapsed().as_secs_f64().max(0.001);
        let delta = metrics.totals().since(&base);
        let latencies = metrics.take_latencies(target_ms);
        let last = server.last().unwrap_or_default();
        let connected = metrics.connected.get();
        let request_columns: String = latencies
            .requests
            .iter()
            .map(|(_, percentiles)| format!(" {:>11}", percentiles.p50_p95()))
            .collect();
        println!(
            "{:>6} {:>5} {:>5} {:>5} {:>5} | {:>7} {:>6.0} {:>4} | {:>5.1}% |{request_columns} | {:>7.1} {:>6.1} | {:>5} {:>5} | {:>4} {:>4} {:>4} {:>4} {:>5}",
            spawned,
            connected,
            metrics.playing.get(),
            metrics.queued.get(),
            metrics.spectating.get(),
            last.rss_mb,
            server.cpu_percent(),
            last.threads,
            latencies.within_target * 100.0,
            delta.updates as f64 / secs / connected.max(1) as f64,
            delta.bytes as f64 / secs / 1e6,
            delta.matches_finished,
            delta.replays_fetched,
            delta.disconnects,
            delta.connect_failures,
            delta.http_failures,
            delta.server_errors,
            delta.update_gaps,
        );
        if stop_reason.is_none() && latencies.within_target < min_on_time {
            stop_reason = Some(format!(
                "only {:.1}% of requests answered within {target_ms} ms",
                latencies.within_target * 100.0
            ));
        }
        if let Some(reason) = stop_reason {
            println!("stopped: {reason}");
            break;
        }
        if spawned >= max_bots {
            println!("stopped: reached {max_bots} bots");
            break;
        }
    }
    for note in notes {
        println!("{note}");
    }
}
