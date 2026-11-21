//! Piece drawing routines for the canvas renderer.

use super::color::{NPC_PIECE_COLOR, PieceIconColors, piece_icon_colors};
use super::renderer::{ART_PIXELS_PER_TILE, ArtRect};
use super::types::{PieceDrawParams, PieceNameDrawParams, PieceSvgKey, Renderer};
use crate::math::vec2;
use crate::utils::svg_data_url;
use common::models::{GameState, Piece};
use common::types::{DurationMs, PieceTypeId, TimestampMs};
use gloo_net::http::Request;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

/// Thickness of the cooldown ring around the rim of a piece's square, in art pixels.
const COOLDOWN_RING_PIXELS: f64 = 1.0;
/// Number of steps each side of the cooldown ring charges up in.
const COOLDOWN_PARTS_PER_SIDE: usize = 4;
/// Largest whole factor piece sprites are enlarged by before being scaled to the screen.
const MAX_RASTER_SCALE: f64 = 16.0;

/// Loaded SVG image plus its cache key.
struct PieceSvgHandle {
    key: PieceSvgKey,
    image: HtmlImageElement,
}

impl PieceDrawParams<'_> {
    /// Returns the art pixels of the piece's square, snapped to the art pixel grid even
    /// while the piece slides.
    fn square(&self) -> ArtRect {
        ArtRect::square(self.geometry.pos_art_origin(self.pos))
    }
}

impl Renderer {
    /// Draws a piece glyph and, for the local player's real pieces, its cooldown indicator.
    pub(super) fn draw_piece(&self, params: PieceDrawParams) {
        let base_color = Self::team_color(params.piece, params.state);
        // Prefer SVGs when available; fall back to a simple circle when missing.
        match self.loaded_icon(params.piece, params.state) {
            Some(handle) => self.draw_piece_icon(&handle, params),
            None => self.draw_piece_fallback(&base_color, params),
        }
        if params.shows_cooldown {
            self.draw_cooldown_bar(params, &base_color);
        }
    }

    /// Returns true when a cooldown ring on `state`, drawn for every player's pieces, looks
    /// different at `to` than at `from`.
    pub fn cooldown_rings_change(state: &GameState, from: TimestampMs, to: TimestampMs) -> bool {
        state
            .pieces
            .values()
            .filter(|piece| piece.owner_id.is_some())
            .flat_map(cooldown_ring_changes)
            .any(|at| from < at && at <= to)
    }

    /// Returns true once every piece on `state` can be drawn with its sprite, starting the
    /// loads still missing.
    pub fn sprites_ready(&self, state: &GameState) -> bool {
        state
            .pieces
            .values()
            .all(|piece| self.loaded_icon(piece, state).is_some())
    }

    /// Returns the sprite of `piece` in its team colors once it has loaded, starting the load
    /// on first use.
    fn loaded_icon(&self, piece: &Piece, state: &GameState) -> Option<PieceSvgHandle> {
        let colors = piece_icon_colors(&Self::team_color(piece, state));
        self.catalog
            .pieces
            .get(&piece.piece_type)
            .and_then(|config| {
                self.resolve_piece_icon(&piece.piece_type, &config.svg_path, &colors)
            })
            .filter(|handle| handle.image.complete() && handle.image.natural_width() > 0)
    }

    /// Returns the color of the team owning `piece`, or the NPC color for unowned pieces.
    pub(super) fn team_color(piece: &Piece, state: &GameState) -> String {
        piece
            .owner_id
            .and_then(|owner_id| state.players.get(&owner_id))
            .map(|player| player.color.as_ref().to_string())
            .unwrap_or_else(|| NPC_PIECE_COLOR.to_string())
    }

