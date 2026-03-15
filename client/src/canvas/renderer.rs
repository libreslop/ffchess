//! Canvas rendering implementation for the board and pieces.

use super::color::hex_to_rgba;
use super::types::{PieceDrawParams, PieceNameDrawParams, RenderParams, Renderer};
use common::logic::{evaluate_expression, is_valid_move, is_within_board};
use common::models::{PieceConfig, ShopConfig};
use common::types::{PieceTypeId, PlayerId, ShopId};
use glam::IVec2;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

impl Renderer {
    /// Creates a new renderer for a given canvas and config sets.
    ///
    /// `canvas` is the HTML canvas element, `piece_configs` and `shop_configs` define assets.
    /// Returns a ready-to-use `Renderer`.
    pub fn new(
        canvas: HtmlCanvasElement,
        piece_configs: HashMap<PieceTypeId, PieceConfig>,
        shop_configs: HashMap<ShopId, ShopConfig>,
    ) -> Self {
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
        Self {
            ctx,
            piece_configs,
            shop_configs,
        }
    }

    /// Renders a full frame including board, shops, pieces, and fog of war.
    ///
    /// `params` bundles render inputs for the frame. Returns nothing.
    pub fn draw_with_ghosts(&self, params: RenderParams<'_>) {
        let RenderParams {
            state,
            player_id,
            selected_piece_id,
            pm_queue,
            ghost_pieces,
            animated_positions,
            camera_pos,
            width,
            height,
            zoom,
            tile_size_px,
            mode,
            shop_configs,
        } = params;
        let tile_size = tile_size_px * zoom;
        let player_king = state
            .players
            .get(&player_id)
            .and_then(|p| state.pieces.get(&p.king_id));

        let has_king = player_king.is_some();
        let king_pos = player_king.map(|k| k.position).unwrap_or(IVec2::ZERO);
        let piece_count = state
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();

        let fog_of_war_radius = if let Some(m) = mode {
            let mut vars = HashMap::new();
            vars.insert("player_piece_count".to_string(), piece_count as f64);
            evaluate_expression(&m.fog_of_war_radius, &vars)
        } else {
            let zoom_factor = (piece_count as f64).sqrt().max(1.0);
            15.0 * zoom_factor
        };

        let view_radius_squares = if player_id == PlayerId::nil() || !has_king {
            100
        } else {
            fog_of_war_radius as i32
        };
        let view_radius_px = (view_radius_squares as f64 + 0.5) * tile_size;

        // Background (Off-board color)
        self.ctx.set_fill_style_str("#e2e8f0");
        self.ctx.fill_rect(0.0, 0.0, width, height);

        // Offset mapping world -> screen
        let offset_x = width / 2.0 - camera_pos.0;
        let offset_y = height / 2.0 - camera_pos.1;

        let half = state.board_size.half();
        let limit_pos = state.board_size.limit_pos();

        // Board Pixel Boundaries
        let board_left = -(half as f64) * tile_size + offset_x;
        let board_top = -(half as f64) * tile_size + offset_y;
        let board_dim = state.board_size.as_i32() as f64 * tile_size;

        // Draw Board Background
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx
            .fill_rect(board_left, board_top, board_dim, board_dim);

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
                    self.ctx.fill_rect(
                        x as f64 * tile_size + offset_x,
                        y as f64 * tile_size + offset_y,
                        tile_size,
                        tile_size,
                    );
                }
            }
        }

        // Grid Lines
        self.ctx.set_stroke_style_str("#cbd5e1");
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();

        for x in start_x..=end_x {
            self.ctx.move_to(
                x as f64 * tile_size + offset_x,
                start_y as f64 * tile_size + offset_y,
            );
            self.ctx.line_to(
                x as f64 * tile_size + offset_x,
                end_y as f64 * tile_size + offset_y,
            );
        }
        for y in start_y..=end_y {
            self.ctx.move_to(
                start_x as f64 * tile_size + offset_x,
                y as f64 * tile_size + offset_y,
            );
            self.ctx.line_to(
                end_x as f64 * tile_size + offset_x,
                y as f64 * tile_size + offset_y,
            );
        }
        self.ctx.stroke();

        // Draw Board Border (Above Grid)
        self.ctx.set_stroke_style_str("#1e293b");
        self.ctx.set_line_width(2.0);
        self.ctx
            .stroke_rect(board_left, board_top, board_dim, board_dim);

        // Shops
        for shop in &state.shops {
            if (shop.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                let color = shop_configs
                    .get(&shop.shop_id)
                    .and_then(|c| c.color.as_ref())
                    .map(|s| s.as_ref())
                    .unwrap_or("#fde047");
                self.ctx.set_fill_style_str(color);
                self.ctx.fill_rect(
                    shop.position.x as f64 * tile_size + offset_x + 5.0 * zoom,
                    shop.position.y as f64 * tile_size + offset_y + 5.0 * zoom,
                    tile_size - 10.0 * zoom,
                    tile_size - 10.0 * zoom,
                );
            }
        }

        // Highlights for valid moves
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid)
            && let Some(config) = self.piece_configs.get(&piece.piece_type)
        {
            self.ctx.set_fill_style_str("rgba(34, 197, 94, 0.2)");
            let range = 10;
            for x in (piece.position.x - range)..(piece.position.x + range + 1) {
                for y in (piece.position.y - range)..(piece.position.y + range + 1) {
                    let t = IVec2::new(x, y);
                    if !is_within_board(t, state.board_size) {
                        continue;
                    }

                    let target_piece = ghost_pieces.values().find(|p| p.position == t);
                    let is_friendly = target_piece
                        .map(|tp| tp.owner_id == Some(player_id))
                        .unwrap_or(false);
                    if is_friendly {
                        continue;
                    }

                    let is_capture = target_piece.is_some();
                    if is_valid_move(common::logic::MoveValidationParams {
                        piece_config: config,
                        start: piece.position,
                        end: t,
                        is_capture,
                        board_size: state.board_size,
                        pieces: ghost_pieces,
                        moving_owner: piece.owner_id,
                    }) {
                        self.ctx.fill_rect(
                            x as f64 * tile_size + offset_x + 2.0,
                            y as f64 * tile_size + offset_y + 2.0,
                            tile_size - 4.0,
                            tile_size - 4.0,
                        );
                    }
                }
            }
        }

        // Pmove lines
        for pm in pm_queue {
            if let Some(real_p) = state.pieces.get(&pm.piece_id) {
                let color = if let Some(owner_id) = real_p.owner_id {
                    if let Some(player) = state.players.get(&owner_id) {
                        hex_to_rgba(player.color.as_ref(), 0.5)
                    } else {
                        "rgba(59, 130, 246, 0.5)".to_string()
                    }
                } else {
                    "rgba(59, 130, 246, 0.5)".to_string()
                };

                self.ctx.set_stroke_style_str(&color);
                self.ctx.set_line_width(2.0);

                let mut start_pos = real_p.position;
                for prev in pm_queue {
                    if prev == pm {
                        break;
                    }
                    if prev.piece_id == pm.piece_id {
                        start_pos = prev.target;
                    }
                }
                self.ctx.begin_path();
                self.ctx.move_to(
                    start_pos.x as f64 * tile_size + offset_x + tile_size / 2.0,
                    start_pos.y as f64 * tile_size + offset_y + tile_size / 2.0,
                );
                self.ctx.line_to(
                    pm.target.x as f64 * tile_size + offset_x + tile_size / 2.0,
                    pm.target.y as f64 * tile_size + offset_y + tile_size / 2.0,
                );
                self.ctx.stroke();
            }
        }

        // Pieces
        for (id, ghost) in ghost_pieces {
            if (ghost.position - king_pos).abs().max_element() <= view_radius_squares + 2 {
                let pos_override = animated_positions.get(id).copied();
                if let Some(real) = state.pieces.get(id) {
                    if real.position != ghost.position {
                        // Draw ghost (predicted) piece faded
                        self.draw_piece(
                            PieceDrawParams {
                                piece: ghost,
                                player_id,
                                offset_x,
                                offset_y,
                                alpha: 0.4,
                                state,
                                draw_name: false,
                                is_ghost: true,
                                pos_override: None,
                                tile_size_px,
                            },
                            zoom,
                        );

                        // Draw real (server) piece solid
                        self.draw_piece(
                            PieceDrawParams {
                                piece: real,
                                player_id,
                                offset_x,
                                offset_y,
                                alpha: 1.0,
                                state,
                                draw_name: false,
                                is_ghost: false,
                                pos_override,
                                tile_size_px,
                            },
                            zoom,
                        );
                    } else {
                        // Piece is not moving or not ours
                        self.draw_piece(
                            PieceDrawParams {
                                piece: ghost,
                                player_id,
                                offset_x,
                                offset_y,
                                alpha: 1.0,
                                state,
                                draw_name: false,
                                is_ghost: false,
                                pos_override,
                                tile_size_px,
                            },
                            zoom,
                        );
                    }
                }
            }
        }

        // Second pass: Draw player names on top of everything
        for (id, ghost_piece) in ghost_pieces {
            let piece = state.pieces.get(id).unwrap_or(ghost_piece);
            if piece.piece_type.is_king()
                && (piece.position - king_pos).abs().max_element() <= view_radius_squares + 2
            {
                let pos_override = animated_positions.get(id).copied();
                self.draw_piece_name(PieceNameDrawParams {
                    piece,
                    offset_x,
                    offset_y,
                    alpha: 1.0,
                    state,
                    zoom,
                    tile_size_px,
                    pos_override,
                });
            }
        }

        // Fog of War Overlay
        if player_id != PlayerId::nil() && has_king {
            let king_screen_x = king_pos.x as f64 * tile_size + offset_x + tile_size / 2.0;
            let king_screen_y = king_pos.y as f64 * tile_size + offset_y + tile_size / 2.0;

            let gradient = self
                .ctx
                .create_radial_gradient(
                    king_screen_x,
                    king_screen_y,
                    view_radius_px * 0.6,
                    king_screen_x,
                    king_screen_y,
                    view_radius_px,
                )
                .unwrap();

            let _ = gradient.add_color_stop(0.0, "rgba(255, 255, 255, 0.0)");
            let _ = gradient.add_color_stop(1.0, "rgba(255, 255, 255, 1.0)");

            self.ctx.set_fill_style_canvas_gradient(&gradient);
            self.ctx.fill_rect(0.0, 0.0, width, height);
        }
    }
}
