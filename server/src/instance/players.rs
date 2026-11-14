//! Player join and departure handling.

use super::world::World;
use crate::spawning::{find_random_nearby_free_pos, find_spawn_pos};
use crate::outbound::ClientTx;
use common::models::{KitConfig, Piece, Player, QueuePresetPlayerConfig};
use common::protocol::{GameError, GameOverStats, ServerMessage};
use common::types::{
    BoardCoord, KitId, PieceId, PieceTypeId, PlayerId, Score, SessionSecret, TimestampMs,
};
use rand::Rng;

/// NPCs this close to a fresh spawn are removed so new players are not ambushed.
const SPAWN_SAFE_RADIUS: f32 = 15.0;

/// Kit pieces spawn within this many squares of the king.
const KIT_SPREAD_RADIUS: i32 = 2;

/// Random placement attempts per kit piece before using another spawn spot.
const KIT_PLACEMENT_ATTEMPTS: usize = 10;

/// A request to enter the match as a player.
pub struct JoinRequest {
    pub name: String,
    pub kit_name: KitId,
    pub tx: ClientTx,
    /// Previous player id of a returning player.
    pub player_id: Option<PlayerId>,
    /// Secret proving ownership of `player_id`.
    pub session_secret: Option<SessionSecret>,
}

/// Why a player leaves the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Departure {
    /// Left voluntarily or lost the connection; receives final stats and is announced in chat.
    Left,
    /// Left to join again elsewhere; no final stats are sent.
    Detached,
    /// King was captured; the connection stays to show the end screen.
    Eliminated,
}

impl Departure {
    /// Returns true when the player's connection is no longer attached afterwards.
    fn releases_channel(self) -> bool {
        !matches!(self, Self::Eliminated)
    }

    /// Returns true when the departure counts as leaving the match for hooks.
    fn is_leave(self) -> bool {
        !matches!(self, Self::Eliminated)
    }
}

/// Where a joining player's army is placed.
struct Deployment {
    king_id: PieceId,
    board_rotation_deg: i32,
}

impl World {
    /// Adds a player to the match and spawns their army.
    ///
    /// Returns the player id and the session secret for future rejoins.
    pub(super) fn join(
        &mut self,
        request: JoinRequest,
        now: TimestampMs,
    ) -> Result<(PlayerId, SessionSecret), GameError> {
        let setup = self.setup.clone();
        let kit = setup
            .mode
            .kit(&request.kit_name)
            .ok_or_else(|| GameError::Internal("Kit not found".to_string()))?;
        if !kit.has_king() {
            return Err(GameError::Internal("Kit missing king piece".to_string()));
        }
        self.check_capacity()?;

        let (player_id, session_secret) =
            self.sessions
                .authenticate(request.player_id, request.session_secret, now);
        if let Some(wait) =
            self.sessions
                .respawn_wait(player_id, setup.mode.respawn_cooldown_ms, now)
        {
            return Err(GameError::Custom {
                title: "Respawn cooldown".to_string(),
                message: format!("Wait {} seconds", (wait.as_u64() / 1000).max(1)),
            });
        }

        self.grow_board_for(self.state.players.len() + 1);
        let deployment = match &setup.mode.queue_layout {
            Some(layout) => {
                let slot = layout
                    .players
                    .get(self.state.players.len())
                    .ok_or_else(|| GameError::Custom {
                        title: "Match Full".to_string(),
                        message: "No more spawn slots are available in this match.".to_string(),
                    })?;
                self.deploy_layout(player_id, slot)?
            }
            None => self.deploy_kit(player_id, kit),
        };

        let color = self
            .colors
            .get_or_assign_color(player_id, &self.outbox.player_ids(), now);
        let was_empty = self.state.players.is_empty();
        self.record_join(request.name.clone());
        self.state.players.insert(
            player_id,
            Player {
                id: player_id,
                name: request.name,
                score: Score::zero(),
                kills: 0,
                pieces_captured: 0,
                join_time: now,
                king_id: deployment.king_id,
                color,
                board_rotation_deg: deployment.board_rotation_deg,
            },
        );
        self.outbox.add_player(player_id, request.tx);
        self.release_spectators_of(player_id);
        self.actions.forget_player(player_id);
        self.victors.remove(&player_id);
        self.sessions.clear_death(player_id);
        if was_empty {
            self.clock.last_started_at = now;
        }
        Ok((player_id, session_secret))
    }