    /// Draws a "?" disc for pieces whose icon is unavailable.
    fn draw_piece_fallback(&self, base_color: &str, params: PieceDrawParams<'_>) {
        let (tile, zoom) = (params.geometry.tile, params.geometry.zoom);
        let center = params.geometry.pos_origin(params.pos) + tile / 2.0;
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

    /// Traces the elapsed share of the piece cooldown around the rim of its square.
    fn draw_cooldown_bar(&self, params: PieceDrawParams<'_>, team_color: &str) {
        let elapsed = params.now - params.piece.last_move_time;
        if elapsed >= params.piece.cooldown_ms {
            return;
        }
        let progress = elapsed.as_i64() as f64 / params.piece.cooldown_ms.as_i64() as f64;

        self.ctx.set_global_alpha(params.alpha);
        self.ctx.set_fill_style_str(team_color);
        self.fill_smooth_art_rects(square_progress(params.square(), progress), &params.geometry);
        self.ctx.set_global_alpha(1.0);
    }

    /// Draws the tinted piece sprite over the exact screen area of its square, like the
    /// board, so it moves and scales smoothly with the camera instead of jittering between
    /// screen pixels.
    ///
    /// The sprite is enlarged without smoothing by a whole factor, to at least its size on
    /// screen, and only then scaled down smoothly, so its art pixels stay crisp and evenly
    /// sized.
    fn draw_piece_icon(&self, handle: &PieceSvgHandle, params: PieceDrawParams) {
        let screen = params.geometry.exact_screen_rect(params.square());
        let (corner, size) = (screen.min, screen.size);
        let scale = (params.geometry.tile / ART_PIXELS_PER_TILE)
            .ceil()
            .clamp(1.0, MAX_RASTER_SCALE) as u32;

        self.ctx.set_global_alpha(params.alpha);
        let _ = match self.ensure_piece_raster(&handle.key, &handle.image, scale) {
            Some(raster) => self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
                &raster, corner.x, corner.y, size.x, size.y,
            ),
            None => self.ctx.draw_image_with_html_image_element_and_dw_and_dh(
                &handle.image,
                corner.x,
                corner.y,
                size.x,
                size.y,
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
        let data_url = svg_data_url(&tinted);
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
    pub(super) fn draw_piece_name(&self, params: PieceNameDrawParams<'_>) {
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
        let (tile, zoom) = (params.geometry.tile, params.geometry.zoom);
        self.ctx.set_font(&format!("{}px Upheaval", 12.0 * zoom));
        let Ok(metrics) = self.ctx.measure_text(name) else {
            return;
        };
        let anchor = params.geometry.pos_origin(params.pos);
        let x = anchor.x + tile / 2.0 - metrics.width() / 2.0;
        let y = anchor.y - 5.0 * zoom;

        self.ctx.set_stroke_style_str("rgba(255, 255, 255, 1)");
        self.ctx.set_line_width(3.0 * zoom);
        let _ = self.ctx.stroke_text(name, x, y);
        self.ctx.set_fill_style_str(player.color.as_ref());
        let _ = self.ctx.fill_text(name, x, y);
    }

    /// Returns the sprite rasterized at one pixel per art pixel and enlarged `scale` times
    /// without smoothing, rendering it on first use.
    fn ensure_piece_raster(
        &self,
        key: &PieceSvgKey,
        image: &HtmlImageElement,
        scale: u32,
    ) -> Option<HtmlCanvasElement> {
        let cache_key = (key.clone(), scale);
        {
            let cache = self.svg_cache.borrow();
            if let Some(raster) = cache.rasters.get(&cache_key) {
                return Some(raster.clone());
            }
        }

        let side = ART_PIXELS_PER_TILE as u32;
        let (base, base_ctx) = Self::raster_canvas(side)?;
        let _ = base_ctx.draw_image_with_html_image_element_and_dw_and_dh(
            image,
            0.0,
            0.0,
            ART_PIXELS_PER_TILE,
            ART_PIXELS_PER_TILE,
        );
        let (raster, ctx) = Self::raster_canvas(side * scale)?;
        let enlarged = (side * scale) as f64;
        let _ = ctx
            .draw_image_with_html_canvas_element_and_dw_and_dh(&base, 0.0, 0.0, enlarged, enlarged);

        let mut cache = self.svg_cache.borrow_mut();
        cache.rasters.insert(cache_key, raster.clone());
        Some(raster)
    }

    /// Creates an offscreen square canvas `side` pixels across that draws without smoothing.
    pub(super) fn raster_canvas(
        side: u32,
    ) -> Option<(HtmlCanvasElement, CanvasRenderingContext2d)> {
        let canvas = web_sys::window()?
            .document()?
            .create_element("canvas")
            .ok()?
            .dyn_into::<HtmlCanvasElement>()
            .ok()?;
        canvas.set_width(side);
        canvas.set_height(side);
        let ctx = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<CanvasRenderingContext2d>()
            .ok()?;
        ctx.set_image_smoothing_enabled(false);
        Some((canvas, ctx))
    }
}

/// Number of steps the whole cooldown ring charges up in.
const COOLDOWN_STEPS: i64 = 4 * COOLDOWN_PARTS_PER_SIDE as i64;

/// Returns the moments the cooldown ring of `piece` changes how it looks, from its last move
/// until the ring disappears: just past each step it charges up in, then when the cooldown is
/// over.
fn cooldown_ring_changes(piece: &Piece) -> impl Iterator<Item = TimestampMs> {
    let (start, cooldown) = (piece.last_move_time, piece.cooldown_ms.as_i64());
    let steps = if cooldown > 0 { COOLDOWN_STEPS } else { 0 };
    (0..steps)
        .map(move |step| step * cooldown / COOLDOWN_STEPS + 1)
        .chain(std::iter::once(cooldown))
        .map(move |elapsed| start + DurationMs::from_millis(elapsed))
}

/// Returns the elapsed share of a ring around the rim of `square`, clockwise from the top
/// center, in whole parts of a side so the ring charges up in steps.
///
/// Each side owns the stretch from one corner up to, but excluding, the next, so the parts
/// never overlap.
fn square_progress(square: ArtRect, progress: f64) -> impl Iterator<Item = ArtRect> {
    let thickness = COOLDOWN_RING_PIXELS;
    let side = ART_PIXELS_PER_TILE;
    let length = side - thickness;
    let parts = COOLDOWN_PARTS_PER_SIDE;
    let filled = (progress.clamp(0.0, 1.0) * COOLDOWN_STEPS as f64).ceil() as usize;
    let boundary = move |part: usize| (part as f64 * length / parts as f64).round();
    let start = parts / 2;
    let end = start + filled;
    // Each run covers the parts left on the current side, as one rectangle.
    std::iter::successors(Some(start), move |&index| {
        Some(index + parts - index % parts).filter(|&next| next < end)
    })
    .filter(move |&index| index < end)
    .map(move |index| {
        let from = index % parts;
        let to = (from + end - index).min(parts);
        let (near, far) = (boundary(from), boundary(to));
        let (min, size) = match (index / parts) % 4 {
            0 => (vec2(near, 0.0), vec2(far - near, thickness)),
            1 => (vec2(length, near), vec2(thickness, far - near)),
            2 => (vec2(side - far, length), vec2(far - near, thickness)),
            _ => (vec2(0.0, side - far), vec2(thickness, far - near)),
        };
        ArtRect {
            min: square.min + min,
            max: square.min + min + size,
        }
    })
}
