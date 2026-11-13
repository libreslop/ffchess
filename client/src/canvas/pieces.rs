//! Piece drawing routines for the canvas renderer.

use crate::canvas::color::{NPC_PIECE_COLOR, PieceIconColors, piece_icon_colors};
use crate::canvas::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgKey, Renderer};
use crate::math::Vec2;
use common::types::{PieceTypeId, TimestampMs};
use gloo_net::http::Request;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

const PIECE_RASTER_SIZE_PX: u32 = 256;
const COOLDOWN_GAP_RATIO: f64 = 0.03;
const COOLDOWN_LINE_WIDTH_PX: f64 = 2.0;
/// Number of steps each side of the cooldown ring charges up in.
const COOLDOWN_PARTS_PER_SIDE: usize = 4;

/// Loaded SVG image plus its cache key.
struct PieceSvgHandle {
    key: PieceSvgKey,
    image: HtmlImageElement,
}

impl Renderer {
    /// Draws a piece glyph and, for the local player's real pieces, its cooldown indicator.
    pub(crate) fn draw_piece(&self, params: PieceDrawParams) {
        let base_color = self.owner_color(&params);
        let colors = piece_icon_colors(&base_color);
        let svg_handle = self
            .catalog
            .pieces
            .get(&params.piece.piece_type)
            .and_then(|config| {
                self.resolve_piece_icon(&params.piece.piece_type, &config.svg_path, &colors)
            });
        let svg_ready = svg_handle
            .as_ref()
            .filter(|handle| handle.image.complete() && handle.image.natural_width() > 0);

        // Prefer SVGs when available; fall back to a simple circle when missing.
        match svg_ready {
            Some(handle) => self.draw_piece_icon(handle, params),
            None => self.draw_piece_fallback(&base_color, params),
        }
        if self.should_draw_cooldown(&params) {
            self.draw_cooldown_bar(params, &base_color);
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
            .unwrap_or_else(|| NPC_PIECE_COLOR.to_string())
    }

    /// Draws a "?" disc for pieces whose icon is unavailable.
    fn draw_piece_fallback(&self, base_color: &str, params: PieceDrawParams<'_>) {
        let (tile, zoom) = (params.tile_size, params.zoom);
        let center = params.pos * tile + params.offset + tile / 2.0;
        self.ctx.set_fill_style_str(base_color);
        self.ctx.set_global_alpha(params.alpha);
        self.ctx.begin_path();
        let _ = self
            .ctx
            .arc(center.x, center.y, tile / 3.0, 0.0, std::f64::consts::TAU);
        self.ctx.fill();
        self.ctx.set_global_alpha(1.0);

        self.ctx
            .set_fill_style_str(&format!("rgba(255, 255, 255, {})", params.alpha));
        self.ctx
            .set_font(&format!("bold {}px Upheaval", 16.0 * zoom));
        let _ = self
            .ctx
            .fill_text("?", center.x - 5.0 * zoom, center.y + 6.0 * zoom);
    }

    fn should_draw_cooldown(&self, params: &PieceDrawParams<'_>) -> bool {
        params.piece.owner_id == Some(params.player_id) && !params.is_ghost
    }

    /// Traces the elapsed share of the piece cooldown around its square.
    fn draw_cooldown_bar(&self, params: PieceDrawParams<'_>, team_color: &str) {
        let now = TimestampMs::from_millis(js_sys::Date::now() as i64 + params.clock_offset_ms);
        let elapsed = now - params.piece.last_move_time;
        if elapsed >= params.piece.cooldown_ms {
            return;
        }
        let progress = elapsed.as_i64() as f64 / params.piece.cooldown_ms.as_i64() as f64;

        let tile = params.tile_size;
        let thickness = Self::cooldown_line_width(params.zoom);
        let gap = Self::cooldown_gap(tile);
        let side = (tile - 2.0 * gap).max(2.0 * thickness).round();
        let origin = (params.pos * tile + params.offset + (tile - side) / 2.0).round();

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_fill_style_str(team_color);
        self.draw_square_progress(origin, side, thickness, progress);
        self.ctx.set_global_alpha(1.0);
    }

    /// Draws the tinted piece icon, using a cached raster when available.
    fn draw_piece_icon(&self, handle: &PieceSvgHandle, params: PieceDrawParams) {
        let tile = params.tile_size;
        let icon_size = Self::piece_icon_size(tile, params.zoom);
        let corner = params.pos * tile + params.offset + (tile - icon_size) / 2.0;

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_image_smoothing_enabled(true);
        Self::set_image_smoothing_quality(&self.ctx, "high");
        let _ = match self.ensure_piece_raster(&handle.key, &handle.image) {
            Some(raster) => self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
                &raster, corner.x, corner.y, icon_size, icon_size,
            ),
            None => self.ctx.draw_image_with_html_image_element_and_dw_and_dh(
                &handle.image,
                corner.x,
                corner.y,
                icon_size,
                icon_size,
            ),
        };
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

    /// Returns the SVG template of a piece type, starting a fetch on first use.
    ///
    /// Failed fetches are not retried, so a missing asset does not cause a request per frame.
    fn ensure_svg_template(&self, piece_type: &PieceTypeId, svg_path: &str) -> Option<String> {
        {
            let mut cache = self.svg_cache.borrow_mut();
            if let Some(template) = cache.templates.get(piece_type) {
                return Some(template.clone());
            }
            if !cache.requested.insert(piece_type.clone()) {
                return None;
            }
        }

        let cache = self.svg_cache.clone();
        let piece_type = piece_type.clone();
        let svg_path = format!("/assets/pieces/{svg_path}");
        // Fetch the SVG template asynchronously to avoid blocking the render loop.
        spawn_local(async move {
            let text = match Request::get(&svg_path).send().await {
                Ok(response) => response.text().await.ok(),
                Err(_) => None,
            };
            if let Some(text) = text {
                cache.borrow_mut().templates.insert(piece_type, text);
            }
        });
        None
    }

    /// Draws the owner's name above a king.
    pub(crate) fn draw_piece_name(&self, params: PieceNameDrawParams<'_>) {
        let Some(player) = params
            .piece
            .owner_id
            .and_then(|owner| params.state.players.get(&owner))
        else {
            return;
        };
        let name = match player.name.trim() {
            "" => "An Unnamed Player",
            _ => &player.name,
        };
        let (tile, zoom) = (params.tile_size, params.zoom);
        self.ctx.set_font(&format!("{}px Upheaval", 12.0 * zoom));
        let Ok(metrics) = self.ctx.measure_text(name) else {
            return;
        };
        let anchor = params.pos * tile + params.offset;
        let x = anchor.x + tile / 2.0 - metrics.width() / 2.0;
        let y = anchor.y - 5.0 * zoom;

        self.ctx.set_stroke_style_str("rgba(255, 255, 255, 1)");
        self.ctx.set_line_width(3.0 * zoom);
        let _ = self.ctx.stroke_text(name, x, y);
        self.ctx.set_fill_style_str(player.color.as_ref());
        let _ = self.ctx.fill_text(name, x, y);
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

    /// Fills the elapsed share of the border cells of a `cells` by `cells` grid at `origin`,
    /// clockwise from the top center, in whole cells so the ring charges up pixel by pixel.
    fn draw_square_progress(&self, origin: Vec2, side: f64, thickness: f64, progress: f64) {
        // Each side owns the stretch from one corner up to, but excluding, the next.
        let length = side - thickness;
        let parts = COOLDOWN_PARTS_PER_SIDE;
        let filled = (progress.clamp(0.0, 1.0) * (4 * parts) as f64).ceil() as usize;
        let boundary = |part: usize| (part as f64 * length / parts as f64).round();
        let start = parts / 2;
        let end = start + filled;
        let mut index = start;
        while index < end {
            // Fills the run of parts left on the current side as one rectangle.
            let from = index % parts;
            let to = (from + end - index).min(parts);
            let (near, far) = (boundary(from), boundary(to));
            let (x, y, width, height) = match (index / parts) % 4 {
                0 => (near, 0.0, far - near, thickness),
                1 => (length, near, thickness, far - near),
                2 => (side - far, length, far - near, thickness),
                _ => (0.0, side - far, thickness, far - near),
            };
            self.ctx
                .fill_rect(origin.x + x, origin.y + y, width, height);
            index += to - from;
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
}
