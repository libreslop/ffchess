//! Piece drawing routines for the canvas renderer.

use crate::canvas::color::{PieceIconColors, piece_icon_colors};
use crate::canvas::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgKey, Renderer};
use crate::math::{Vec2, vec2};
use common::types::PieceTypeId;
use gloo_net::http::Request;
use js_sys;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

const PIECE_RASTER_SIZE_PX: u32 = 256;
const COOLDOWN_GAP_RATIO: f64 = 0.03;
const COOLDOWN_LINE_WIDTH_PX: f64 = 2.0;

/// Loaded SVG image plus its cache key.
struct PieceSvgHandle {
    key: PieceSvgKey,
    image: HtmlImageElement,
}

impl Renderer {
    /// Draws a piece glyph, cooldown indicator, and optional nameplate.
    ///
    /// `params` describes the piece and draw state, `zoom` is the current zoom factor.
    /// Returns nothing.
    pub fn draw_piece(&self, params: PieceDrawParams, zoom: f64) {
        let tile_size = params.tile_size_px * zoom;
        let base_color = self.owner_color(&params);
        let colors = piece_icon_colors(&base_color);
        let config = self.piece_configs.get(&params.piece.piece_type);

        let svg_handle = config.and_then(|cfg| {
            self.resolve_piece_icon(&params.piece.piece_type, &cfg.svg_path, &colors)
        });
        let svg_ready = svg_handle
            .as_ref()
            .filter(|handle| handle.image.complete() && handle.image.natural_width() > 0);

        // Prefer SVGs when available; fall back to a simple circle when missing.
        if let Some(handle) = svg_ready {
            self.draw_piece_icon(handle, params, tile_size, zoom);
        } else {
            self.draw_piece_fallback(&base_color, params, zoom, tile_size);
        }

        // Only draw cooldown on the real (non-ghost) piece for the owner.
        if self.should_draw_cooldown(&params) {
            self.draw_cooldown_bar(params, tile_size, zoom, &base_color);
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
        let pos = Self::piece_pos(params);
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

    fn draw_cooldown_bar(
        &self,
        params: PieceDrawParams<'_>,
        tile_size: f64,
        zoom: f64,
        team_color: &str,
    ) {
        let pos = Self::piece_pos(params);
        #[cfg(target_arch = "wasm32")]
        let now = common::types::TimestampMs::from_millis(js_sys::Date::now() as i64);
        #[cfg(not(target_arch = "wasm32"))]
        let now = common::types::TimestampMs::from_millis(chrono::Utc::now().timestamp_millis());

        let elapsed = now - params.piece.last_move_time;
        if elapsed >= params.piece.cooldown_ms {
            return;
        }

        let progress = elapsed.as_i64() as f64 / params.piece.cooldown_ms.as_i64() as f64;

        let line_width = Self::cooldown_line_width(zoom);
        let gap = Self::cooldown_gap(tile_size);
        let inset = (gap + (line_width / 2.0)).min(tile_size / 2.0 - 1.0);
        let square_size = (tile_size - (2.0 * inset)).max(line_width + 1.0).round();
        let square_x = Self::align_stroke_coord(
            pos.x * tile_size + params.offset_x + inset,
            line_width,
        );
        let square_y = Self::align_stroke_coord(
            pos.y * tile_size + params.offset_y + inset,
            line_width,
        );

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_line_width(line_width);
        self.ctx.set_line_cap("butt");
        self.ctx.set_line_join("miter");

        self.ctx.set_stroke_style_str(team_color);
        self.draw_square_progress(square_x, square_y, square_size, progress);

        self.ctx.set_global_alpha(1.0);
    }

    fn draw_piece_icon(
        &self,
        handle: &PieceSvgHandle,
        params: PieceDrawParams,
        tile_size: f64,
        zoom: f64,
    ) {
        let icon_size = Self::piece_icon_size(tile_size, zoom);
        let pos = Self::piece_pos(params);
        let draw_x = pos.x * tile_size + params.offset_x + (tile_size - icon_size) / 2.0;
        let draw_y = pos.y * tile_size + params.offset_y + (tile_size - icon_size) / 2.0;

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_image_smoothing_enabled(true);
        Self::set_image_smoothing_quality(&self.ctx, "high");

        if let Some(raster) = self.ensure_piece_raster(&handle.key, &handle.image) {
            let _ = self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
                &raster, draw_x, draw_y, icon_size, icon_size,
            );
        } else {
            let _ = self.ctx.draw_image_with_html_image_element_and_dw_and_dh(
                &handle.image, draw_x, draw_y, icon_size, icon_size,
            );
        }
        self.ctx.set_global_alpha(1.0);
    }

    fn resolve_piece_icon(
        &self,
        piece_type: &PieceTypeId,
        svg_path: &str,
        colors: &PieceIconColors,
    ) -> Option<PieceSvgHandle> {
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
            return Some(PieceSvgHandle {
                key,
                image: image.clone(),
            });
        }

        let tinted = template
            .replace("${primary}", &colors.primary)
            .replace("${secondary}", &colors.secondary)
            .replace("${tertiary}", &colors.tertiary);
        let tinted = Self::soften_svg_edges(&tinted);
        let data_url = Self::svg_data_url(&tinted);
        let image = HtmlImageElement::new().ok()?;
        image.set_src(&data_url);
        cache.images.insert(key.clone(), image.clone());
        Some(PieceSvgHandle { key, image })
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

    fn ensure_piece_raster(
        &self,
        key: &PieceSvgKey,
        image: &HtmlImageElement,
    ) -> Option<HtmlCanvasElement> {
        {
            let cache = self.svg_cache.borrow();
            if let Some(raster) = cache.rasters.get(key) {
                return Some(raster.clone());
            }
        }

        let window = web_sys::window()?;
        let document = window.document()?;
        let canvas = document
            .create_element("canvas")
            .ok()?
            .dyn_into::<HtmlCanvasElement>()
            .ok()?;
        canvas.set_width(PIECE_RASTER_SIZE_PX);
        canvas.set_height(PIECE_RASTER_SIZE_PX);

        let ctx = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<CanvasRenderingContext2d>()
            .ok()?;
        ctx.set_image_smoothing_enabled(true);
        Self::set_image_smoothing_quality(&ctx, "high");
        let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(
            image,
            0.0,
            0.0,
            PIECE_RASTER_SIZE_PX as f64,
            PIECE_RASTER_SIZE_PX as f64,
        );

        let mut cache = self.svg_cache.borrow_mut();
        cache.rasters.insert(key.clone(), canvas.clone());
        Some(canvas)
    }

    fn set_image_smoothing_quality(ctx: &CanvasRenderingContext2d, quality: &str) {
        let _ = js_sys::Reflect::set(
            ctx.as_ref(),
            &JsValue::from_str("imageSmoothingQuality"),
            &JsValue::from_str(quality),
        );
    }

    fn soften_svg_edges(svg: &str) -> String {
        if svg.contains("shape-rendering=\"crispEdges\"") {
            return svg.replace("shape-rendering=\"crispEdges\"", "shape-rendering=\"auto\"");
        }
        if svg.contains("shape-rendering='crispEdges'") {
            return svg.replace("shape-rendering='crispEdges'", "shape-rendering='auto'");
        }
        svg.to_string()
    }

    fn svg_data_url(svg: &str) -> String {
        let encoded = js_sys::encode_uri_component(svg);
        let encoded = encoded.as_string().unwrap_or_default();
        format!("data:image/svg+xml;utf8,{encoded}")
    }

    fn draw_square_progress(&self, x: f64, y: f64, size: f64, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= 0.0 {
            return;
        }

        let perimeter = 4.0 * size;
        let mut remaining = perimeter * progress;

        // Trace clockwise from 12 o'clock (top center) as a single path
        // to avoid visible segment seams at the corners.
        let half = size / 2.0;
        let mut cursor = vec2(x + half, y);
        self.ctx.begin_path();
        self.ctx.move_to(cursor.x, cursor.y);

        remaining = self.append_segment_path(&mut cursor, vec2(x + size, y), remaining);
        if remaining <= 0.0 {
            self.ctx.stroke();
            return;
        }
        remaining = self.append_segment_path(&mut cursor, vec2(x + size, y + size), remaining);
        if remaining <= 0.0 {
            self.ctx.stroke();
            return;
        }
        remaining = self.append_segment_path(&mut cursor, vec2(x, y + size), remaining);
        if remaining <= 0.0 {
            self.ctx.stroke();
            return;
        }
        remaining = self.append_segment_path(&mut cursor, vec2(x, y), remaining);
        if remaining <= 0.0 {
            self.ctx.stroke();
            return;
        }
        let _ = self.append_segment_path(&mut cursor, vec2(x + half, y), remaining);
        self.ctx.stroke();
    }

    fn append_segment_path(&self, cursor: &mut Vec2, target: Vec2, remaining: f64) -> f64 {
        if remaining <= 0.0 {
            return 0.0;
        }
        let dx = target.x - cursor.x;
        let dy = target.y - cursor.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        if seg_len <= 0.0 {
            return remaining;
        }
        let draw_len = remaining.min(seg_len);
        let t = draw_len / seg_len;
        let px = cursor.x + dx * t;
        let py = cursor.y + dy * t;
        self.ctx.line_to(px, py);
        *cursor = vec2(px, py);
        remaining - draw_len
    }

    fn align_stroke_coord(value: f64, line_width: f64) -> f64 {
        let width = line_width.round() as i64;
        if width % 2 == 0 {
            value.round()
        } else {
            value.round() + 0.5
        }
    }

    fn cooldown_gap(tile_size: f64) -> f64 {
        tile_size * COOLDOWN_GAP_RATIO
    }

    fn cooldown_line_width(zoom: f64) -> f64 {
        (COOLDOWN_LINE_WIDTH_PX * zoom).round().max(1.0)
    }

    fn piece_icon_size(tile_size: f64, zoom: f64) -> f64 {
        let gap = Self::cooldown_gap(tile_size);
        let line_width = Self::cooldown_line_width(zoom);
        let size = tile_size - (4.0 * gap) - (2.0 * line_width);
        size.max(line_width + 1.0).round()
    }

    fn piece_pos(params: PieceDrawParams<'_>) -> Vec2 {
        params.pos_override.unwrap_or_else(|| {
            vec2(params.piece.position.x as f64, params.piece.position.y as f64)
        })
    }
}
