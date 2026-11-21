//! Board switches: when one board replaces another, the screen is cut into a grid of regions
//! and the new board takes them over one by one, in row-major order, from the picture of the
//! old one.

use super::renderer::OFF_BOARD_COLOR;
use super::types::Renderer;
use crate::app::GlobalClientConfig;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// How a board switch runs, from the client config.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WipeTiming {
    /// Number of rows, and of columns, of regions.
    regions: u32,
    /// Milliseconds between one region taken over and the next.
    region_ms: f64,
}

impl From<&GlobalClientConfig> for WipeTiming {
    fn from(globals: &GlobalClientConfig) -> Self {
        Self {
            regions: globals.board_switch_regions.max(1),
            region_ms: globals.board_switch_region_ms.max(0.0),
        }
    }
}

impl WipeTiming {
    /// Returns how many regions the new board has taken over `elapsed_ms` into the switch.
    fn regions_taken(self, elapsed_ms: f64) -> u32 {
        if self.region_ms <= 0.0 {
            return self.regions.pow(2);
        }
        (elapsed_ms / self.region_ms).max(0.0) as u32
    }
}

/// A board switch in progress: a picture of the screen before it, still covering the regions
/// the new board has not taken over yet.
pub struct BoardWipe {
    old: HtmlCanvasElement,
    started_ms: f64,
}

impl BoardWipe {
    /// Starts a switch away from what `renderer` shows now, at `now_ms`. Whatever is not
    /// drawn yet counts as empty space around a board.
    pub fn from_screen(renderer: &Renderer, now_ms: f64) -> Option<Self> {
        let screen = renderer.ctx.canvas()?;
        let old: HtmlCanvasElement = gloo_utils::document()
            .create_element("canvas")
            .ok()?
            .unchecked_into();
        old.set_width(screen.width());
        old.set_height(screen.height());
        let ctx: CanvasRenderingContext2d = old.get_context("2d").ok()??.unchecked_into();
        ctx.set_fill_style_str(OFF_BOARD_COLOR);
        ctx.fill_rect(0.0, 0.0, f64::from(old.width()), f64::from(old.height()));
        ctx.draw_image_with_html_canvas_element(&screen, 0.0, 0.0)
            .ok()?;
        Some(Self {
            old,
            started_ms: now_ms,
        })
    }

    /// Covers the regions the new board has not taken over at `now_ms` with the old picture,
    /// over what `renderer` just drew.
    ///
    /// Returns false once the new board has taken over every region.
    pub fn cover(&self, renderer: &Renderer, timing: WipeTiming, now_ms: f64) -> bool {
        let Some(screen) = renderer.ctx.canvas() else {
            return false;
        };
        let regions = timing.regions;
        let taken = timing.regions_taken(now_ms - self.started_ms);
        if taken >= regions.pow(2) {
            return false;
        }
        let (row, column) = (taken / regions, taken % regions);
        // The rest of the current row, then every row below it, each as one rectangle.
        self.copy_region(renderer, &screen, (column, row), (regions, row + 1), regions);
        self.copy_region(renderer, &screen, (0, row + 1), (regions, regions), regions);
        true
    }

    /// Copies the old picture of the regions from `from` up to, but excluding, `to`, as
    /// (column, row) pairs of a grid of `regions` by `regions`, onto the screen.
    fn copy_region(
        &self,
        renderer: &Renderer,
        screen: &HtmlCanvasElement,
        from: (u32, u32),
        to: (u32, u32),
        regions: u32,
    ) {
        if from.0 >= to.0 || from.1 >= to.1 {
            return;
        }
        // Region edges are rounded to whole pixels, so neighbouring regions meet without seams.
        let edge = |index: u32, size: u32| {
            (f64::from(index) * f64::from(size) / f64::from(regions)).round()
        };
        let span = |canvas: &HtmlCanvasElement| {
            let (width, height) = (canvas.width(), canvas.height());
            let (left, top) = (edge(from.0, width), edge(from.1, height));
            (left, top, edge(to.0, width) - left, edge(to.1, height) - top)
        };
        let (sx, sy, sw, sh) = span(&self.old);
        let (dx, dy, dw, dh) = span(screen);
        let _ = renderer
            .ctx
            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &self.old, sx, sy, sw, sh, dx, dy, dw, dh,
            );
    }
}
