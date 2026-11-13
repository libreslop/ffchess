//! Piece and shop definitions shared by the server and its clients.

use crate::models::{GameState, PieceConfig, ShopConfig};
use crate::protocol::GameError;
use crate::types::{DurationMs, ExprVars, PieceTypeId, PlayerId, Score, ShopId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cooldown applied to pieces whose type has no config entry.
const FALLBACK_COOLDOWN: DurationMs = DurationMs(1000);

/// All piece and shop configs loaded for a server.
///
/// Equality is `Eq` so `Rc<Catalog>` comparisons short-circuit on pointer identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub pieces: HashMap<PieceTypeId, PieceConfig>,
    pub shops: HashMap<ShopId, ShopConfig>,
}

impl Catalog {
    /// Returns the config of a piece type.
    pub fn piece(&self, id: &PieceTypeId) -> Result<&PieceConfig, GameError> {
        self.pieces
            .get(id)
            .ok_or_else(|| GameError::Internal(format!("Piece config {id} not found")))
    }

    /// Returns the config of a shop type.
    pub fn shop(&self, id: &ShopId) -> Result<&ShopConfig, GameError> {
        self.shops
            .get(id)
            .ok_or_else(|| GameError::Internal(format!("Shop config {id} not found")))
    }

    /// Checks that every special move uses valid conditions and known piece types.
    ///
    /// Returns a message naming the offending piece type otherwise.
    pub fn validate(&self) -> Result<(), String> {
        self.pieces.values().try_for_each(|config| {
            config
                .special_moves
                .iter()
                .try_for_each(|special| special.validate(self))
                .map_err(|error| format!("piece `{}` special move: {error}", config.id))
        })
    }

    /// Returns the movement cooldown of a piece type.
    pub fn cooldown_of(&self, id: &PieceTypeId) -> DurationMs {
        self.pieces
            .get(id)
            .map_or(FALLBACK_COOLDOWN, |config| config.cooldown_ms)
    }

    /// Returns the score awarded for capturing a piece type.
    pub fn score_of(&self, id: &PieceTypeId) -> Score {
        self.pieces
            .get(id)
            .map_or_else(Score::zero, |config| config.score_value)
    }

    /// Builds the variables available to shop price expressions for `player_id`.
    ///
    /// Provides `player_piece_count` plus one `<piece_type>_count` per configured piece type.
    pub fn price_vars(&self, state: &GameState, player_id: PlayerId) -> ExprVars {
        let mut counts: HashMap<&PieceTypeId, usize> =
            self.pieces.keys().map(|id| (id, 0)).collect();
        let mut total: usize = 0;
        for piece in state.pieces_of(player_id) {
            total += 1;
            if let Some(count) = counts.get_mut(&piece.piece_type) {
                *count += 1;
            }
        }

        counts.into_iter().fold(
            ExprVars::new().with("player_piece_count", total),
            |vars, (id, count)| vars.with(format!("{id}_count"), count),
        )
    }
}
