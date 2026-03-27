//! Piece drawing routines for the canvas renderer.

use crate::canvas::color::{PieceIconColors, piece_icon_colors};
use crate::canvas::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgKey, Renderer};
use crate::math::{Vec2, vec2};
use common::types::PieceTypeId;
use gloo_net::http::Request;
use js_sys;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlImageElement;

impl Renderer {
    /// Draws a piece glyph, cooldown bar, and optional nameplate.
    ///
    /// `params` describes the piece and draw state, `zoom` is the current zoom factor.
    /// Returns nothing.
    pub fn draw_piece(&self, params: PieceDrawParams, zoom: f64) {
        let tile_size = params.tile_size_px * zoom;
        let base_color = self.owner_color(&params);
        let colors = piece_icon_colors(&base_color);
        let config = self.piece_configs.get(&params.piece.piece_type);

        let svg_image = config.and_then(|cfg| {
            self.resolve_piece_icon(&params.piece.piece_type, &cfg.svg_path, &colors)
        });
        let svg_ready = svg_image
            .as_ref()
            .filter(|img| img.complete() && img.natural_width() > 0);

        // Prefer SVGs when available; fall back to a simple circle when missing.
        if let Some(image) = svg_ready {
            self.draw_piece_icon(image, params, tile_size);
        } else {
            self.draw_piece_fallback(&base_color, params, zoom, tile_size);
        }

        // Only draw cooldown on the real (non-ghost) piece for the owner.
        if self.should_draw_cooldown(&params) {
            self.draw_cooldown_bar(params, tile_size, zoom);
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

    fn owner_color(&self, params: &PieceDrawParams<'_>) -> String {
        params
            .piece
            .owner_id
            .and_then(|owner_id| {
                params
                    .state
                    .players
                    .get(&owner_id)
                    .map(|player| player.color.as_ref().to_string())
            })
            .unwrap_or_else(|| "#555555".to_string())
    }

    fn draw_piece_fallback(
        &self,
        base_color: &str,
        params: PieceDrawParams<'_>,
        zoom: f64,
        tile_size: f64,
    ) {
        let pos = piece_pos(params);
        self.ctx.set_fill_style_str(base_color);
        self.ctx.set_global_alpha(params.alpha);

        self.ctx.begin_path();
        let _ = self.ctx.arc(
            pos.x * tile_size + params.offset_x + tile_size / 2.0,
            pos.y * tile_size + params.offset_y + tile_size / 2.0,
            tile_size / 3.0,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.fill();

        self.ctx.set_global_alpha(1.0);

        self.ctx
            .set_fill_style_str(&format!("rgba(255, 255, 255, {})", params.alpha));
        let font_size = 16.0 * zoom;
        self.ctx.set_font(&format!("bold {}px Arbutus", font_size));
        let label = "?";
        let _ = self.ctx.fill_text(
            label,
            pos.x * tile_size + params.offset_x + tile_size / 2.0 - (5.0 * zoom),
            pos.y * tile_size + params.offset_y + tile_size / 2.0 + (6.0 * zoom),
        );
    }

    fn should_draw_cooldown(&self, params: &PieceDrawParams<'_>) -> bool {
        params.piece.owner_id == Some(params.player_id) && !params.is_ghost
    }

    fn draw_cooldown_bar(&self, params: PieceDrawParams<'_>, tile_size: f64, zoom: f64) {
        let pos = piece_pos(params);
        #[cfg(target_arch = "wasm32")]
        let now = common::types::TimestampMs::from_millis(js_sys::Date::now() as i64);
        #[cfg(not(target_arch = "wasm32"))]
        let now = common::types::TimestampMs::from_millis(chrono::Utc::now().timestamp_millis());

        let elapsed = now - params.piece.last_move_time;
        if elapsed >= params.piece.cooldown_ms {
            return;
        }

        let progress = elapsed.as_i64() as f64 / params.piece.cooldown_ms.as_i64() as f64;
        self.ctx.set_fill_style_str("#000");
        let bar_h = 4.0 * zoom;
        let bar_margin = 5.0 * zoom;
        let bar_y_offset = tile_size - (8.0 * zoom);

        self.ctx.fill_rect(
            pos.x * tile_size + params.offset_x + bar_margin,
            pos.y * tile_size + params.offset_y + bar_y_offset,
            tile_size - (bar_margin * 2.0),
            bar_h,
        );
        self.ctx.set_fill_style_str("#0f0");
        self.ctx.fill_rect(
            pos.x * tile_size + params.offset_x + bar_margin,
            pos.y * tile_size + params.offset_y + bar_y_offset,
            (tile_size - (bar_margin * 2.0)) * progress,
            bar_h,
        );
    }

    fn draw_piece_icon(&self, image: &HtmlImageElement, params: PieceDrawParams, tile_size: f64) {
        let icon_size = tile_size * 0.85;
        let pos = piece_pos(params);
        let draw_x = pos.x * tile_size + params.offset_x + (tile_size - icon_size) / 2.0;
        let draw_y = pos.y * tile_size + params.offset_y + (tile_size - icon_size) / 2.0;

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_image_smoothing_enabled(false);
        let _ = self.ctx.draw_image_with_html_image_element_and_dw_and_dh(
            image, draw_x, draw_y, icon_size, icon_size,
        );
        self.ctx.set_global_alpha(1.0);
    }

    fn resolve_piece_icon(
        &self,
        piece_type: &PieceTypeId,
        svg_path: &str,
        colors: &PieceIconColors,
    ) -> Option<HtmlImageElement> {
        if svg_path.trim().is_empty() {
            return None;
        }
        let template = self.ensure_svg_template(piece_type, svg_path)?;
        let key = PieceSvgKey {
            piece_type: piece_type.clone(),
            primary: colors.primary.clone(),
            secondary: colors.secondary.clone(),
            tertiary: colors.tertiary.clone(),
        };

        // Cache is keyed by piece type + derived team colors to avoid regenerating SVGs.
        let mut cache = self.svg_cache.borrow_mut();
        if let Some(image) = cache.images.get(&key) {
            return Some(image.clone());
        }

        let tinted = template
            .replace("${primary}", &colors.primary)
            .replace("${secondary}", &colors.secondary)
            .replace("${tertiary}", &colors.tertiary);
        let data_url = svg_data_url(&tinted);
        let image = HtmlImageElement::new().ok()?;
        image.set_src(&data_url);
        cache.images.insert(key, image.clone());
        Some(image)
    }

    fn ensure_svg_template(&self, piece_type: &PieceTypeId, svg_path: &str) -> Option<String> {
        {
            let cache = self.svg_cache.borrow();
            if let Some(template) = cache.templates.get(piece_type) {
                return Some(template.clone());
            }
            if cache.pending.contains(piece_type) {
                return None;
            }
        }

        let cache = self.svg_cache.clone();
        let piece_type = piece_type.clone();
        let svg_path = format!("/assets/pieces/{svg_path}");
        {
            let mut cache = cache.borrow_mut();
            cache.pending.insert(piece_type.clone());
        }

        // Fetch the SVG template asynchronously to avoid blocking the render loop.
        spawn_local(async move {
            let response = Request::get(&svg_path).send().await;
            let text = if let Ok(response) = response {
                response.text().await.ok()
            } else {
                None
            };
            let mut cache = cache.borrow_mut();
            cache.pending.remove(&piece_type);
            if let Some(text) = text {
                cache.templates.insert(piece_type, text);
            }
        });

        None
    }

    /// Draws a king nameplate above the piece if applicable.
    ///
    /// `params` describes the piece and draw state. Returns nothing.
    pub fn draw_piece_name(&self, params: PieceNameDrawParams<'_>) {
        let tile_size = params.tile_size_px * params.zoom;
        let pos = params.pos_override.unwrap_or_else(|| {
            vec2(params.piece.position.x as f64, params.piece.position.y as f64)
        });
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
                let x = pos.x * tile_size + params.offset_x + tile_size / 2.0 - text_width / 2.0;
                let y = pos.y * tile_size + params.offset_y - (5.0 * params.zoom);

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

fn svg_data_url(svg: &str) -> String {
    let encoded = js_sys::encode_uri_component(svg);
    let encoded = encoded.as_string().unwrap_or_default();
    format!("data:image/svg+xml;utf8,{encoded}")
}

fn piece_pos(params: PieceDrawParams<'_>) -> Vec2 {
    params.pos_override.unwrap_or_else(|| {
        vec2(params.piece.position.x as f64, params.piece.position.y as f64)
    })
}
