//! Headless clients that behave like real players, spectators, and visitors.

use crate::http::http_get;
use crate::metrics::{Metrics, RequestKind};
use common::models::{GameState, MoveTiming, Piece, PieceAction, Player, Shop};
use common::protocol::{ActionRequest, ClientMessage, ServerMessage, SpectateTarget};
use common::rules::{Catalog, MoveKind};
use common::types::{ActionId, BoardSize, KitId, ModeId, PlayerId, ReplayId};
use futures_util::{SinkExt, StreamExt};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep_until};
use tokio_tungstenite::tungstenite::Message;

/// A request still unanswered after this long counts as failed, with this as its time.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// What a bot does on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Plays the shared world of a public mode, respawning after deaths.
    WorldPlayer(&'static str),
    /// Watches the shared world of a public mode as a named spectator.
    WorldSpectator(&'static str),
    /// Queues for matches of a queue mode, plays them out, watches some replays, and requeues.
    MatchPlayer(&'static str),
    /// Spectates running matches of a queue mode one after another.
    MatchSpectator(&'static str),
    /// Keeps the list of spectatable matches open, now and then watching one for a while.
    MatchBrowser(&'static str),
    /// Opens the homepage (page, mode list, menu board), looks at it a while, and leaves.
    Visitor(&'static str),
}

impl Role {
    fn mode(self) -> &'static str {
        match self {
            Self::WorldPlayer(mode)
            | Self::WorldSpectator(mode)
            | Self::MatchPlayer(mode)
            | Self::MatchSpectator(mode)
            | Self::MatchBrowser(mode)
            | Self::Visitor(mode) => mode,
        }
    }
}

/// Where the bots connect.
#[derive(Debug, Clone)]
pub struct Target {
    /// `host:port` of the server.
    pub addr: String,
}

/// How a bot plays.
#[derive(Debug, Clone, Copy)]
pub struct Tempo {
    /// Bots act this often, plus up to the same again at random.
    pub action_every: Duration,
    /// Match players resign after this long, plus up to the same again at random.
    pub resign_after: Duration,
    /// Share of finished matches whose replay the bot downloads.
    pub replay_watch_chance: f64,
}

/// The fields of an `UpdateState` message a bot reads.
#[derive(serde::Deserialize)]
struct BoardUpdate {
    players: Vec<Player>,
    pieces: Vec<Piece>,
    shops: Vec<Shop>,
    board_size: BoardSize,
}

/// Board snapshot that is only decoded when the bot needs it.
struct LazyBoard(Option<String>);

impl LazyBoard {
    /// Decodes the latest snapshot into a board.
    fn decode(&self, mode_id: &ModeId) -> Option<GameState> {
        let text = self.0.as_ref()?;
        let mut value: Value = serde_json::from_str(text).ok()?;
        let update: BoardUpdate =
            serde_json::from_value(value.get_mut("UpdateState")?.take()).ok()?;
        Some(GameState {
            players: update.players.into_iter().map(|p| (p.id, p)).collect(),
            pieces: update.pieces.into_iter().map(|p| (p.id, p)).collect(),
            shops: update.shops,
            board_size: update.board_size,
            mode_id: mode_id.clone(),
        })
    }
}

/// Phase of one bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Queued,
    Playing,
    /// Finished playing or watching; waiting a moment before going on.
    Done,
    Spectating,
}

/// What the connection loop does after the bot woke up.
enum Next {
    Stay,
    Disconnect,
}

/// Why a connection ended.
enum Ended {
    /// The load test is over.
    Stopped,
    /// The bot closed the page on purpose.
    Left,
    /// The server dropped the connection.
    Dropped,
}

/// One bot's view of its session.
struct Session {
    role: Role,
    mode_id: ModeId,
    name: String,
    player_id: Option<PlayerId>,
    catalog: Option<Arc<Catalog>>,
    phase: Phase,
    /// Seen on the board as a player since joining.
    seen_on_board: bool,
    board: LazyBoard,
    next_action: ActionId,
    pending_actions: HashMap<ActionId, Instant>,
    /// Pings sent and not answered yet, by their client time.
    pending_pings: VecDeque<(u64, Instant)>,
    /// The one request the bot waits on, if any.
    waiting: Option<(RequestKind, Instant)>,
    /// When the bot next does something on its own.
    wake_at: Instant,
    /// When the bot stops what it is doing: resigning, leaving a board, or closing the page.
    leave_at: Option<Instant>,
    last_update: Option<Instant>,
    replay: Option<ReplayId>,
    rng: SmallRng,
}

/// Messages a bot wants to send.
type Outbox = Vec<ClientMessage>;

impl Session {
    fn new(role: Role, index: usize, connected_at: Instant) -> Self {
        let mut rng = SmallRng::from_entropy();
        let leave_at = matches!(role, Role::Visitor(_))
            .then(|| connected_at + Duration::from_secs(rng.gen_range(10..30)));
        Self {
            role,
            mode_id: ModeId::from(role.mode()),
            name: format!("bot{index}"),
            player_id: None,
            catalog: None,
            phase: Phase::Idle,
            seen_on_board: false,
            board: LazyBoard(None),
            next_action: ActionId(1),
            pending_actions: HashMap::new(),
            pending_pings: VecDeque::new(),
            // Opening the page waits for its first board.
            waiting: Some((RequestKind::Connect, connected_at)),
            wake_at: connected_at,
            leave_at,
            last_update: None,
            replay: None,
            rng,
        }
    }

    fn kit(&self) -> KitId {
        KitId::from(match self.role.mode() {
            "blitz" | "bullet" => "Classic",
            _ => "Standard",
        })
    }

    fn set_phase(&mut self, phase: Phase, metrics: &Metrics) {
        let gauge = |phase| match phase {
            Phase::Queued => Some(&metrics.queued),
            Phase::Playing => Some(&metrics.playing),
            Phase::Spectating => Some(&metrics.spectating),
            Phase::Idle | Phase::Done => None,
        };
        if let Some(gauge) = gauge(self.phase) {
            gauge.down();
        }
        if let Some(gauge) = gauge(phase) {
            gauge.up();
        }
        self.phase = phase;
        // Board changes come with a pause between updates, which is no gap.
        self.last_update = None;
    }

    fn sleep_for(&mut self, base: Duration) {
        let jitter = self.rng.gen_range(0.0..1.0);
        self.wake_at = Instant::now() + base.mul_f64(1.0 + jitter);
    }

    /// Starts waiting on a request.
    fn request(&mut self, kind: RequestKind) {
        self.waiting = Some((kind, Instant::now()));
    }

    /// Records the answer to the awaited request, if it is of `kind`.
    fn answered(&mut self, kind: RequestKind, metrics: &Metrics) {
        if let Some((waiting, sent)) = self.waiting
            && waiting == kind
        {
            metrics
                .requests
                .record(kind, sent.elapsed().as_millis() as u64);
            self.waiting = None;
        }
    }

    /// Counts requests that went unanswered for too long as failed.
    fn expire_requests(&mut self, metrics: &Metrics) {
        let timeout_ms = REQUEST_TIMEOUT.as_millis() as u64;
        if let Some((kind, sent)) = self.waiting
            && sent.elapsed() > REQUEST_TIMEOUT
        {
            metrics.requests.record(kind, timeout_ms);
            self.waiting = None;
        }
        while let Some((_, sent)) = self.pending_pings.front()
            && sent.elapsed() > REQUEST_TIMEOUT
        {
            metrics.requests.record(RequestKind::Ping, timeout_ms);
            self.pending_pings.pop_front();
        }
    }

    fn join(&mut self, out: &mut Outbox, metrics: &Metrics) {
        out.push(ClientMessage::Join {
            name: self.name.clone(),
            kit_name: self.kit(),
            player_id: None,
            session_secret: None,
        });
        if matches!(self.role, Role::WorldPlayer(_)) {
            self.request(RequestKind::Join);
        }
        self.seen_on_board = false;
        self.set_phase(Phase::Queued, metrics);
    }

    fn spectate(&mut self, target: SpectateTarget, out: &mut Outbox, metrics: &Metrics) {
        out.push(ClientMessage::Spectate {
            name: self.name.clone(),
            target,
            player_id: None,
        });
        self.request(RequestKind::Spectate);
        self.set_phase(Phase::Spectating, metrics);
        let watch_secs = match self.role {
            Role::WorldSpectator(_) => self.rng.gen_range(60..120),
            Role::MatchBrowser(_) => self.rng.gen_range(10..20),
            _ => return,
        };
        self.leave_at = Some(Instant::now() + Duration::from_secs(watch_secs));
    }

    /// Reacts to one decoded message.
    fn on_message(
        &mut self,
        message: ServerMessage,
        tempo: &Tempo,
        metrics: &Metrics,
        out: &mut Outbox,
    ) {
        match message {
            ServerMessage::Init {
                player_id, catalog, ..
            } => {
                self.catalog = Some(Arc::new(*catalog));
                self.answered(RequestKind::Connect, metrics);
                if !player_id.is_nil() {
                    self.answered(RequestKind::Join, metrics);
                    self.player_id = Some(player_id);
                    self.set_phase(Phase::Playing, metrics);
                    self.sleep_for(tempo.action_every);
                    self.leave_at = matches!(self.role, Role::MatchPlayer(_)).then(|| {
                        let factor = 1.0 + self.rng.gen_range(0.0..1.0);
                        Instant::now() + tempo.resign_after.mul_f64(factor)
                    });
                } else if self.phase == Phase::Spectating {
                    self.answered(RequestKind::Spectate, metrics);
                    metrics.spectate_sessions.inc();
                }
                self.last_update = None;
            }
            ServerMessage::ActionResult { id, result, .. } => {
                self.pending_actions.remove(&id);
                if result.is_err() {
                    metrics.actions_failed.inc();
                }
            }
            ServerMessage::Pong(sent_ms, _) => {
                let answered = self.pending_pings.iter().position(|(ms, _)| *ms == sent_ms);
                if let Some((_, sent)) = answered.and_then(|i| self.pending_pings.remove(i)) {
                    let ms = sent.elapsed().as_millis() as u64;
                    metrics.requests.record(RequestKind::Ping, ms);
                }
            }
            ServerMessage::Victory { .. } => self.finish_playing(metrics),
            ServerMessage::GameOver(_) => {
                if self.phase == Phase::Playing {
                    self.finish_playing(metrics);
                }
            }
            ServerMessage::MatchFinished { replay } => {
                metrics.matches_finished.inc();
                if replay.is_some() {
                    metrics.replays_announced.inc();
                }
                self.replay = replay.filter(|_| self.rng.gen_bool(tempo.replay_watch_chance));
            }
            ServerMessage::SpectateEnded => {
                self.answered(RequestKind::Spectate, metrics);
                if self.phase == Phase::Spectating {
                    self.set_phase(Phase::Done, metrics);
                    self.leave_at = None;
                    self.sleep_for(Duration::from_secs(1));
                }
            }
            ServerMessage::MatchList { matches } => {
                self.answered(RequestKind::MatchList, metrics);
                let live: Vec<_> = matches
                    .into_iter()
                    .filter(|m| !m.players.is_empty())
                    .collect();
                // Browsers mostly look at the list; spectators always pick a match.
                let picks = match self.role {
                    Role::MatchBrowser(_) => self.rng.gen_bool(0.25),
                    _ => true,
                };
                match live.choose(&mut self.rng).filter(|_| picks) {
                    Some(target) => {
                        let target = SpectateTarget::Match(target.id.clone());
                        self.spectate(target, out, metrics);
                    }
                    None => self.sleep_for(Duration::from_secs(3)),
                }
            }
            ServerMessage::Error(_) => {
                metrics.server_errors.inc();
                self.waiting = None;
                // Full worlds and cooldowns: try again later.
                if matches!(self.phase, Phase::Queued | Phase::Spectating) {
                    self.set_phase(Phase::Idle, metrics);
                }
                self.sleep_for(Duration::from_secs(5));
            }
            ServerMessage::QueueState { .. }
            | ServerMessage::Chat { .. }
            | ServerMessage::SetCamera(_) => {}
            ServerMessage::UpdateState { .. } => unreachable!("decoded lazily"),
        }
    }

    /// The bot's own match or life is over.
    fn finish_playing(&mut self, metrics: &Metrics) {
        self.set_phase(Phase::Done, metrics);
        self.player_id = None;
        self.leave_at = None;
        self.pending_actions.clear();
        // Waits for MatchFinished and lets the end screen sit a moment, like a person would.
        self.sleep_for(Duration::from_secs(2));
    }

    /// Records a board snapshot.
    fn on_update(&mut self, text: String, metrics: &Metrics) {
        let now = Instant::now();
        if let Some(last) = self.last_update
            && now - last > Duration::from_secs(1)
        {
            metrics.update_gaps.inc();
        }
        self.last_update = Some(now);
        metrics.updates.inc();
        self.board = LazyBoard(Some(text));
    }

    /// Does whatever the bot does on its own once it wakes.
    async fn on_wake(
        &mut self,
        target: &Target,
        tempo: &Tempo,
        metrics: &Metrics,
        out: &mut Outbox,
    ) -> Next {
        // Every branch waits at least this long before waking again, unless it says otherwise.
        self.sleep_for(Duration::from_secs(1));
        self.expire_requests(metrics);
        let leaving = self.leave_at.is_some_and(|at| Instant::now() >= at);
        match (self.role, self.phase) {
            (Role::Visitor(_), _) if leaving => return Next::Disconnect,
            (Role::Visitor(_), _) => {}
            (Role::WorldPlayer(_) | Role::MatchPlayer(_), Phase::Idle) => self.join(out, metrics),
            (Role::WorldSpectator(_), Phase::Idle) => {
                self.spectate(SpectateTarget::Own, out, metrics);
            }
            (Role::MatchSpectator(_) | Role::MatchBrowser(_), Phase::Idle) => {
                if self.waiting.is_none() {
                    out.push(ClientMessage::ListMatches);
                    self.request(RequestKind::MatchList);
                }
            }
            (_, Phase::Playing) if leaving => {
                out.push(ClientMessage::Leave);
                self.finish_playing(metrics);
            }
            (_, Phase::Playing) => {
                self.act(out, metrics);
                self.sleep_for(tempo.action_every);
            }
            (_, Phase::Spectating) if leaving => {
                out.push(ClientMessage::Leave);
                self.leave_at = None;
                self.set_phase(Phase::Done, metrics);
            }
            (_, Phase::Done) => {
                if let Some(replay) = self.replay.take() {
                    watch_replay(target, replay, metrics).await;
                }
                out.push(ClientMessage::Leave);
                self.set_phase(Phase::Idle, metrics);
            }
            (_, Phase::Queued | Phase::Spectating) => {}
        }
        Next::Stay
    }

    /// Checks whether the bot's player is still on the board, and makes one move.
    fn act(&mut self, out: &mut Outbox, metrics: &Metrics) {
        let (Some(player_id), Some(catalog)) = (self.player_id, self.catalog.clone()) else {
            return;
        };
        let Some(state) = self.board.decode(&self.mode_id) else {
            return;
        };
        if !state.players.contains_key(&player_id) {
            if self.seen_on_board {
                // Eliminated: the server sends no message for it, the board shows it.
                self.player_id = None;
                self.set_phase(Phase::Done, metrics);
                self.sleep_for(Duration::from_secs(2));
            }
            return;
        }
        self.seen_on_board = true;
        let mine: Vec<&Piece> = state.pieces_of(player_id).collect();
        let Some(piece) = mine.choose(&mut self.rng) else {
            return;
        };
        let moves = catalog.legal_moves(&state, piece, MoveTiming::Now);
        // Captures first, so matches end like real ones do.
        let captures: Vec<_> = moves
            .iter()
            .filter(|m| m.kind == MoveKind::Capture)
            .collect();
        let chosen = captures
            .choose(&mut self.rng)
            .copied()
            .or_else(|| moves.choose(&mut self.rng));
        let Some(chosen) = chosen else {
            return;
        };
        let id = self.next_action;
        self.next_action = id.next();
        self.pending_actions.insert(id, Instant::now());
        metrics.actions_sent.inc();
        out.push(ClientMessage::QueueAction(ActionRequest {
            id,
            piece_id: piece.id,
            action: PieceAction::Move {
                target: chosen.target,
            },
            after: None,
        }));
    }
}

/// Fetches one page, recording how long it took as a request of `kind`.
async fn timed_get(
    target: &Target,
    path: &str,
    kind: RequestKind,
    metrics: &Metrics,
) -> Option<Vec<u8>> {
    let started = Instant::now();
    let response = tokio::time::timeout(REQUEST_TIMEOUT, http_get(&target.addr, path)).await;
    let elapsed = started.elapsed().as_millis() as u64;
    match response {
        Ok(Ok((200, body))) => {
            metrics.requests.record(kind, elapsed);
            Some(body)
        }
        Ok(_) => {
            metrics.requests.record(kind, elapsed);
            metrics.http_failures.inc();
            None
        }
        Err(_) => {
            metrics
                .requests
                .record(kind, REQUEST_TIMEOUT.as_millis() as u64);
            metrics.http_failures.inc();
            None
        }
    }
}

/// Downloads a replay like the replay viewer does.
async fn watch_replay(target: &Target, replay: ReplayId, metrics: &Metrics) {
    let path = format!("/api/replays/{replay}");
    let Some(body) = timed_get(target, &path, RequestKind::Replay, metrics).await else {
        return metrics.replay_failures.inc();
    };
    metrics.replays_fetched.inc();
    metrics.replay_bytes.add(body.len() as u64);
    if serde_json::from_slice::<common::models::Replay>(&body).is_err() {
        metrics.decode_errors.inc();
    }
}

/// Returns wall-clock milliseconds since the epoch.
fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Runs one bot until `stop` resolves, reconnecting after disconnects.
pub async fn run_bot(
    index: usize,
    role: Role,
    target: Target,
    tempo: Tempo,
    metrics: Arc<Metrics>,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let url = format!("ws://{}/api/ws/{}", target.addr, role.mode());
    let mut rng = SmallRng::from_entropy();
    while !*stop.borrow() {
        if matches!(role, Role::Visitor(_)) {
            // The page and the mode list load before the page opens its websocket.
            timed_get(&target, "/", RequestKind::Homepage, &metrics).await;
            timed_get(&target, "/api/modes", RequestKind::Modes, &metrics).await;
        }
        let connecting = Instant::now();
        let connected =
            tokio::time::timeout(REQUEST_TIMEOUT, tokio_tungstenite::connect_async(&url)).await;
        let Ok(Ok((socket, _))) = connected else {
            metrics.connect_failures.inc();
            let ms = connecting.elapsed().as_millis() as u64;
            metrics.requests.record(RequestKind::Connect, ms);
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };
        metrics.connected.up();
        let (mut write, mut read) = socket.split();
        let mut session = Session::new(role, index, connecting);
        session.sleep_for(Duration::from_millis(500));
        let mut ping = interval(Duration::from_secs(2));
        let ended = loop {
            let mut out = Outbox::new();
            tokio::select! {
                _ = stop.changed() => break Ended::Stopped,
                _ = ping.tick() => {
                    let sent = epoch_ms();
                    session.pending_pings.push_back((sent, Instant::now()));
                    out.push(ClientMessage::Ping(sent));
                }
                _ = sleep_until(session.wake_at.into()) => {
                    let next = session.on_wake(&target, &tempo, &metrics, &mut out).await;
                    if let Next::Disconnect = next {
                        break Ended::Left;
                    }
                }
                frame = read.next() => match frame {
                    Some(Ok(Message::Text(text))) => {
                        metrics.bytes.add(text.len() as u64);
                        if text.starts_with("{\"UpdateState\"") {
                            session.on_update(text, &metrics);
                        } else {
                            match serde_json::from_str::<ServerMessage>(&text) {
                                Ok(message) => {
                                    session.on_message(message, &tempo, &metrics, &mut out);
                                }
                                Err(_) => metrics.decode_errors.inc(),
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break Ended::Dropped,
                },
            }
            let mut failed = false;
            for message in out {
                let text = serde_json::to_string(&message).expect("encode");
                if write.send(Message::Text(text)).await.is_err() {
                    failed = true;
                    break;
                }
            }
            if failed {
                break Ended::Dropped;
            }
        };
        session.set_phase(Phase::Idle, &metrics);
        metrics.connected.down();
        match ended {
            Ended::Stopped => {
                let _ = write.close().await;
                return;
            }
            Ended::Left => {
                let _ = write.close().await;
                tokio::time::sleep(Duration::from_secs(rng.gen_range(2..6))).await;
            }
            Ended::Dropped => {
                metrics.disconnects.inc();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