    /// Removes a player from the match.
    ///
    /// Returns true when the player was still on the board.
    pub(super) fn remove_player(
        &mut self,
        player_id: PlayerId,
        departure: Departure,
        now: TimestampMs,
    ) -> bool {
        let Some(player) = self.state.players.get(&player_id) else {
            if departure.releases_channel() {
                self.outbox.remove_player(player_id);
            }
            return false;
        };

        match departure {
            Departure::Left => {
                let stats = GameOverStats {
                    final_score: player.score,
                    kills: player.kills,
                    pieces_captured: player.pieces_captured,
                    time_survived_secs: (now - player.join_time).as_u64() / 1000,
                };
                self.record_disconnect(player.name.clone());
                self.outbox
                    .send_to_player(player_id, ServerMessage::GameOver(stats));
            }
            Departure::Detached | Departure::Eliminated => {}
        }

        self.state.players.remove(&player_id);
        if departure.releases_channel() {
            self.outbox.remove_player(player_id);
        }
        if departure.is_leave() {
            self.record_leave();
        }
        self.victors.remove(&player_id);
        self.sessions.record_death(player_id, now);
        let removed = self
            .state
            .remove_pieces_where(|piece| piece.owner_id == Some(player_id));
        self.cancel_actions_of(removed);
        true
    }

    /// Rejects joins once the mode's player limit is reached.
    fn check_capacity(&self) -> Result<(), GameError> {
        let max_players = self.setup.mode.max_players.as_u32() as usize;
        if max_players > 0 && self.state.players.len() >= max_players {
            return Err(GameError::Custom {
                title: "Game Full".to_string(),
                message: "This game is full. Try again later.".to_string(),
            });
        }
        Ok(())
    }

    /// Enlarges the board when the mode wants more room for `player_count` players.
    fn grow_board_for(&mut self, player_count: usize) {
        let wanted = self.setup.mode.board_size_for(player_count);
        if wanted > self.state.board_size {
            self.state.board_size = wanted;
        }
    }

    /// Places a queue-mode preset army; nothing is placed unless every square is usable.
    fn deploy_layout(
        &mut self,
        player_id: PlayerId,
        slot: &QueuePresetPlayerConfig,
    ) -> Result<Deployment, GameError> {
        for placement in &slot.pieces {
            let pos = placement.board_coord();
            if !self.state.board_size.contains(pos) {
                return Err(GameError::Internal(format!(
                    "Queue preset position {pos} is outside board size {}",
                    self.state.board_size
                )));
            }
            if self.state.piece_at(pos).is_some() {
                return Err(GameError::Internal(format!(
                    "Queue preset position {pos} is occupied"
                )));
            }
        }
        if !slot
            .pieces
            .iter()
            .any(|placement| placement.piece_id.is_king())
        {
            return Err(GameError::Internal(
                "Queue preset missing king piece".to_string(),
            ));
        }

        let mut king_id = PieceId::nil();
        for placement in &slot.pieces {
            let id = self.place_piece(player_id, &placement.piece_id, placement.board_coord());
            if placement.piece_id.is_king() {
                king_id = id;
            }
        }
        Ok(Deployment {
            king_id,
            board_rotation_deg: slot.board_rotation_deg,
        })
    }

    /// Places a kit army around a fresh spawn point, clearing nearby NPCs first.
    fn deploy_kit(&mut self, player_id: PlayerId, kit: &KitConfig) -> Deployment {
        let mut rng = rand::thread_rng();
        let spawn = find_spawn_pos(&self.state, &mut rng);
        self.state.remove_pieces_where(|piece| {
            !piece.is_player_owned()
                && (piece.position - spawn).as_vec2().length() <= SPAWN_SAFE_RADIUS
        });

        let mut king_id = PieceId::nil();
        for piece_type in &kit.pieces {
            let pos = self.kit_piece_position(spawn, piece_type.is_king(), &mut rng);
            let id = self.place_piece(player_id, piece_type, pos);
            if piece_type.is_king() {
                king_id = id;
            }
        }
        Deployment {
            king_id,
            board_rotation_deg: 0,
        }
    }

    /// Picks a square for one kit piece near `spawn`.
    ///
    /// Kings prefer the spawn square itself, other pieces prefer squares around it.
    fn kit_piece_position(
        &self,
        spawn: BoardCoord,
        is_king: bool,
        rng: &mut impl Rng,
    ) -> BoardCoord {
        let spawn_if_free = || self.state.is_free(spawn).then_some(spawn);
        let mut nearby = || {
            find_random_nearby_free_pos(
                &self.state,
                spawn,
                &mut *rng,
                KIT_SPREAD_RADIUS,
                KIT_PLACEMENT_ATTEMPTS,
            )
        };
        let preferred = if is_king {
            spawn_if_free().or_else(nearby)
        } else {
            nearby().or_else(spawn_if_free)
        };
        preferred.unwrap_or_else(|| find_spawn_pos(&self.state, rng))
    }

    /// Inserts one ready-to-move piece for `player_id`.
    fn place_piece(
        &mut self,
        player_id: PlayerId,
        piece_type: &PieceTypeId,
        pos: BoardCoord,
    ) -> PieceId {
        let piece = Piece::new(Some(player_id), piece_type.clone(), pos);
        let id = piece.id;
        self.state.pieces.insert(id, piece);
        id
    }
}
