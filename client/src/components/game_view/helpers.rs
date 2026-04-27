//! Helper utilities for the game view component.

use crate::reducer::Pmove;
use common::models::{GameState, Piece, Shop, ShopConfig};
use common::types::{BoardCoord, PieceId, ShopId};
use std::collections::HashMap;

/// Duration of a single move animation in milliseconds.
pub const MOVE_ANIM_MS: f64 = 200.0;

/// Applies visible queued moves to ghost piece positions.
///
/// `ghosts` is the mutable ghost map, `pm_queue` is the pending move list,
/// and `state` is the current game state. Returns nothing.
pub fn apply_visible_ghosts(
    ghosts: &mut HashMap<PieceId, Piece>,
    pm_queue: &[Pmove],
    state: &GameState,
    shop_configs: &HashMap<ShopId, ShopConfig>,
) {
    let mut virtual_shops = state.shops.clone();

    for pm in pm_queue {
        if !pm_visible(pm, state) {
            continue;
        }

        if let Some(p) = ghosts.get_mut(&pm.piece_id) {
            p.position = BoardCoord(pm.target);
        }

        apply_auto_shop_action_for_premove(
            ghosts,
            &mut virtual_shops,
            shop_configs,
            pm.piece_id,
            BoardCoord(pm.target),
        );
    }
}

/// Determines if a pending move is visible based on current game state.
///
/// `pm` is the pending move and `state` is the game state. Returns `true` if visible.
pub fn pm_visible(pm: &Pmove, state: &GameState) -> bool {
    // Show ghosts/paths for any queued move as long as the piece still exists.
    state.pieces.contains_key(&pm.piece_id)
}

/// Applies auto single-item shop actions to a ghost piece after a premove lands.
fn apply_auto_shop_action_for_premove(
    ghosts: &mut HashMap<PieceId, Piece>,
    virtual_shops: &mut Vec<Shop>,
    shop_configs: &HashMap<ShopId, ShopConfig>,
    piece_id: PieceId,
    piece_pos: BoardCoord,
) {
    let Some((shop_index, shop_id)) = virtual_shops
        .iter()
        .enumerate()
        .find(|(_, shop)| shop.position == piece_pos)
        .map(|(i, shop)| (i, shop.shop_id.clone()))
    else {
        return;
    };

    let Some(shop_config) = shop_configs.get(&shop_id) else {
        return;
    };
    if !shop_config.auto_upgrade_single_item {
        return;
    }

    let Some(piece_on_shop) = ghosts.get(&piece_id).cloned() else {
        return;
    };
    let Some(group) = common::logic::select_shop_group(shop_config, Some(&piece_on_shop)) else {
        return;
    };
    if group.items.len() != 1 {
        return;
    }
    let item = &group.items[0];

    if let Some(replace_with) = &item.replace_with
        && let Some(piece) = ghosts.get_mut(&piece_id)
    {
        piece.piece_type = replace_with.clone();
    }

    if let Some(shop) = virtual_shops.get_mut(shop_index)
        && shop.uses_remaining > 0
    {
        shop.uses_remaining -= 1;
    }
    if virtual_shops
        .get(shop_index)
        .map(|shop| shop.uses_remaining == 0)
        .unwrap_or(false)
    {
        virtual_shops.remove(shop_index);
    }
}
