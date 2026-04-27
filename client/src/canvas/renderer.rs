//! Canvas rendering implementation for the board and pieces.

use super::color::hex_to_rgba;
use super::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgCache, RenderParams, Renderer};
use crate::math::Vec2;
use common::logic::{evaluate_expression, is_valid_move, is_within_board};
use common::models::PieceConfig;
use common::types::{PieceTypeId, PlayerId};
use glam::IVec2;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
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
            svg_cache: Rc::new(RefCell::new(PieceSvgCache::new())),
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
            pmove_lines,
            ghost_pieces,
            animated_positions,
            camera_pos,
            canvas_size,
            zoom,
            tile_size_px,
            mode,
            board_rotated_180,
            shop_configs,
            active_shop_highlight_pos,
            disable_fog_of_war,
            clock_offset_ms,
        } = params;
        let width = canvas_size.x;
        let height = canvas_size.y;
        let tile_size = tile_size_px * zoom;
        let player_king = state
            .players
            .get(&player_id)
            .and_then(|p| state.pieces.get(&p.king_id));

        let has_king = player_king.is_some();
        let king_pos = player_king
            .map(|k| k.position)
            .unwrap_or(common::types::BoardCoord(IVec2::ZERO));
        let piece_count = state
            .pieces
            .values()
            .filter(|p| p.owner_id == Some(player_id))
            .count();

        let fog_of_war_radius = if let Some(m) = mode {
            let mut vars = HashMap::new();
            vars.insert("player_piece_count".to_string(), piece_count as f64);
            m.fog_of_war_radius
                .as_ref()
                .map(|expr| evaluate_expression(expr, &vars))
        } else {
            let zoom_factor = (piece_count as f64).sqrt().max(1.0);
            Some(15.0 * zoom_factor)
        };
        let fog_is_disabled = disable_fog_of_war || fog_of_war_radius.is_none();

        let view_radius_squares = if player_id == PlayerId::nil() || !has_king || fog_is_disabled {
            state.board_size.as_i32()
        } else {
            fog_of_war_radius.unwrap_or(0.0).max(0.0) as i32
        };
        let view_radius_px = (view_radius_squares as f64 + 0.5) * tile_size;

        // Background (Off-board color)
        self.ctx.set_fill_style_str("#e2e8f0");
        self.ctx.fill_rect(0.0, 0.0, width, height);

        // Offset mapping world -> screen
        let canvas_center = canvas_size * 0.5;
        let offset = if board_rotated_180 {
            canvas_center + camera_pos
        } else {
            canvas_center - camera_pos
        };
        let offset_x = offset.x;
        let offset_y = offset.y;
        let map_grid = |g: IVec2| {
            if board_rotated_180 {
                IVec2::new(-g.x - 1, -g.y - 1)
            } else {
                g
            }
        };
        let map_grid_f = |p: Vec2| {
            if board_rotated_180 {
                -p - Vec2::new(1.0, 1.0)
            } else {
                p
            }
        };
        let map_line = |v: i32| {
            if board_rotated_180 { -v } else { v }
        };

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

        let start_x = -half;
        let end_x = limit_pos;
        let start_y = -half;
        let end_y = limit_pos;

        // Checkerboard
        self.ctx.set_fill_style_str("#f1f5f9");
        for x in start_x..end_x {
            for y in start_y..end_y {
                // Proper checkerboard for centered system
                if (x.rem_euclid(2) + y.rem_euclid(2)) % 2 != 0 {
                    let mapped = map_grid(IVec2::new(x, y));
                    self.ctx.fill_rect(
                        mapped.x as f64 * tile_size + offset_x,
                        mapped.y as f64 * tile_size + offset_y,
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
            let mapped_x = map_line(x);
            let mapped_start_y = map_line(start_y);
            let mapped_end_y = map_line(end_y);
            self.ctx.move_to(
                mapped_x as f64 * tile_size + offset_x,
                mapped_start_y as f64 * tile_size + offset_y,
            );
            self.ctx.line_to(
                mapped_x as f64 * tile_size + offset_x,
                mapped_end_y as f64 * tile_size + offset_y,
            );
        }
        for y in start_y..=end_y {
            let mapped_y = map_line(y);
            let mapped_start_x = map_line(start_x);
            let mapped_end_x = map_line(end_x);
            self.ctx.move_to(
                mapped_start_x as f64 * tile_size + offset_x,
                mapped_y as f64 * tile_size + offset_y,
            );
            self.ctx.line_to(
                mapped_end_x as f64 * tile_size + offset_x,
                mapped_y as f64 * tile_size + offset_y,
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
            if (shop.position.0 - king_pos.0).abs().max_element() <= view_radius_squares + 2 {
                let color = shop_configs
                    .get(&shop.shop_id)
                    .and_then(|c| c.color.as_ref())
                    .map(|s| s.as_ref())
                    .unwrap_or("#fde047");
                let mapped = map_grid(shop.position.0);
                self.ctx.set_fill_style_str(color);
                self.ctx.fill_rect(
                    mapped.x as f64 * tile_size + offset_x + 5.0 * zoom,
                    mapped.y as f64 * tile_size + offset_y + 5.0 * zoom,
                    tile_size - 10.0 * zoom,
                    tile_size - 10.0 * zoom,
                );
            }
        }

        if let Some(highlight_pos) = active_shop_highlight_pos
            && (highlight_pos.0 - king_pos.0).abs().max_element() <= view_radius_squares + 2
        {
            let color = state
                .shops
                .iter()
                .find(|shop| shop.position == highlight_pos)
                .and_then(|shop| shop_configs.get(&shop.shop_id))
                .and_then(|c| c.focus_color.as_ref().or(c.color.as_ref()))
                .map(|s| s.as_ref())
                .unwrap_or("rgba(253, 224, 71, 0.35)");
            let mapped = map_grid(highlight_pos.0);
            self.ctx.set_fill_style_str(color);
            self.ctx.fill_rect(
                mapped.x as f64 * tile_size + offset_x + 2.0,
                mapped.y as f64 * tile_size + offset_y + 2.0,
                tile_size - 4.0,
                tile_size - 4.0,
            );
        }

        // Selected piece highlight
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid)
            && (piece.position.0 - king_pos.0).abs().max_element() <= view_radius_squares + 2
        {
            let highlight = if let Some(owner_id) = piece.owner_id {
                if let Some(player) = state.players.get(&owner_id) {
                    hex_to_rgba(player.color.as_ref(), 0.2)
                } else {
                    "rgba(59, 130, 246, 0.2)".to_string()
                }
            } else {
                "rgba(59, 130, 246, 0.2)".to_string()
            };
            let mapped = map_grid(piece.position.0);
            self.ctx.set_fill_style_str(&highlight);
            self.ctx.fill_rect(
                mapped.x as f64 * tile_size + offset_x + 2.0,
                mapped.y as f64 * tile_size + offset_y + 2.0,
                tile_size - 4.0,
                tile_size - 4.0,
            );
        }

        // Highlights for valid moves
        if let Some(sid) = selected_piece_id
            && let Some(piece) = ghost_pieces.get(&sid)
            && let Some(config) = self.piece_configs.get(&piece.piece_type)
        {
            self.ctx.set_fill_style_str("rgba(34, 197, 94, 0.2)");
            let range = 10;
            for x in (piece.position.0.x - range)..(piece.position.0.x + range + 1) {
                for y in (piece.position.0.y - range)..(piece.position.0.y + range + 1) {
                    let t = IVec2::new(x, y);
                    if !is_within_board(common::types::BoardCoord(t), state.board_size) {
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
                        end: common::types::BoardCoord(t),
                        is_capture,
                        board_size: state.board_size,
                        pieces: ghost_pieces,
                        moving_owner: piece.owner_id,
                    }) {
                        let mapped = map_grid(t);
                        self.ctx.fill_rect(
                            mapped.x as f64 * tile_size + offset_x + 2.0,
                            mapped.y as f64 * tile_size + offset_y + 2.0,
                            tile_size - 4.0,
                            tile_size - 4.0,
                        );
                    }
                }
            }
        }

        // Premove lines (already validated against projected ghost state)
        for line in pmove_lines {
            if let Some(real_p) = state.pieces.get(&line.piece_id) {
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

                let mapped_start = map_grid(line.start.0);
                let mapped_target = map_grid(line.target.0);
                self.ctx.begin_path();
                self.ctx.move_to(
                    mapped_start.x as f64 * tile_size + offset_x + tile_size / 2.0,
                    mapped_start.y as f64 * tile_size + offset_y + tile_size / 2.0,
                );
                self.ctx.line_to(
                    mapped_target.x as f64 * tile_size + offset_x + tile_size / 2.0,
                    mapped_target.y as f64 * tile_size + offset_y + tile_size / 2.0,
                );
                self.ctx.stroke();
            }
        }

        // Pieces
        for (id, ghost) in ghost_pieces {
            if (ghost.position.0 - king_pos.0).abs().max_element() <= view_radius_squares + 2 {
                let static_pos = map_grid_f(Vec2::new(
                    ghost.position.0.x as f64,
                    ghost.position.0.y as f64,
                ));
                let pos_override = animated_positions.get(id).copied().map(map_grid_f);
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
                                pos_override: Some(static_pos),
                                tile_size_px,
                                clock_offset_ms,
                            },
                            zoom,
                        );

                        // Draw real (server) piece solid
                        let real_static_pos = map_grid_f(Vec2::new(
                            real.position.0.x as f64,
                            real.position.0.y as f64,
                        ));
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
                                pos_override: pos_override.or(Some(real_static_pos)),
                                tile_size_px,
                                clock_offset_ms,
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
                                pos_override: pos_override.or(Some(static_pos)),
                                tile_size_px,
                                clock_offset_ms,
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
                && (piece.position.0 - king_pos.0).abs().max_element() <= view_radius_squares + 2
            {
                let static_pos = map_grid_f(Vec2::new(
                    piece.position.0.x as f64,
                    piece.position.0.y as f64,
                ));
                let pos_override = animated_positions
                    .get(id)
                    .copied()
                    .map(map_grid_f)
                    .or(Some(static_pos));
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
        if !fog_is_disabled && player_id != PlayerId::nil() && has_king {
            let mapped_king = map_grid(king_pos.0);
            let king_screen_x = mapped_king.x as f64 * tile_size + offset_x + tile_size / 2.0;
            let king_screen_y = mapped_king.y as f64 * tile_size + offset_y + tile_size / 2.0;

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
