use crate::canvas::types::{PieceDrawParams, Renderer};
use common::models::GameState;
use common::models::Piece;

impl Renderer {
    pub fn draw_piece(&self, params: PieceDrawParams, zoom: f64) {
        let tile_size = 40.0 * zoom;
        let is_self = params.piece.owner_id == Some(params.player_id);
        
        let color_str = if let Some(owner_id) = params.piece.owner_id {
            if let Some(player) = params.state.players.get(&owner_id) {
                player.color.clone()
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
            params.piece.position.x as f64 * tile_size
                + params.offset_x
                + tile_size / 2.0,
            params.piece.position.y as f64 * tile_size
                + params.offset_y
                + tile_size / 2.0,
            tile_size / 3.0,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.fill();
        
        // Reset alpha for text
        self.ctx.set_global_alpha(1.0);

        self.ctx.set_fill_style_str(&format!(
            "rgba(255, 255, 255, {})",
            params.alpha
        ));
        let font_size = 16.0 * zoom;
        self.ctx.set_font(&format!("bold {}px Arbutus", font_size));
        
        let config = self.piece_configs.get(&params.piece.piece_type);
        let label = config.map(|c| c.char.to_string()).unwrap_or_else(|| "?".to_string());
        
        let _ = self.ctx.fill_text(
            &label,
            params.piece.position.x as f64 * tile_size
                + params.offset_x
                + tile_size / 2.0
                - (5.0 * zoom),
            params.piece.position.y as f64 * tile_size
                + params.offset_y
                + tile_size / 2.0
                + (6.0 * zoom),
        );

        if params.alpha >= 1.0 && is_self {
            #[cfg(target_arch = "wasm32")]
            let now = js_sys::Date::now() as i64;
            #[cfg(not(target_arch = "wasm32"))]
            let now = chrono::Utc::now().timestamp_millis();

            let elapsed = now - params.piece.last_move_time;
            if elapsed < params.piece.cooldown_ms {
                let progress = elapsed as f64 / params.piece.cooldown_ms as f64;
                self.ctx.set_fill_style_str("#000");
                let bar_h = 4.0 * zoom;
                let bar_margin = 5.0 * zoom;
                let bar_y_offset = tile_size - (8.0 * zoom);

                self.ctx.fill_rect(
                    params.piece.position.x as f64 * tile_size + params.offset_x + bar_margin,
                    params.piece.position.y as f64 * tile_size
                        + params.offset_y
                        + bar_y_offset,
                    tile_size - (bar_margin * 2.0),
                    bar_h,
                );
                self.ctx.set_fill_style_str("#0f0");
                self.ctx.fill_rect(
                    params.piece.position.x as f64 * tile_size + params.offset_x + bar_margin,
                    params.piece.position.y as f64 * tile_size
                        + params.offset_y
                        + bar_y_offset,
                    (tile_size - (bar_margin * 2.0)) * progress,
                    bar_h,
                );
            }
        }

        if params.alpha >= 1.0 && params.draw_name {
            self.draw_piece_name(
                params.piece,
                params.offset_x,
                params.offset_y,
                params.alpha,
                params.state,
                zoom,
            );
        }
    }

    pub fn draw_piece_name(
        &self,
        piece: &Piece,
        offset_x: f64,
        offset_y: f64,
        alpha: f64,
        state: &GameState,
        zoom: f64,
    ) {
        let tile_size = 40.0 * zoom;
        if piece.piece_type == "king"
            && let Some(owner_id) = piece.owner_id
            && let Some(player) = state.players.get(&owner_id)
        {
            let name = if player.name.trim().is_empty() {
                "An Unnamed Player"
            } else {
                &player.name
            };

            let name_font_size = 12.0 * zoom;
            self.ctx.set_font(&format!("{}px Arbutus", name_font_size));

            if let Ok(text_metrics) = self.ctx.measure_text(name) {
                let text_width = text_metrics.width();
                let x = piece.position.x as f64 * tile_size + offset_x + tile_size / 2.0
                    - text_width / 2.0;
                let y = piece.position.y as f64 * tile_size + offset_y - (5.0 * zoom);

                // Draw outline
                self.ctx.set_stroke_style_str(&format!(
                    "rgba(255, 255, 255, {})",
                    alpha
                ));
                self.ctx.set_line_width(3.0 * zoom);
                let _ = self.ctx.stroke_text(name, x, y);

                // Draw fill
                self.ctx.set_fill_style_str(&player.color);
                self.ctx.set_global_alpha(alpha);
                let _ = self.ctx.fill_text(name, x, y);
                self.ctx.set_global_alpha(1.0);
            }
        }
    }
}
