//! Deterministic application of piece actions to a game state.
//!
//! The server runs this against the live board and clients run it against a copy of the
//! latest snapshot to predict queued actions, so both sides agree on what an action does.

use super::catalog::Catalog;
use super::movement::MoveKind;
use crate::models::{GameState, Piece, PieceAction, ShopItemConfig, ShopItemIndex};
use crate::protocol::GameError;
use crate::types::{BoardCoord, PieceId, PlayerId, ShopId, TimestampMs};
use glam::IVec2;

/// Neighbour offsets searched, in order, when placing pieces bought from a shop.
const ADJACENT_OFFSETS: [IVec2; 8] = [
    IVec2::new(1, 0),
    IVec2::new(-1, 0),
    IVec2::new(0, 1),
    IVec2::new(0, -1),
    IVec2::new(1, 1),
    IVec2::new(-1, 1),
    IVec2::new(1, -1),
    IVec2::new(-1, -1),
];

/// Side effects produced by one applied action.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActionEffects {
    /// Piece removed by a capture.
    pub captured: Option<Piece>,
    /// Shops whose last use was consumed and that were removed from the board.
    pub depleted_shops: Vec<ShopId>,
}

impl ActionEffects {
    /// Folds the effects of a follow-up action into these effects.
    fn merge(mut self, other: ActionEffects) -> Self {
        self.captured = self.captured.or(other.captured);
        self.depleted_shops.extend(other.depleted_shops);
        self
    }
}

impl GameState {
    /// Returns the first free square adjacent to `origin`.
    pub fn adjacent_free(&self, origin: BoardCoord) -> Option<BoardCoord> {
        ADJACENT_OFFSETS
            .iter()
            .map(|offset| origin + *offset)
            .find(|pos| self.is_free(*pos))
    }
}

/// Applies piece actions to a mutable game state using one catalog.
pub struct Simulation<'a> {
    catalog: &'a Catalog,
    state: &'a mut GameState,
    now: TimestampMs,
}

impl<'a> Simulation<'a> {
    /// Creates a simulation that stamps moves with `now`.
    pub fn new(catalog: &'a Catalog, state: &'a mut GameState, now: TimestampMs) -> Self {
        Self {
            catalog,
            state,
            now,
        }
    }

    /// Applies one action to `piece_id`, ignoring cooldowns.
    pub fn apply(
        &mut self,
        piece_id: PieceId,
        action: PieceAction,
    ) -> Result<ActionEffects, GameError> {
        match action {
            PieceAction::Move { target } => self.apply_move(piece_id, target),
            PieceAction::Buy { item } => self.apply_purchase(piece_id, item),
        }
    }

    /// Moves a piece, resolving captures, rewards, and automatic shop upgrades.
    pub fn apply_move(
        &mut self,
        piece_id: PieceId,
        target: BoardCoord,
    ) -> Result<ActionEffects, GameError> {
        let piece = self.piece(piece_id)?.clone();
        let config = self.catalog.piece(&piece.piece_type)?;
        let kind = config.check_move(self.state, piece.position, target, piece.owner_id)?;

        let captured = match kind {
            MoveKind::Capture => self.capture_at(target, piece.owner_id),
            MoveKind::Quiet => None,
        };

        let cooldown = config.cooldown_ms;
        if let Some(moved) = self.state.pieces.get_mut(&piece_id) {
            moved.position = target;
            moved.last_move_time = self.now;
            moved.cooldown_ms = cooldown;
        }

        let effects = ActionEffects {
            captured,
            ..ActionEffects::default()
        };
        // An automatic upgrade that cannot be afforded or placed simply does not happen.
        let upgrade = self.apply_auto_upgrade(piece_id).unwrap_or_default();
        Ok(effects.merge(upgrade))
    }

    /// Buys the `item`-th offer of the shop under `piece_id` for the piece's owner.
    pub fn apply_purchase(
        &mut self,
        piece_id: PieceId,
        item: ShopItemIndex,
    ) -> Result<ActionEffects, GameError> {
        let piece = self.piece(piece_id)?;
        let shop = self
            .state
            .shop_at(piece.position)
            .ok_or(GameError::ShopNotFound)?;
        let shop_config = self.catalog.shop(&shop.shop_id)?;
        let item = shop_config
            .group_for(Some(piece))
            .ok_or(GameError::NoPieceOnShop)?
            .items
            .get(item.0)
            .ok_or_else(|| GameError::Internal("Invalid shop item index".to_string()))?;
        self.apply_item(piece_id, item)
    }

