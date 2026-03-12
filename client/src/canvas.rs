#![allow(deprecated)]
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use common::*;
use glam::IVec2;
use uuid::Uuid;
use std::collections::HashMap;
use crate::reducer::Pmove;

pub struct Renderer {
    pub ctx: CanvasRenderingContext2d,
    pub width: f64,
    pub height: f64,
    pub tile_size: f64,
    pub zoom: f64,
}

struct PieceDrawParams<'a> {
    piece: &'a Piece,
    player_id: Uuid,
    offset_x: f64,
    offset_y: f64,
    alpha: f64,
    state: &'a GameState,
    draw_name: bool,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement, zoom: f64) -> Self {
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
            tile_size: 40.0 * zoom,
            zoom,
        }
    }

    pub fn draw_with_ghosts(
        &self, 
        state: &GameState, 
        player_id: Uuid, 
        selected_piece_id: Option<Uuid>, 
        pm_queue: &[Pmove], 
        ghost_pieces: &HashMap<Uuid, Piece>,
        camera_pos: (f64, f64) // Pixel coords in world
    ) {
        let player_king = state.players.get(&player_id)
            .and_then(|p| state.pieces.get(&p.king_id));
        
        let has_king = player_king.is_some();
        let king_pos = player_king.map(|k| k.position).unwrap_or(IVec2::ZERO);
        let piece_count = state.pieces.values().filter(|p| p.owner_id == Some(player_id)).count();
        let zoom_factor = (piece_count as f64).sqrt().max(1.0);
        let view_radius_squares = if player_id == Uuid::nil() || !has_king { 100 } else { (15.0 * zoom_factor) as i32 }; 
        let view_radius_px = (view_radius_squares as f64 + 0.5) * self.tile_size;

        // Background (Off-board color)
        self.ctx.set_fill_style(&JsValue::from_str("#e2e8f0"));
        self.ctx.fill_rect(0.0, 0.0, self.width, self.height);
        
        // Offset mapping world -> screen
        let offset_x = self.width / 2.0 - camera_pos.0;
        let offset_y = self.height / 2.0 - camera_pos.1;

        let half = state.board_size / 2;
        let limit_pos = (state.board_size + 1) / 2;

        // Board Pixel Boundaries
        let board_left = -(half as f64) * self.tile_size + offset_x;
        let board_top = -(half as f64) * self.tile_size + offset_y;
        let board_dim = state.board_size as f64 * self.tile_size;

        // Draw Board Background
        self.ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        self.ctx.fill_rect(board_left, board_top, board_dim, board_dim);

        // Calculate visible range for optimizations
        let v_start_x = ((-offset_x) / self.tile_size).floor() as i32;
        let v_end_x = ((self.width - offset_x) / self.tile_size).ceil() as i32;
        let v_start_y = ((-offset_y) / self.tile_size).floor() as i32;
        let v_end_y = ((self.height - offset_y) / self.tile_size).ceil() as i32;

        let start_x = v_start_x.clamp(-half, limit_pos);
        let end_x = v_end_x.clamp(-half, limit_pos);
        let start_y = v_start_y.clamp(-half, limit_pos);
        let end_y = v_end_y.clamp(-half, limit_pos);

        // Checkerboard
        self.ctx.set_fill_style(&JsValue::from_str("#f1f5f9"));
        for x in start_x..end_x {
            for y in start_y..end_y {
                // Proper checkerboard for centered system
                if (x.rem_euclid(2) + y.rem_euclid(2)) % 2 != 0 {
                    self.ctx.fill_rect(x as f64 * self.tile_size + offset_x, y as f64 * self.tile_size + offset_y, self.tile_size, self.tile_size);
                }
            }
        }

        // Grid Lines
        self.ctx.set_stroke_style(&JsValue::from_str("#cbd5e1"));
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        
        for x in start_x..=end_x {
            self.ctx.move_to(x as f64 * self.tile_size + offset_x, start_y as f64 * self.tile_size + offset_y);
            self.ctx.line_to(x as f64 * self.tile_size + offset_x, end_y as f64 * self.tile_size + offset_y);
        }
        for y in start_y..=end_y {
            self.ctx.move_to(start_x as f64 * self.tile_size + offset_x, y as f64 * self.tile_size + offset_y);
            self.ctx.line_to(end_x as f64 * self.tile_size + offset_x, y as f64 * self.tile_size + offset_y);
        }
        self.ctx.stroke();

        // Draw Board Border (Above Grid)
        self.ctx.set_stroke_style(&JsValue::from_str("#1e293b"));
        self.ctx.set_line_width(2.0);
        self.ctx.stroke_rect(board_left, board_top, board_dim, board_dim);

        // Shops
        for shop in &state.shops {
            if (shop.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.ctx.set_fill_style(&JsValue::from_str("#fde047"));
                self.ctx.fill_rect(shop.position.x as f64 * self.tile_size + offset_x + 5.0 * self.zoom, shop.position.y as f64 * self.tile_size + offset_y + 5.0 * self.zoom, self.tile_size - 10.0 * self.zoom, self.tile_size - 10.0 * self.zoom);
            }
        }

        // Highlights for valid moves
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid) {
            self.ctx.set_fill_style(&JsValue::from_str("rgba(34, 197, 94, 0.2)"));
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

        // Pmove lines
        self.ctx.set_stroke_style(&JsValue::from_str("rgba(59, 130, 246, 0.5)"));
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
            if (piece.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.draw_piece(PieceDrawParams {
                    piece, player_id, offset_x, offset_y, alpha: 1.0, state, draw_name: false
                });
            }
        }

        // Ghosts
        for (id, ghost) in ghost_pieces {
            if let Some(real) = state.pieces.get(id)
                && real.position != ghost.position {
                self.draw_piece(PieceDrawParams {
                    piece: ghost, player_id, offset_x, offset_y, alpha: 0.4, state, draw_name: false
                });
            }
        }

        // Second pass: Draw player names on top of everything
        for piece in state.pieces.values() {
            if piece.piece_type == PieceType::King && (piece.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.draw_piece_name(piece, offset_x, offset_y, 1.0, state);
            }
        }

        // Fog of War Overlay
        if player_id != Uuid::nil() && has_king {
            let king_screen_x = king_pos.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0;
            let king_screen_y = king_pos.y as f64 * self.tile_size + offset_y + self.tile_size / 2.0;

            let gradient = self.ctx.create_radial_gradient(
                king_screen_x, king_screen_y, view_radius_px * 0.6,
                king_screen_x, king_screen_y, view_radius_px
            ).unwrap();
            
            let _ = gradient.add_color_stop(0.0, "rgba(255, 255, 255, 0.0)");
            let _ = gradient.add_color_stop(1.0, "rgba(255, 255, 255, 1.0)");

            self.ctx.set_fill_style(&gradient);
            self.ctx.fill_rect(0.0, 0.0, self.width, self.height);
        }
    }

    fn draw_piece(&self, params: PieceDrawParams) {
        let color = if params.piece.owner_id == Some(params.player_id) {
            "rgba(0, 0, 255, "
        } else if params.piece.owner_id.is_none() {
            "rgba(85, 85, 85, "
        } else {
            "rgba(255, 0, 0, "
        };
        let final_color = format!("{}{})", color, params.alpha);

        self.ctx.set_fill_style(&JsValue::from_str(&final_color));
        self.ctx.begin_path();
        let _ = self.ctx.arc(params.piece.position.x as f64 * self.tile_size + params.offset_x + self.tile_size / 2.0, params.piece.position.y as f64 * self.tile_size + params.offset_y + self.tile_size / 2.0, self.tile_size / 3.0, 0.0, std::f64::consts::TAU);
        self.ctx.fill();

        self.ctx.set_fill_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", params.alpha)));
        let font_size = 16.0 * self.zoom;
        self.ctx.set_font(&format!("bold {}px Arbutus", font_size));
        let label = match params.piece.piece_type {
            PieceType::King => "K", PieceType::Queen => "Q", PieceType::Rook => "R", PieceType::Bishop => "B", PieceType::Knight => "N", PieceType::Pawn => "P",
        };
        let _ = self.ctx.fill_text(label, params.piece.position.x as f64 * self.tile_size + params.offset_x + self.tile_size / 2.0 - (5.0 * self.zoom), params.piece.position.y as f64 * self.tile_size + params.offset_y + self.tile_size / 2.0 + (6.0 * self.zoom));

        if params.alpha >= 1.0 && params.piece.owner_id == Some(params.player_id) {
            let now = chrono::Utc::now().timestamp_millis();
            let elapsed = now - params.piece.last_move_time;
            if elapsed < params.piece.cooldown_ms {
                let progress = elapsed as f64 / params.piece.cooldown_ms as f64;
                self.ctx.set_fill_style(&JsValue::from_str("#000"));
                let bar_h = 4.0 * self.zoom;
                let bar_margin = 5.0 * self.zoom;
                let bar_y_offset = self.tile_size - (8.0 * self.zoom);
                
                self.ctx.fill_rect(params.piece.position.x as f64 * self.tile_size + params.offset_x + bar_margin, params.piece.position.y as f64 * self.tile_size + params.offset_y + bar_y_offset, self.tile_size - (bar_margin * 2.0), bar_h);
                self.ctx.set_fill_style(&JsValue::from_str("#0f0"));
                self.ctx.fill_rect(params.piece.position.x as f64 * self.tile_size + params.offset_x + bar_margin, params.piece.position.y as f64 * self.tile_size + params.offset_y + bar_y_offset, (self.tile_size - (bar_margin * 2.0)) * progress, bar_h);
            }
        }
        
        if params.alpha >= 1.0 && params.draw_name {
            self.draw_piece_name(params.piece, params.offset_x, params.offset_y, params.alpha, params.state);
        }
    }

    fn draw_piece_name(&self, piece: &Piece, offset_x: f64, offset_y: f64, alpha: f64, state: &GameState) {
        if piece.piece_type == PieceType::King
            && let Some(owner_id) = piece.owner_id
            && let Some(player) = state.players.get(&owner_id) {
            let name = if player.name.trim().is_empty() { "An Unnamed Player" } else { &player.name };
            
            let name_font_size = 12.0 * self.zoom;
            self.ctx.set_font(&format!("{}px Arbutus", name_font_size));
            
            if let Ok(text_metrics) = self.ctx.measure_text(name) {
                let text_width = text_metrics.width();
                let x = piece.position.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0 - text_width / 2.0;
                let y = piece.position.y as f64 * self.tile_size + offset_y - (5.0 * self.zoom);

                // Draw outline
                self.ctx.set_stroke_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", alpha)));
                self.ctx.set_line_width(3.0 * self.zoom);
                let _ = self.ctx.stroke_text(name, x, y);

                // Draw fill
                self.ctx.set_fill_style(&JsValue::from_str(&format!("rgba(0, 0, 0, {})", 0.7 * alpha)));
                let _ = self.ctx.fill_text(name, x, y);
            }
        }
    }
}
