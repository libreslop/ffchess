#![allow(deprecated)]
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use common::*;
use glam::IVec2;
use uuid::Uuid;
use std::collections::HashMap;

pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub width: f64,
    pub height: f64,
    pub tile_size: f64,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement) -> Self {
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();
        
        Self {
            ctx,
            width: canvas.width() as f64,
            height: canvas.height() as f64,
            tile_size: 40.0,
        }
    }

    pub fn draw_with_ghosts(
        &self, 
        state: &GameState, 
        player_id: Uuid, 
        selected_piece_id: Option<Uuid>, 
        pm_queue: &[crate::Pmove], 
        ghost_pieces: &HashMap<Uuid, Piece>,
        camera_pos: (f64, f64) // Pixel coords in world
    ) {
        let player_king = state.players.get(&player_id)
            .and_then(|p| state.pieces.get(&p.king_id));
        
        // King pos for FoW center
        let king_pos = player_king.map(|k| k.position).unwrap_or_else(|| {
            state.players.values().next()
                .and_then(|p| state.pieces.get(&p.king_id))
                .map(|k| k.position)
                .unwrap_or(IVec2::new(state.board_size / 2, state.board_size / 2))
        });
        
        let piece_count = state.pieces.values().filter(|p| p.owner_id == Some(player_id)).count();
        let zoom_factor = (piece_count as f64).sqrt().max(1.0);
        let view_radius_squares = if player_id == Uuid::nil() { 100 } else { (20.0 * zoom_factor) as i32 }; 
        let view_radius_px = (view_radius_squares as f64 + 0.5) * self.tile_size;

        // Background
        self.ctx.set_fill_style(&JsValue::from_str("#1a1a1a"));
        self.ctx.fill_rect(0.0, 0.0, self.width, self.height);
        
        // Offset mapping world -> screen
        let offset_x = self.width / 2.0 - camera_pos.0;
        let offset_y = self.height / 2.0 - camera_pos.1;

        // Clip FoW around KING (only if joined)
        self.ctx.save();
        if player_id != Uuid::nil() {
            let king_screen_x = king_pos.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0;
            let king_screen_y = king_pos.y as f64 * self.tile_size + offset_y + self.tile_size / 2.0;

            self.ctx.begin_path();
            let _ = self.ctx.arc(king_screen_x, king_screen_y, view_radius_px, 0.0, std::f64::consts::TAU);
            self.ctx.clip();
        }

        self.ctx.set_fill_style(&JsValue::from_str("#fafafa"));
        self.ctx.fill_rect(0.0, 0.0, self.width, self.height);

        // Grid & Checkerboard (only within visible range)
        self.ctx.set_stroke_style(&JsValue::from_str("#eee"));
        for x in (king_pos.x - view_radius_squares)..(king_pos.x + view_radius_squares + 1) {
            for y in (king_pos.y - view_radius_squares)..(king_pos.y + view_radius_squares + 1) {
                if is_within_board(IVec2::new(x, y), state.board_size) {
                    if (x + y) % 2 != 0 {
                        self.ctx.set_fill_style(&JsValue::from_str("#f4f4f4"));
                        self.ctx.fill_rect(x as f64 * self.tile_size + offset_x, y as f64 * self.tile_size + offset_y, self.tile_size, self.tile_size);
                    }
                    self.ctx.stroke_rect(x as f64 * self.tile_size + offset_x, y as f64 * self.tile_size + offset_y, self.tile_size, self.tile_size);
                }
            }
        }

        // Shops
        for shop in &state.shops {
            if (shop.position - king_pos).abs().max_element() <= view_radius_squares {
                self.ctx.set_fill_style(&JsValue::from_str("#ff0"));
                self.ctx.fill_rect(shop.position.x as f64 * self.tile_size + offset_x + 5.0, shop.position.y as f64 * self.tile_size + offset_y + 5.0, self.tile_size - 10.0, self.tile_size - 10.0);
            }
        }

        // Highlights for valid moves
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid) {
            self.ctx.set_fill_style(&JsValue::from_str("rgba(0, 255, 0, 0.1)"));
            let range = 10;
            for x in (piece.position.x - range)..(piece.position.x + range + 1) {
                for y in (piece.position.y - range)..(piece.position.y + range + 1) {
                    let t = IVec2::new(x, y);
                    if !is_within_board(t, state.board_size) { continue; }
                    
                    let target_piece = state.pieces.values().find(|p| p.position == t);
                    let is_friendly = target_piece.map(|tp| tp.owner_id == Some(player_id)).unwrap_or(false);
                    if is_friendly { continue; }
                    
                    let is_capture = target_piece.is_some();
                    if is_valid_chess_move(piece.piece_type, piece.position, t, is_capture, state.board_size) {
                        let blocked = piece.piece_type != PieceType::Knight && is_move_blocked(piece.position, t, &state.pieces);
                        if !blocked {
                            self.ctx.fill_rect(x as f64 * self.tile_size + offset_x + 2.0, y as f64 * self.tile_size + offset_y + 2.0, self.tile_size - 4.0, self.tile_size - 4.0);
                        }
                    }
                }
            }
        }

        // Premove lines
        self.ctx.set_stroke_style(&JsValue::from_str("rgba(0, 0, 255, 0.4)"));
        self.ctx.set_line_width(2.0);
        for pm in pm_queue {
            if let Some(real_p) = state.pieces.get(&pm.piece_id) {
                let mut start_pos = real_p.position;
                for prev in pm_queue {
                    if prev == pm { break; }
                    if prev.piece_id == pm.piece_id { start_pos = prev.target; }
                }
                self.ctx.begin_path();
                self.ctx.move_to(start_pos.x as f64 * self.tile_size + offset_x + self.tile_size/2.0, start_pos.y as f64 * self.tile_size + offset_y + self.tile_size/2.0);
                self.ctx.line_to(pm.target.x as f64 * self.tile_size + offset_x + self.tile_size/2.0, pm.target.y as f64 * self.tile_size + offset_y + self.tile_size/2.0);
                self.ctx.stroke();
            }
        }

        // Real pieces
        for piece in state.pieces.values() {
            if (piece.position - king_pos).abs().max_element() <= view_radius_squares {
                self.draw_piece(piece, player_id, offset_x, offset_y, 1.0, state);
            }
        }

        // Ghosts
        for (id, ghost) in ghost_pieces {
            if let Some(real) = state.pieces.get(id)
                && real.position != ghost.position {
                self.draw_piece(ghost, player_id, offset_x, offset_y, 0.4, state);
            }
        }

        self.ctx.restore();
    }

    fn draw_piece(&self, piece: &Piece, player_id: Uuid, offset_x: f64, offset_y: f64, alpha: f64, state: &GameState) {
        let color = if piece.owner_id == Some(player_id) {
            "rgba(0, 0, 255, "
        } else if piece.owner_id.is_none() {
            "rgba(85, 85, 85, "
        } else {
            "rgba(255, 0, 0, "
        };
        let final_color = format!("{}{})", color, alpha);

        self.ctx.set_fill_style(&JsValue::from_str(&final_color));
        self.ctx.begin_path();
        let _ = self.ctx.arc(piece.position.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0, piece.position.y as f64 * self.tile_size + offset_y + self.tile_size / 2.0, self.tile_size / 3.0, 0.0, std::f64::consts::TAU);
        self.ctx.fill();

        self.ctx.set_fill_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", alpha)));
        self.ctx.set_font("bold 16px Arial");
        let label = match piece.piece_type {
            PieceType::King => "K", PieceType::Queen => "Q", PieceType::Rook => "R", PieceType::Bishop => "B", PieceType::Knight => "N", PieceType::Pawn => "P",
        };
        let _ = self.ctx.fill_text(label, piece.position.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0 - 5.0, piece.position.y as f64 * self.tile_size + offset_y + self.tile_size / 2.0 + 6.0);

        if alpha >= 1.0 {
            let now = chrono::Utc::now().timestamp_millis();
            let elapsed = now - piece.last_move_time;
            if elapsed < piece.cooldown_ms {
                let progress = elapsed as f64 / piece.cooldown_ms as f64;
                self.ctx.set_fill_style(&JsValue::from_str("#000"));
                self.ctx.fill_rect(piece.position.x as f64 * self.tile_size + offset_x + 5.0, piece.position.y as f64 * self.tile_size + offset_y + self.tile_size - 8.0, self.tile_size - 10.0, 4.0);
                self.ctx.set_fill_style(&JsValue::from_str("#0f0"));
                self.ctx.fill_rect(piece.position.x as f64 * self.tile_size + offset_x + 5.0, piece.position.y as f64 * self.tile_size + offset_y + self.tile_size - 8.0, (self.tile_size - 10.0) * progress, 4.0);
            }
// Draw player name above King
if piece.piece_type == PieceType::King
    && let Some(owner_id) = piece.owner_id
    && let Some(player) = state.players.get(&owner_id) {
    let name = if player.name.trim().is_empty() { "An Unnamed Player" } else { &player.name };
    self.ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.7)"));
    self.ctx.set_font("12px Arial");
    if let Ok(text_metrics) = self.ctx.measure_text(name) {
        let text_width = text_metrics.width();
        let _ = self.ctx.fill_text(
            name,
            piece.position.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0 - text_width / 2.0,
            piece.position.y as f64 * self.tile_size + offset_y - 5.0,
        );
    }
}
        }

        if piece.piece_type == PieceType::Pawn {
            // No orientation indicator anymore
        }
    }
}
