//! Pixel-art fog of war: stepped rings of growing opacity around the local king.

use super::renderer::{ART_PIXELS_PER_TILE, ArtRect, FrameGeometry, ScreenRect, Visibility};
use super::types::Renderer;
use crate::math::Vec2;
use web_sys::HtmlCanvasElement;

/// Color of the fog, the off-board color.
const FOG_COLOR: &str = "#413931";

/// Size of one fog pixel, in art pixels, so the fog's edges step like the board's art.
const FOG_PIXEL_ART_PIXELS: f64 = 1.0;

/// The rings of the fog, outermost first: each covers what lies within its share of the view
/// radius, at its opacity. Beyond the outermost ring the fog is opaque.
const FOG_RINGS: &[FogRing] = &[
    FogRing {
        radius: 1.0,
        alpha: 0.75,
    },
    FogRing {
        radius: 0.87,
        alpha: 0.5,
    },
    FogRing {
        radius: 0.73,
        alpha: 0.25,
    },
    FogRing {
        radius: 0.6,
        alpha: 0.0,
    },
];

/// A disc of the fog at one opacity.
struct FogRing {
    /// Radius of the disc, as a share of the view radius.
    radius: f64,
    alpha: f64,
}

/// The fog around the view circle of one radius, one canvas pixel per fog pixel.
pub(crate) struct FogTexture {
    radius_squares: i32,
    canvas: HtmlCanvasElement,
    /// Half the width of the texture, in fog pixels.
    half_pixels: u32,
}

impl FogTexture {
    /// Paints the fog of a view `radius_squares` squares around the king's square.
    fn paint(radius_squares: i32) -> Option<Self> {
        let radius = (radius_squares.max(0) as f64 + 0.5) * ART_PIXELS_PER_TILE
            / FOG_PIXEL_ART_PIXELS;
        let half_pixels = radius.ceil() as u32;
        let (canvas, ctx) = Renderer::raster_canvas(half_pixels * 2)?;
        let side = (half_pixels * 2) as f64;
        let half = half_pixels as f64;
        ctx.set_fill_style_str(FOG_COLOR);
        ctx.fill_rect(0.0, 0.0, side, side);
        // Each row is cut out of the opaque fog ring by ring, from the outside in, refilling
        // what a ring covers at its opacity.
        for row in 0..half_pixels * 2 {
            let dy = row as f64 + 0.5 - half;
            for ring in FOG_RINGS {
                let reach = Self::half_span(dy, ring.radius * radius);
                if reach == 0.0 {
                    continue;
                }
                let (x, width) = (half - reach, reach * 2.0);
                ctx.clear_rect(x, row as f64, width, 1.0);
                if ring.alpha > 0.0 {
                    ctx.set_global_alpha(ring.alpha);
                    ctx.fill_rect(x, row as f64, width, 1.0);
                    ctx.set_global_alpha(1.0);
                }
            }
        }
        Some(Self {
            radius_squares,
            canvas,
            half_pixels,
        })
    }

    /// Returns how many pixels either side of the middle of a row, `dy` pixels from the
    /// center, have their centers within `radius` of the center.
    fn half_span(dy: f64, radius: f64) -> f64 {
        if dy.abs() >= radius {
            return 0.0;
        }
        ((radius * radius - dy * dy).sqrt() - 0.5).ceil().max(0.0)
    }

    /// Returns the art area the texture covers when centered on `center`, in art pixels.
    fn art_rect(&self, center: Vec2) -> ArtRect {
        let half = self.half_pixels as f64 * FOG_PIXEL_ART_PIXELS;
        ArtRect {
            min: center - half,
            max: center + half,
        }
    }
}

impl Renderer {
    /// Covers everything outside the view radius around the local king in fog, in rings of
    /// growing opacity that step along the art pixel grid.
    pub(super) fn draw_fog(
        &self,
        canvas_size: Vec2,
        geometry: &FrameGeometry,
        visibility: &Visibility,
    ) {
        if !visibility.fogged {
            return;
        }
        let mut cache = self.fog_cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|fog| fog.radius_squares != visibility.radius_squares)
        {
            *cache = FogTexture::paint(visibility.radius_squares);
        }
        let Some(fog) = cache.as_ref() else {
            return;
        };

        let center = geometry.square_art_center(visibility.center);
        let screen = geometry.screen_rect(fog.art_rect(center));
        self.ctx.save();
        self.ctx.set_image_smoothing_enabled(false);
        let _ = self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
            &fog.canvas,
            screen.min.x,
            screen.min.y,
            screen.size.x,
            screen.size.y,
        );
        self.ctx.restore();

        // Beyond the texture the fog is opaque.
        self.ctx.set_fill_style_str(FOG_COLOR);
        for rect in Self::surrounding_rects(screen, canvas_size) {
            self.ctx
                .fill_rect(rect.min.x, rect.min.y, rect.size.x, rect.size.y);
        }
    }

    /// Returns the rectangles covering a canvas of `canvas_size` outside `inner`: full-width
    /// bands above and below it, and the sides between them.
    fn surrounding_rects(inner: ScreenRect, canvas_size: Vec2) -> [ScreenRect; 4] {
        let max = inner.min + inner.size;
        let band = |top: f64, bottom: f64| ScreenRect {
            min: Vec2::new(0.0, top),
            size: Vec2::new(canvas_size.x, (bottom - top).max(0.0)),
        };
        let side = |left: f64, right: f64| ScreenRect {
            min: Vec2::new(left, inner.min.y),
            size: Vec2::new((right - left).max(0.0), inner.size.y),
        };
        [
            band(0.0, inner.min.y),
            band(max.y, canvas_size.y),
            side(0.0, inner.min.x),
            side(max.x, canvas_size.x),
        ]
    }
}
