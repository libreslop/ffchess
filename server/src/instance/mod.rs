//! Live game instances.
//!
//! All mutable state of an instance lives in one [`World`] behind one mutex. Every public
//! operation locks it once and never awaits while holding it, so operations are atomic and
//! messages leave in the same order as the state changes they describe.

mod actions;
mod chat_history;
mod hooks;
mod npcs;
mod outbox;
mod players;
mod sessions;
mod setup;
mod shops;
mod ticks;
mod world;

pub use players::JoinRequest;

use crate::time::now_ms;
use crate::types::{ClientTx, ConnectionId};
use common::models::{GameModeConfig, GameState};
use common::protocol::{ActionRequest, ChatLine, GameError, ServerMessage};
use common::rules::Catalog;
use common::types::{
    ColorHex, DurationMs, ModeId, PieceId, PlayerCount, PlayerId, SessionSecret, TimestampMs,
};
use players::Departure;
use setup::InstanceSetup;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use world::World;

/// One running board: the public world of a mode or a private queue match.
pub struct GameInstance {
    setup: Arc<InstanceSetup>,
    world: Mutex<World>,
}

impl GameInstance {
    /// Creates the shared public instance of `mode`.
    pub fn public(mode: GameModeConfig, catalog: Arc<Catalog>, chat_ttl: DurationMs) -> Self {
        Self::with_setup(InstanceSetup::public(mode, catalog), chat_ttl)
    }

    /// Creates a private match of the queue mode `template`.
    pub fn private_match(
        template: &GameModeConfig,
        catalog: Arc<Catalog>,
        chat_ttl: DurationMs,
    ) -> Self {
        Self::with_setup(InstanceSetup::private_match(template, catalog), chat_ttl)
    }

    /// Creates an instance from its setup.
    fn with_setup(setup: InstanceSetup, chat_ttl: DurationMs) -> Self {
        let setup = Arc::new(setup);
        Self {
            world: Mutex::new(World::new(setup.clone(), chat_ttl, now_ms())),
            setup,
        }
    }

    /// Locks the world. A panic while locked cannot leave it half-updated in a way that
    /// matters more than stopping the whole server would, so poisoning is ignored.
    fn world(&self) -> MutexGuard<'_, World> {
        self.world.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the internal id of this instance.
    pub fn mode_id(&self) -> &ModeId {
        &self.setup.mode.id
    }

    /// Returns the id of the public mode this instance belongs to.
    pub fn public_mode_id(&self) -> &ModeId {
        &self.setup.public_mode_id
    }

    /// Returns true for private queue matches.
    pub fn is_private(&self) -> bool {
        self.setup.is_private
    }

    /// Returns the display name of the mode.
    pub fn display_name(&self) -> &str {
        &self.setup.mode.display_name
    }

    /// Returns the configured player limit.
    pub fn max_players(&self) -> PlayerCount {
        self.setup.mode.max_players
    }

    /// Returns the configured respawn cooldown.
    pub fn respawn_cooldown(&self) -> DurationMs {
        self.setup.mode.respawn_cooldown_ms
    }

    /// Returns the number of players still competing.
    pub fn player_count(&self) -> PlayerCount {
        PlayerCount::new(self.world().active_player_count() as u32)
    }

    /// Returns when the instance last went from empty to having players.
    pub fn last_started_at(&self) -> TimestampMs {
        self.world().clock.last_started_at
    }

    /// Returns true when no player, player channel, or spectator remains.
    pub fn is_abandoned(&self) -> bool {
        let world = self.world();
        world.state.players.is_empty() && world.outbox.is_empty()
    }

    /// Places the configured shops; call once after creation.
    pub fn spawn_initial_shops(&self) {
        self.world().spawn_initial_shops();
    }

    /// Starts the pre-match countdown of a private match, announcing each second.
    pub fn start_countdown(self: &Arc<Self>) {
        let countdown = self.setup.mode.queue_countdown_ms;
        let seconds = (countdown.as_u64() / 1000) as u32;
        {
            let mut world = self.world();
            if countdown > DurationMs::zero() {
                world.clock.move_unlock_at = Some(now_ms() + countdown);
            }
            if seconds == 0 {
                world.record_game_start();
                return;
            }
        }

        let instance = Arc::clone(self);
        tokio::spawn(async move {
            for remaining in (1..=seconds).rev() {
                instance.world().record_countdown(remaining);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            instance.world().record_game_start();
        });
    }

    /// Builds the snapshot a client needs to start viewing this instance.
    pub fn init_message(
        &self,
        player_id: PlayerId,
        secret: SessionSecret,
        sync_interval_ms: u32,
    ) -> ServerMessage {
        self.world()
            .init_message(player_id, secret, sync_interval_ms, now_ms())
    }

    /// Starts sending updates to a spectator connection.
    pub fn add_viewer(&self, conn_id: ConnectionId, tx: ClientTx) {
        self.world().outbox.add_viewer(conn_id, tx);
    }

    /// Stops sending updates to a spectator connection.
    pub fn remove_viewer(&self, conn_id: ConnectionId) {
        self.world().outbox.remove_viewer(conn_id);
    }

    /// Returns true when `conn_id` watches this instance as a spectator.
    pub fn has_viewer(&self, conn_id: ConnectionId) -> bool {
        self.world().outbox.has_viewer(conn_id)
    }

    /// Adds a player to the match.
    pub fn join(&self, request: JoinRequest) -> Result<(PlayerId, SessionSecret), GameError> {
        self.world().join(request, now_ms())
    }

    /// Returns true when `player_id` is playing here with a live connection.
    pub fn has_active_session(&self, player_id: PlayerId) -> bool {
        let world = self.world();
        world.outbox.has_player(player_id) && world.state.players.contains_key(&player_id)
    }

    /// Returns true while `player_id` is on the board and has not won yet.
    pub fn is_competing(&self, player_id: PlayerId) -> bool {
        let world = self.world();
        world.state.players.contains_key(&player_id) && !world.victors.contains(&player_id)
    }

    /// Removes a player who left or whose connection dropped, sending them their final stats.
    pub fn leave(&self, player_id: PlayerId) {
        self.world()
            .remove_player(player_id, Departure::Left, now_ms());
    }

    /// Removes a player who is about to join somewhere else.
    pub fn detach(&self, player_id: PlayerId) {
        self.world()
            .remove_player(player_id, Departure::Detached, now_ms());
    }

    /// Returns the display name and color of a player in this match.
    pub fn player_identity(&self, player_id: PlayerId) -> Option<(String, ColorHex)> {
        self.world()
            .state
            .players
            .get(&player_id)
            .map(|player| (player.name.clone(), player.color.clone()))
    }

    /// Queues or runs a piece action requested by `player_id`.
    pub fn submit_action(&self, player_id: PlayerId, request: ActionRequest) {
        self.world().submit_action(player_id, request, now_ms());
    }

    /// Cancels every queued action of `player_id` on `piece_id`.
    pub fn clear_actions(&self, player_id: PlayerId, piece_id: PieceId) {
        self.world().clear_actions(player_id, piece_id);
    }

    /// Adds a chat line to the room and sends it to everyone in it.
    pub fn post_chat(&self, line: ChatLine) {
        self.world().post_chat(line, now_ms());
    }

    /// Advances the instance by one tick.
    pub fn tick(&self) {
        self.world().tick(now_ms());
    }

    /// Runs `f` with direct access to the board state.
    ///
    /// Intended for tests and diagnostics; gameplay goes through the methods above.
    pub fn with_state<R>(&self, f: impl FnOnce(&mut GameState) -> R) -> R {
        f(&mut self.world().state)
    }
}
