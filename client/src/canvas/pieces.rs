//! Piece drawing routines for the canvas renderer.

use crate::canvas::types::{PieceDrawParams, PieceNameDrawParams, Renderer};

impl Renderer {
    /// Draws a piece glyph, cooldown bar, and optional nameplate.
    ///
    /// `params` describes the piece and draw state, `zoom` is the current zoom factor.
    /// Returns nothing.
    pub fn draw_piece(&self, params: PieceDrawParams, zoom: f64) {
        let tile_size = params.tile_size_px * zoom;
        let is_self = params.piece.owner_id == Some(params.player_id);
        let pos = params.pos_override.unwrap_or((
            params.piece.position.x as f64,
            params.piece.position.y as f64,
        ));

        let color_str = if let Some(owner_id) = params.piece.owner_id {
            if let Some(player) = params.state.players.get(&owner_id) {
                player.color.as_ref().to_string()
            } else {
                "#555555".to_string()
            }
        } else {
            "#555555".to_string()
        };

        self.ctx.set_fill_style_str(&color_str);
        self.ctx.set_global_alpha(params.alpha);

        self.ctx.begin_path();
        let _ = self.ctx.arc(
            pos.0 * tile_size + params.offset_x + tile_size / 2.0,
            pos.1 * tile_size + params.offset_y + tile_size / 2.0,
            tile_size / 3.0,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.fill();

        // Reset alpha for text
        self.ctx.set_global_alpha(1.0);

        self.ctx
            .set_fill_style_str(&format!("rgba(255, 255, 255, {})", params.alpha));
        let font_size = 16.0 * zoom;
        self.ctx.set_font(&format!("bold {}px Arbutus", font_size));

        let config = self.piece_configs.get(&params.piece.piece_type);
        let label = config
            .map(|c| c.char.to_string())
            .unwrap_or_else(|| "?".to_string());

        let _ = self.ctx.fill_text(
            &label,
            pos.0 * tile_size + params.offset_x + tile_size / 2.0 - (5.0 * zoom),
            pos.1 * tile_size + params.offset_y + tile_size / 2.0 + (6.0 * zoom),
        );

        // Only draw cooldown on the real (non-ghost) piece for the owner
        if is_self && !params.is_ghost {
            #[cfg(target_arch = "wasm32")]
            let now = common::types::TimestampMs::from_millis(js_sys::Date::now() as i64);
            #[cfg(not(target_arch = "wasm32"))]
            let now =
                common::types::TimestampMs::from_millis(chrono::Utc::now().timestamp_millis());

            let elapsed = now - params.piece.last_move_time;
            if elapsed < params.piece.cooldown_ms {
                let progress = elapsed.as_i64() as f64 / params.piece.cooldown_ms.as_i64() as f64;
                self.ctx.set_fill_style_str("#000");
                let bar_h = 4.0 * zoom;
                let bar_margin = 5.0 * zoom;
                let bar_y_offset = tile_size - (8.0 * zoom);

                self.ctx.fill_rect(
                    pos.0 * tile_size + params.offset_x + bar_margin,
                    pos.1 * tile_size + params.offset_y + bar_y_offset,
                    tile_size - (bar_margin * 2.0),
                    bar_h,
                );
                self.ctx.set_fill_style_str("#0f0");
                self.ctx.fill_rect(
                    pos.0 * tile_size + params.offset_x + bar_margin,
                    pos.1 * tile_size + params.offset_y + bar_y_offset,
                    (tile_size - (bar_margin * 2.0)) * progress,
                    bar_h,
                );
            }
        }

        if params.alpha >= 1.0 && params.draw_name {
            self.draw_piece_name(PieceNameDrawParams {
                piece: params.piece,
                offset_x: params.offset_x,
                offset_y: params.offset_y,
                alpha: params.alpha,
                state: params.state,
                zoom,
                tile_size_px: params.tile_size_px,
                pos_override: params.pos_override,
            });
        }
    }

    /// Draws a king nameplate above the piece if applicable.
    ///
    /// `params` describes the piece and draw state. Returns nothing.
    pub fn draw_piece_name(&self, params: PieceNameDrawParams<'_>) {
        let tile_size = params.tile_size_px * params.zoom;
        let pos = params.pos_override.unwrap_or((
            params.piece.position.x as f64,
            params.piece.position.y as f64,
        ));
        if params.piece.piece_type.is_king()
            && let Some(owner_id) = params.piece.owner_id
            && let Some(player) = params.state.players.get(&owner_id)
        {
            let name = if player.name.trim().is_empty() {
                "An Unnamed Player"
            } else {
                &player.name
            };

            let name_font_size = 12.0 * params.zoom;
            self.ctx.set_font(&format!("{}px Arbutus", name_font_size));

            if let Ok(text_metrics) = self.ctx.measure_text(name) {
                let text_width = text_metrics.width();
                let x = pos.0 * tile_size + params.offset_x + tile_size / 2.0 - text_width / 2.0;
                let y = pos.1 * tile_size + params.offset_y - (5.0 * params.zoom);

                // Draw outline
                self.ctx
                    .set_stroke_style_str(&format!("rgba(255, 255, 255, {})", params.alpha));
                self.ctx.set_line_width(3.0 * params.zoom);
                let _ = self.ctx.stroke_text(name, x, y);

                // Draw fill
                self.ctx.set_fill_style_str(player.color.as_ref());
                self.ctx.set_global_alpha(params.alpha);
                let _ = self.ctx.fill_text(name, x, y);
                self.ctx.set_global_alpha(1.0);
            }
        }
    }
}
