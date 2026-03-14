pub mod types;
pub mod pieces;

use common::models::{GameState, Piece, PieceConfig};
use common::logic::{is_within_board, is_valid_move, evaluate_expression};
use glam::IVec2;
use uuid::Uuid;
use std::collections::HashMap;
use crate::reducer::Pmove;
use wasm_bindgen::JsCast;
pub use types::*;

use web_sys::HtmlCanvasElement;

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement, piece_configs: HashMap<String, PieceConfig>) -> Self {
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
        Self {
            ctx,
            width: canvas.width() as f64,
            height: canvas.height() as f64,
            tile_size: 40.0,
            zoom: 1.0,
            piece_configs,
        }
    }

    pub fn draw_with_ghosts(
        &self, 
        state: &GameState, 
        player_id: Uuid, 
        selected_piece_id: Option<Uuid>, 
        pm_queue: &[Pmove], 
        ghost_pieces: &HashMap<Uuid, Piece>,
        camera_pos: (f64, f64), // Pixel coords in world
        width: f64,
        height: f64,
        zoom: f64,
        mode: Option<&common::models::GameModeConfig>,
    ) {
        let tile_size = 40.0 * zoom;
        let player_king = state.players.get(&player_id)
            .and_then(|p| state.pieces.get(&p.king_id));
        
        let has_king = player_king.is_some();
        let king_pos = player_king.map(|k| k.position).unwrap_or(IVec2::ZERO);
        let piece_count = state.pieces.values().filter(|p| p.owner_id == Some(player_id)).count();
        
        let fog_of_war_radius = if let Some(m) = mode {
            let mut vars = HashMap::new();
            vars.insert("player_piece_count".to_string(), piece_count as f64);
            evaluate_expression(&m.fog_of_war_radius, &vars)
        } else {
            let zoom_factor = (piece_count as f64).sqrt().max(1.0);
            15.0 * zoom_factor
        };

        let view_radius_squares = if player_id == Uuid::nil() || !has_king { 100 } else { fog_of_war_radius as i32 }; 
        let view_radius_px = (view_radius_squares as f64 + 0.5) * tile_size;

        // Background (Off-board color)
        self.ctx.set_fill_style_str("#e2e8f0");
        self.ctx.fill_rect(0.0, 0.0, width, height);
        
        // Offset mapping world -> screen
        let offset_x = width / 2.0 - camera_pos.0;
        let offset_y = height / 2.0 - camera_pos.1;

        let half = state.board_size / 2;
        let limit_pos = (state.board_size + 1) / 2;

        // Board Pixel Boundaries
        let board_left = -(half as f64) * tile_size + offset_x;
        let board_top = -(half as f64) * tile_size + offset_y;
        let board_dim = state.board_size as f64 * tile_size;

        // Draw Board Background
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx.fill_rect(board_left, board_top, board_dim, board_dim);

        // Calculate visible range for optimizations
        let v_start_x = ((-offset_x) / tile_size).floor() as i32;
        let v_end_x = ((width - offset_x) / tile_size).ceil() as i32;
        let v_start_y = ((-offset_y) / tile_size).floor() as i32;
        let v_end_y = ((height - offset_y) / tile_size).ceil() as i32;

        let start_x = v_start_x.clamp(-half, limit_pos);
        let end_x = v_end_x.clamp(-half, limit_pos);
        let start_y = v_start_y.clamp(-half, limit_pos);
        let end_y = v_end_y.clamp(-half, limit_pos);

        // Checkerboard
        self.ctx.set_fill_style_str("#f1f5f9");
        for x in start_x..end_x {
            for y in start_y..end_y {
                // Proper checkerboard for centered system
                if (x.rem_euclid(2) + y.rem_euclid(2)) % 2 != 0 {
                    self.ctx.fill_rect(x as f64 * tile_size + offset_x, y as f64 * tile_size + offset_y, tile_size, tile_size);
                }
            }
        }

        // Grid Lines
        self.ctx.set_stroke_style_str("#cbd5e1");
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        
        for x in start_x..=end_x {
            self.ctx.move_to(x as f64 * tile_size + offset_x, start_y as f64 * tile_size + offset_y);
            self.ctx.line_to(x as f64 * tile_size + offset_x, end_y as f64 * tile_size + offset_y);
        }
        for y in start_y..=end_y {
            self.ctx.move_to(start_x as f64 * tile_size + offset_x, y as f64 * tile_size + offset_y);
            self.ctx.line_to(end_x as f64 * tile_size + offset_x, y as f64 * tile_size + offset_y);
        }
        self.ctx.stroke();

        // Draw Board Border (Above Grid)
        self.ctx.set_stroke_style_str("#1e293b");
        self.ctx.set_line_width(2.0);
        self.ctx.stroke_rect(board_left, board_top, board_dim, board_dim);

        // Shops
        for shop in &state.shops {
            if (shop.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.ctx.set_fill_style_str("#fde047");
                self.ctx.fill_rect(shop.position.x as f64 * tile_size + offset_x + 5.0 * zoom, shop.position.y as f64 * tile_size + offset_y + 5.0 * zoom, tile_size - 10.0 * zoom, tile_size - 10.0 * zoom);
            }
        }

        // Highlights for valid moves
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid)
            && let Some(config) = self.piece_configs.get(&piece.piece_type) {
            self.ctx.set_fill_style_str("rgba(34, 197, 94, 0.2)");
            let range = 10;
            for x in (piece.position.x - range)..(piece.position.x + range + 1) {
                for y in (piece.position.y - range)..(piece.position.y + range + 1) {
                    let t = IVec2::new(x, y);
                    if !is_within_board(t, state.board_size) { continue; }
                    
                    let target_piece = state.pieces.values().find(|p| p.position == t);
                    let is_friendly = target_piece.map(|tp| tp.owner_id == Some(player_id)).unwrap_or(false);
                    if is_friendly { continue; }
                    
                    let is_capture = target_piece.is_some();
                    if is_valid_move(config, piece.position, t, is_capture, state.board_size, &state.pieces, piece.owner_id) {
                        self.ctx.fill_rect(x as f64 * tile_size + offset_x + 2.0, y as f64 * tile_size + offset_y + 2.0, tile_size - 4.0, tile_size - 4.0);
                    }
                }
            }
        }

        // Pmove lines
        self.ctx.set_stroke_style_str("rgba(59, 130, 246, 0.5)");
        self.ctx.set_line_width(2.0);
        for pm in pm_queue {
            if let Some(real_p) = state.pieces.get(&pm.piece_id) {
                let mut start_pos = real_p.position;
                for prev in pm_queue {
                    if prev == pm { break; }
                    if prev.piece_id == pm.piece_id { start_pos = prev.target; }
                }
                self.ctx.begin_path();
                self.ctx.move_to(start_pos.x as f64 * tile_size + offset_x + tile_size/2.0, start_pos.y as f64 * tile_size + offset_y + tile_size/2.0);
                self.ctx.line_to(pm.target.x as f64 * tile_size + offset_x + tile_size/2.0, pm.target.y as f64 * tile_size + offset_y + tile_size/2.0);
                self.ctx.stroke();
            }
        }

        // Real pieces
        for piece in state.pieces.values() {
            if (piece.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.draw_piece(PieceDrawParams {
                    piece, player_id, offset_x, offset_y, alpha: 1.0, state, draw_name: false
                }, zoom);
            }
        }

        // Ghosts
        for (id, ghost) in ghost_pieces {
            if let Some(real) = state.pieces.get(id)
                && real.position != ghost.position {
                self.draw_piece(PieceDrawParams {
                    piece: ghost, player_id, offset_x, offset_y, alpha: 0.4, state, draw_name: false
                }, zoom);
            }
        }

        // Second pass: Draw player names on top of everything
        for piece in state.pieces.values() {
            if piece.piece_type == "king" && (piece.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                self.draw_piece_name(piece, offset_x, offset_y, 1.0, state, zoom);
            }
        }

        // Fog of War Overlay
        if player_id != Uuid::nil() && has_king {
            let king_screen_x = king_pos.x as f64 * tile_size + offset_x + tile_size / 2.0;
            let king_screen_y = king_pos.y as f64 * tile_size + offset_y + tile_size / 2.0;

            let gradient = self.ctx.create_radial_gradient(
                king_screen_x, king_screen_y, view_radius_px * 0.6,
                king_screen_x, king_screen_y, view_radius_px
            ).unwrap();
            
            let _ = gradient.add_color_stop(0.0, "rgba(255, 255, 255, 0.0)");
            let _ = gradient.add_color_stop(1.0, "rgba(255, 255, 255, 1.0)");

            self.ctx.set_fill_style_canvas_gradient(&gradient);
            self.ctx.fill_rect(0.0, 0.0, width, height);
        }
    }
}
