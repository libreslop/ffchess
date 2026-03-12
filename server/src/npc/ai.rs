use common::*;
use glam::IVec2;
use rand::Rng;

pub async fn find_npc_target(game: &GameState, pos: IVec2, p_type: PieceType) -> Option<IVec2> {
    let board_size = game.board_size;

    // 1. Check if visible to any player
    let mut visible_to_player = None;
    for player in game.players.values() {
        if let Some(king) = game.pieces.get(&player.king_id) {
            let p_piece_count = game
                .pieces
                .values()
                .filter(|p| p.owner_id == Some(player.id))
                .count();
            let view_radius = (10.0 * (p_piece_count as f64).sqrt().max(1.0)) as i32;
            if (pos - king.position).abs().max_element() <= view_radius {
                visible_to_player = Some(player.id);
                break;
            }
        }
    }

    // Only engage if a player is relatively close (12 squares)
    let mut player_nearby = false;
    if let Some(pid) = visible_to_player {
        for piece in game.pieces.values() {
            if piece.owner_id == Some(pid) && (piece.position - pos).abs().max_element() <= 12 {
                player_nearby = true;
                break;
            }
        }
    }

    if player_nearby {
        // 2. Aggressive Mode: PRIORITIZE King captures, then other player pieces
        let mut king_target = None;
        let mut other_target = None;

        for other_p in game.pieces.values() {
            if other_p.owner_id.is_some() {
                let dist = (other_p.position - pos).abs();
                if dist.max_element() <= 10
                    && is_within_board(other_p.position, board_size)
                    && is_valid_chess_move(p_type, pos, other_p.position, true, board_size)
                    && (p_type == PieceType::Knight
                        || !is_move_blocked(pos, other_p.position, &game.pieces))
                {
                    if other_p.piece_type == PieceType::King {
                        king_target = Some(other_p.position);
                        break;
                    } else {
                        other_target = Some(other_p.position);
                    }
                }
            }
        }

        if let Some(t) = king_target {
            return Some(t);
        }
        if let Some(t) = other_target {
            return Some(t);
        }

        // 3. Hunt Mode: Move toward the nearest player piece (PRIORITIZE King)
        let mut nearest_p_pos = None;
        let mut min_dist = f32::MAX;

        for player in game.players.values() {
            if let Some(king) = game.pieces.get(&player.king_id) {
                if !is_within_board(king.position, board_size) {
                    continue;
                }
                let d = (king.position - pos).as_vec2().length();
                if d < 12.0 {
                    min_dist = d;
                    nearest_p_pos = Some(king.position);
                    break;
                }
            }
        }

        if nearest_p_pos.is_none() {
            for other_p in game.pieces.values() {
                if other_p.owner_id.is_some() {
                    if !is_within_board(other_p.position, board_size) {
                        continue;
                    }
                    let d = (other_p.position - pos).as_vec2().length();
                    if d < 12.0 && d < min_dist {
                        min_dist = d;
                        nearest_p_pos = Some(other_p.position);
                    }
                }
            }
        }

        if let Some(npp) = nearest_p_pos {
            let mut best_move = None;
            let mut best_dist = min_dist;
            let range = 4;
            for dx in -range..=range {
                for dy in -range..=range {
                    let t = pos + IVec2::new(dx, dy);
                    if is_within_board(t, board_size)
                        && is_valid_chess_move(p_type, pos, t, false, board_size)
                        && (p_type == PieceType::Knight || !is_move_blocked(pos, t, &game.pieces))
                        && !game.pieces.values().any(|pc| pc.position == t)
                    {
                        let d = (npp - t).as_vec2().length();
                        if d < best_dist {
                            best_dist = d;
                            best_move = Some(t);
                        }
                    }
                }
            }
            if best_move.is_some() {
                return best_move;
            }
        }
    }

    // 4. Roam Mode (Fallback)
    match p_type {
        PieceType::Pawn => {
            let mut rng = rand::thread_rng();
            let directions = [
                IVec2::new(0, -1),
                IVec2::new(1, 0),
                IVec2::new(0, 1),
                IVec2::new(-1, 0),
            ];
            let dir = directions[rng.gen_range(0..4)];
            let t = pos + dir;
            if is_within_board(t, board_size) && !game.pieces.values().any(|pc| pc.position == t) {
                Some(t)
            } else {
                None
            }
        }
        _ => {
            let mut potential_targets = Vec::new();
            let range = 2;
            for dx in -range..=range {
                for dy in -range..=range {
                    let t = pos + IVec2::new(dx, dy);
                    if is_within_board(t, board_size)
                        && is_valid_chess_move(p_type, pos, t, false, board_size)
                        && (p_type == PieceType::Knight || !is_move_blocked(pos, t, &game.pieces))
                        && !game.pieces.values().any(|pc| pc.position == t)
                    {
                        potential_targets.push(t);
                    }
                }
            }
            if !potential_targets.is_empty() {
                let mut rng = rand::thread_rng();
                Some(potential_targets[rng.gen_range(0..potential_targets.len())])
            } else {
                None
            }
        }
    }
}
