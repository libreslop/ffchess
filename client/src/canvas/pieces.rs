use crate::canvas::types::{PieceDrawParams, Renderer};
use common::*;
use wasm_bindgen::JsValue;

impl Renderer {
    pub fn draw_piece(&self, params: PieceDrawParams) {
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
        let _ = self.ctx.arc(
            params.piece.position.x as f64 * self.tile_size
                + params.offset_x
                + self.tile_size / 2.0,
            params.piece.position.y as f64 * self.tile_size
                + params.offset_y
                + self.tile_size / 2.0,
            self.tile_size / 3.0,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.fill();

        self.ctx.set_fill_style(&JsValue::from_str(&format!(
            "rgba(255, 255, 255, {})",
            params.alpha
        )));
        let font_size = 16.0 * self.zoom;
        self.ctx.set_font(&format!("bold {}px Arbutus", font_size));
        let label = match params.piece.piece_type {
            PieceType::King => "K",
            PieceType::Queen => "Q",
            PieceType::Rook => "R",
            PieceType::Bishop => "B",
            PieceType::Knight => "N",
            PieceType::Pawn => "P",
        };
        let _ = self.ctx.fill_text(
            label,
            params.piece.position.x as f64 * self.tile_size
                + params.offset_x
                + self.tile_size / 2.0
                - (5.0 * self.zoom),
            params.piece.position.y as f64 * self.tile_size
                + params.offset_y
                + self.tile_size / 2.0
                + (6.0 * self.zoom),
        );

        if params.alpha >= 1.0 && params.piece.owner_id == Some(params.player_id) {
            #[cfg(target_arch = "wasm32")]
            let now = js_sys::Date::now() as i64;
            #[cfg(not(target_arch = "wasm32"))]
            let now = chrono::Utc::now().timestamp_millis();

            let elapsed = now - params.piece.last_move_time;
            if elapsed < params.piece.cooldown_ms {
                let progress = elapsed as f64 / params.piece.cooldown_ms as f64;
                self.ctx.set_fill_style(&JsValue::from_str("#000"));
                let bar_h = 4.0 * self.zoom;
                let bar_margin = 5.0 * self.zoom;
                let bar_y_offset = self.tile_size - (8.0 * self.zoom);

                self.ctx.fill_rect(
                    params.piece.position.x as f64 * self.tile_size + params.offset_x + bar_margin,
                    params.piece.position.y as f64 * self.tile_size
                        + params.offset_y
                        + bar_y_offset,
                    self.tile_size - (bar_margin * 2.0),
                    bar_h,
                );
                self.ctx.set_fill_style(&JsValue::from_str("#0f0"));
                self.ctx.fill_rect(
                    params.piece.position.x as f64 * self.tile_size + params.offset_x + bar_margin,
                    params.piece.position.y as f64 * self.tile_size
                        + params.offset_y
                        + bar_y_offset,
                    (self.tile_size - (bar_margin * 2.0)) * progress,
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
    ) {
        if piece.piece_type == PieceType::King
            && let Some(owner_id) = piece.owner_id
            && let Some(player) = state.players.get(&owner_id)
        {
            let name = if player.name.trim().is_empty() {
                "An Unnamed Player"
            } else {
                &player.name
            };

            let name_font_size = 12.0 * self.zoom;
            self.ctx.set_font(&format!("{}px Arbutus", name_font_size));

            if let Ok(text_metrics) = self.ctx.measure_text(name) {
                let text_width = text_metrics.width();
                let x = piece.position.x as f64 * self.tile_size + offset_x + self.tile_size / 2.0
                    - text_width / 2.0;
                let y = piece.position.y as f64 * self.tile_size + offset_y - (5.0 * self.zoom);

                // Draw outline
                self.ctx.set_stroke_style(&JsValue::from_str(&format!(
                    "rgba(255, 255, 255, {})",
                    alpha
                )));
                self.ctx.set_line_width(3.0 * self.zoom);
                let _ = self.ctx.stroke_text(name, x, y);

                // Draw fill
                self.ctx.set_fill_style(&JsValue::from_str(&format!(
                    "rgba(0, 0, 0, {})",
                    0.7 * alpha
                )));
                let _ = self.ctx.fill_text(name, x, y);
            }
        }
    }
}
