//! Helper utilities for the game view component.

use crate::reducer::Pmove;
use common::models::{GameState, Piece, PieceConfig, Shop, ShopConfig};
use common::types::{BoardCoord, PieceId, PieceTypeId, ShopId};
use std::collections::HashMap;

/// Duration of a single move animation in milliseconds.
pub const MOVE_ANIM_MS: f64 = 200.0;

/// One validated movement segment from the projected premove queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisiblePremoveLine {
    pub piece_id: PieceId,
    pub start: BoardCoord,
    pub target: BoardCoord,
}

/// Result of projecting currently visible premoves.
#[derive(Clone, Debug, Default)]
pub struct VisibleProjection {
    pub lines: Vec<VisiblePremoveLine>,
    pub invalid_pmove_ids: Vec<u64>,
}

/// Applies visible queued moves to ghost piece positions.
///
/// `ghosts` is the mutable ghost map, `pm_queue` is the pending move list,
/// and `state` is the current game state. Returns nothing.
pub fn apply_visible_ghosts(
    ghosts: &mut HashMap<PieceId, Piece>,
    pm_queue: &[Pmove],
    state: &GameState,
    shop_configs: &HashMap<ShopId, ShopConfig>,
    piece_configs: &HashMap<PieceTypeId, PieceConfig>,
) -> VisibleProjection {
    let mut virtual_shops = state.shops.clone();
    let mut projection = VisibleProjection::default();

    for pm in pm_queue {
        if !pm_visible(pm, state) {
            continue;
        }

        if let Some(item_index) = pm.shop_item_index {
            let applied = apply_manual_shop_action_for_premove(
                ghosts,
                &mut virtual_shops,
                shop_configs,
                pm.piece_id,
                BoardCoord(pm.target),
                item_index,
            );
            if !applied {
                projection.invalid_pmove_ids.push(pm.id);
            }
        } else {
            let start = ghosts.get(&pm.piece_id).map(|piece| piece.position);
            if !validate_move_against_ghosts(
                ghosts,
                piece_configs,
                state.board_size,
                pm.piece_id,
                BoardCoord(pm.target),
            ) {
                projection.invalid_pmove_ids.push(pm.id);
                continue;
            }
            if let Some(p) = ghosts.get_mut(&pm.piece_id) {
                p.position = BoardCoord(pm.target);
            }
            if let Some(start) = start {
                projection.lines.push(VisiblePremoveLine {
                    piece_id: pm.piece_id,
                    start,
                    target: BoardCoord(pm.target),
                });
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
    projection
}

/// Determines if a pending move is visible based on current game state.
///
/// `pm` is the pending move and `state` is the game state. Returns `true` if visible.
pub fn pm_visible(pm: &Pmove, state: &GameState) -> bool {
    // Show ghosts/paths for any queued move as long as the piece still exists.
    state.pieces.contains_key(&pm.piece_id)
}

fn validate_move_against_ghosts(
    ghosts: &HashMap<PieceId, Piece>,
    piece_configs: &HashMap<PieceTypeId, PieceConfig>,
    board_size: common::types::BoardSize,
    piece_id: PieceId,
    target: BoardCoord,
) -> bool {
    let Some(piece) = ghosts.get(&piece_id) else {
        return false;
    };
    let Some(config) = piece_configs.get(&piece.piece_type) else {
        return false;
    };
    let target_piece = ghosts
        .values()
        .find(|gp| gp.position == target && gp.id != piece_id);
    if target_piece
        .map(|tp| tp.owner_id == piece.owner_id)
        .unwrap_or(false)
    {
        return false;
    }
    let is_capture = target_piece.is_some();

    common::logic::is_valid_move(common::logic::MoveValidationParams {
        piece_config: config,
        start: piece.position,
        end: target,
        is_capture,
        board_size,
        pieces: ghosts,
        moving_owner: piece.owner_id,
    })
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

/// Applies an explicitly queued shop action to a ghost piece.
fn apply_manual_shop_action_for_premove(
    ghosts: &mut HashMap<PieceId, Piece>,
    virtual_shops: &mut Vec<Shop>,
    shop_configs: &HashMap<ShopId, ShopConfig>,
    piece_id: PieceId,
    shop_pos: BoardCoord,
    item_index: usize,
) -> bool {
    let Some(piece_on_shop) = ghosts.get(&piece_id).cloned() else {
        return false;
    };
    if piece_on_shop.position != shop_pos {
        return false;
    }

    let Some((shop_index, shop_id)) = virtual_shops
        .iter()
        .enumerate()
        .find(|(_, shop)| shop.position == shop_pos)
        .map(|(i, shop)| (i, shop.shop_id.clone()))
    else {
        return false;
    };

    let Some(shop_config) = shop_configs.get(&shop_id) else {
        return false;
    };
    let Some(group) = common::logic::select_shop_group(shop_config, Some(&piece_on_shop)) else {
        return false;
    };
    let Some(item) = group.items.get(item_index) else {
        return false;
    };

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
    true
}
