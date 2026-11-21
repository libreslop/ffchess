//! Board switches: when the board of another game mode replaces the one shown, the screen is
//! cut into a grid of regions and the new board takes them over one by one, in row-major order,
//! from the picture of the old one.
//!
//! A switch made while others still run starts from the top again, while the earlier ones carry
//! on below it: each region shows the newest board whose switch has reached it.

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
    /// Returns how many regions a switch has taken over `elapsed_ms` after it started, at
    /// most all of them.
    fn regions_taken(self, elapsed_ms: f64) -> u32 {
        let all = self.regions.pow(2);
        if self.region_ms <= 0.0 {
            return all;
        }
        ((elapsed_ms / self.region_ms).max(0.0) as u32).min(all)
    }
}

/// One board switch: the picture of the board before it, and when it started.
struct Wipe {
    old: HtmlCanvasElement,
    started_ms: f64,
}

/// The board switches of a view that still run, oldest first.
#[derive(Default)]
pub struct BoardWipes {
    running: Vec<Wipe>,
    /// The latest frame of the board shown, as drawn before the switches covered it, kept
    /// while switches run so a new switch can start from the whole of it.
    uncovered: Option<HtmlCanvasElement>,
}

impl BoardWipes {
    /// Starts a switch away from the board `renderer` showed on the last frame, at `now_ms`.
    /// Whatever was never drawn counts as empty space around a board.
    pub fn start(&mut self, renderer: &Renderer, now_ms: f64) {
        let source = match &self.uncovered {
            Some(uncovered) if !self.running.is_empty() => Some(uncovered.clone()),
            _ => renderer.ctx.canvas(),
        };
        if let Some(old) = source.and_then(|source| copy_of(&source)) {
            self.running.push(Wipe {
                old,
                started_ms: now_ms,
            });
        }
    }

    /// Covers what `renderer` just drew with the boards the running switches have not yet
    /// replaced, region by region, and forgets the switches that are over.
    pub fn cover(&mut self, renderer: &Renderer, timing: WipeTiming, now_ms: f64) {
        let all = timing.regions.pow(2);
        self.running
            .retain(|wipe| timing.regions_taken(now_ms - wipe.started_ms) < all);
        let Some(screen) = renderer.ctx.canvas() else {
            return;
        };
        if self.running.is_empty() {
            self.uncovered = None;
            return;
        }
        self.keep_uncovered(&screen);
        // Each switch shows its old board from its own front on; older switches, whose
        // fronts are further down, are drawn last so their old boards stay below them.
        for wipe in self.running.iter().rev() {
            let taken = timing.regions_taken(now_ms - wipe.started_ms);
            wipe.cover_from(renderer, &screen, taken, timing.regions);
        }
    }

    /// Keeps a copy of the frame on `screen` before it is covered.
    fn keep_uncovered(&mut self, screen: &HtmlCanvasElement) {
        let reusable = self.uncovered.as_ref().filter(|uncovered| {
            uncovered.width() == screen.width() && uncovered.height() == screen.height()
        });
        match reusable.and_then(|uncovered| context_of(uncovered).map(|ctx| (uncovered, ctx))) {
            Some((_, ctx)) => {
                let _ = ctx.draw_image_with_html_canvas_element(screen, 0.0, 0.0);
            }
            None => self.uncovered = copy_of(screen),
        }
    }
}

impl Wipe {
    /// Draws the old picture over the regions from `taken` on, of a grid of `regions` by
    /// `regions`.
    fn cover_from(
        &self,
        renderer: &Renderer,
        screen: &HtmlCanvasElement,
        taken: u32,
        regions: u32,
    ) {
        let (row, column) = (taken / regions, taken % regions);
        // The rest of the current row, then every row below it, each as one rectangle.
        self.copy_region(renderer, screen, (column, row), (regions, row + 1), regions);
        self.copy_region(renderer, screen, (0, row + 1), (regions, regions), regions);
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

/// Returns a new canvas holding what `source` shows, over empty space around a board.
fn copy_of(source: &HtmlCanvasElement) -> Option<HtmlCanvasElement> {
    let copy: HtmlCanvasElement = gloo_utils::document()
        .create_element("canvas")
        .ok()?
        .unchecked_into();
    copy.set_width(source.width());
    copy.set_height(source.height());
    let ctx = context_of(&copy)?;
    ctx.set_fill_style_str(OFF_BOARD_COLOR);
    ctx.fill_rect(0.0, 0.0, f64::from(copy.width()), f64::from(copy.height()));
    ctx.draw_image_with_html_canvas_element(source, 0.0, 0.0)
        .ok()?;
    Some(copy)
}

/// Returns the 2D drawing context of `canvas`.
fn context_of(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    Some(canvas.get_context("2d").ok()??.unchecked_into())
}