    /// Buys the automatic offer of the shop under `piece_id`, if there is one.
    fn apply_auto_upgrade(&mut self, piece_id: PieceId) -> Result<ActionEffects, GameError> {
        let piece = self.piece(piece_id)?;
        if !piece.is_player_owned() {
            return Ok(ActionEffects::default());
        }
        let Some(shop) = self.state.shop_at(piece.position) else {
            return Ok(ActionEffects::default());
        };
        match self.catalog.shop(&shop.shop_id)?.auto_item_for(piece) {
            Some(item) => self.apply_item(piece_id, item),
            None => Ok(ActionEffects::default()),
        }
    }

    /// Charges the owner of `piece_id` and applies `item` from the shop under the piece.
    ///
    /// All checks happen before any mutation, so a failed purchase leaves the state untouched.
    fn apply_item(
        &mut self,
        piece_id: PieceId,
        item: &ShopItemConfig,
    ) -> Result<ActionEffects, GameError> {
        let piece = self.piece(piece_id)?;
        let shop_pos = piece.position;
        let buyer = piece.owner_id.ok_or(GameError::NoPieceOnShop)?;
        let price = item.price(&self.catalog.price_vars(self.state, buyer));
        let have = self
            .state
            .players
            .get(&buyer)
            .ok_or(GameError::PlayerNotFound)?
            .score;
        if have < price {
            return Err(GameError::InsufficientScore {
                needed: price,
                have,
            });
        }
        let placements = self.plan_placements(shop_pos, item.add_pieces.len())?;

        if let Some(player) = self.state.players.get_mut(&buyer) {
            player.score -= price;
        }
        if let Some(new_type) = &item.replace_with {
            let cooldown = self.catalog.cooldown_of(new_type);
            if let Some(piece) = self.state.pieces.get_mut(&piece_id) {
                piece.piece_type = new_type.clone();
                piece.cooldown_ms = cooldown;
            }
        }
        for (piece_type, position) in item.add_pieces.iter().zip(placements) {
            let piece = Piece::new(Some(buyer), piece_type.clone(), position);
            self.state.pieces.insert(piece.id, piece);
        }

        Ok(ActionEffects {
            depleted_shops: self.consume_shop_use(shop_pos).into_iter().collect(),
            ..ActionEffects::default()
        })
    }

    /// Picks distinct free squares next to `origin` for `count` new pieces.
    fn plan_placements(
        &self,
        origin: BoardCoord,
        count: usize,
    ) -> Result<Vec<BoardCoord>, GameError> {
        let mut placements: Vec<BoardCoord> = Vec::with_capacity(count);
        for _ in 0..count {
            let pos = ADJACENT_OFFSETS
                .iter()
                .map(|offset| origin + *offset)
                .find(|pos| self.state.is_free(*pos) && !placements.contains(pos))
                .ok_or(GameError::NoSpaceNearby)?;
            placements.push(pos);
        }
        Ok(placements)
    }

    /// Uses up one charge of the shop at `pos`, removing it when none remain.
    ///
    /// Returns the shop type when the shop was removed.
    fn consume_shop_use(&mut self, pos: BoardCoord) -> Option<ShopId> {
        let index = self
            .state
            .shops
            .iter()
            .position(|shop| shop.position == pos)?;
        let shop = &mut self.state.shops[index];
        shop.uses_remaining = shop.uses_remaining.saturating_sub(1);
        (shop.uses_remaining == 0).then(|| self.state.shops.remove(index).shop_id)
    }

    /// Removes the piece at `pos` and rewards `capturer` for it.
    fn capture_at(&mut self, pos: BoardCoord, capturer: Option<PlayerId>) -> Option<Piece> {
        let captured_id = self.state.piece_at(pos)?.id;
        let captured = self.state.pieces.remove(&captured_id)?;
        let reward = self.catalog.score_of(&captured.piece_type);
        if let Some(player) = capturer.and_then(|id| self.state.players.get_mut(&id)) {
            player.score += reward;
            player.pieces_captured += 1;
            if captured.piece_type.is_king() {
                player.kills += 1;
            }
        }
        Some(captured)
    }

    /// Returns the piece with `piece_id`.
    fn piece(&self, piece_id: PieceId) -> Result<&Piece, GameError> {
        self.state
            .pieces
            .get(&piece_id)
            .ok_or(GameError::PieceNotFound)
    }
}
