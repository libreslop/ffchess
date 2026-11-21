//! Board switches: when the board of another game mode replaces the one shown, the screen is
//! cut into a grid of regions and the new board takes them over one by one, in row-major order,
//! from the picture of the old one.
//!
//! A switch made while others still run starts from the top again, while the earlier ones carry
//! on below it: each region shows the newest board whose switch has reached it.

use super::renderer::OFF_BOARD_COLOR;
use super::types::Renderer;
use crate::app::GlobalClientConfig;
use crate::math::{Vec2, vec2};
use common::types::ModeId;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// How a board switch runs, from the client config.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BoardSwitchTiming {
    /// Number of rows, and of columns, of regions.
    regions_per_side: u32,
    /// Milliseconds between one region taken over and the next.
    region_ms: f64,
}

impl From<&GlobalClientConfig> for BoardSwitchTiming {
    fn from(globals: &GlobalClientConfig) -> Self {
        Self {
            regions_per_side: globals.board_switch_regions.max(1),
            region_ms: globals.board_switch_region_ms.max(0.0),
        }
    }
}

impl BoardSwitchTiming {
    /// Returns the number of regions in the grid.
    fn region_count(self) -> u32 {
        self.regions_per_side.pow(2)
    }

    /// Returns how many regions a switch has taken over `elapsed_ms` after it started, at
    /// most all of them.
    fn regions_taken(self, elapsed_ms: f64) -> u32 {
        if self.region_ms <= 0.0 {
            return self.region_count();
        }
        ((elapsed_ms / self.region_ms).max(0.0) as u32).min(self.region_count())
    }

    /// Returns true once a switch started `elapsed_ms` ago has taken over every region.
    fn is_over(self, elapsed_ms: f64) -> bool {
        self.regions_taken(elapsed_ms) == self.region_count()
    }

    /// Returns the region with `index`, counted in row-major order.
    fn region(self, index: u32) -> Region {
        Region {
            column: index % self.regions_per_side,
            row: index / self.regions_per_side,
        }
    }
}

/// One region of the grid the screen is cut into.
#[derive(Clone, Copy)]
struct Region {
    column: u32,
    row: u32,
}

/// A rectangle of whole regions, from `start` up to, but excluding, `end`.
struct RegionBlock {
    start: Region,
    end: Region,
}

impl RegionBlock {
    /// Returns the blocks still showing the old board once `taken` regions have the new one:
    /// the rest of the row the switch is in, then every row below it.
    fn not_taken(timing: BoardSwitchTiming, taken: u32) -> [Self; 2] {
        let front = timing.region(taken);
        let side = timing.regions_per_side;
        let row_end = Region {
            column: side,
            row: front.row + 1,
        };
        [
            Self {
                start: front,
                end: row_end,
            },
            Self {
                start: Region {
                    column: 0,
                    row: row_end.row,
                },
                end: Region {
                    column: side,
                    row: side,
                },
            },
        ]
    }

    /// Returns true when the block holds no region.
    fn is_empty(&self) -> bool {
        self.start.column >= self.end.column || self.start.row >= self.end.row
    }

    /// Returns the pixels the block spans on `canvas`, cut into `per_side` regions per side.
    ///
    /// Region edges are rounded to whole pixels, so neighbouring regions meet without seams.
    fn pixels_on(&self, canvas: &HtmlCanvasElement, per_side: u32) -> PixelRect {
        let size = vec2(f64::from(canvas.width()), f64::from(canvas.height()));
        let edge = |region: Region| {
            (vec2(f64::from(region.column), f64::from(region.row)) * size / f64::from(per_side))
                .round()
        };
        let (origin, end) = (edge(self.start), edge(self.end));
        PixelRect {
            origin,
            size: end - origin,
        }
    }
}

/// A rectangle of canvas pixels.
struct PixelRect {
    origin: Vec2,
    size: Vec2,
}

/// One board switch: the picture of the board before it, and when it started.
struct Switch {
    old: HtmlCanvasElement,
    started_ms: f64,
}

impl Switch {
    /// Draws the old picture over the regions this switch has not taken over yet.
    fn cover(&self, ctx: &CanvasRenderingContext2d, timing: BoardSwitchTiming, now_ms: f64) {
        let Some(screen) = ctx.canvas() else {
            return;
        };
        let taken = timing.regions_taken(now_ms - self.started_ms);
        for block in RegionBlock::not_taken(timing, taken) {
            if block.is_empty() {
                continue;
            }
            let from = block.pixels_on(&self.old, timing.regions_per_side);
            let to = block.pixels_on(&screen, timing.regions_per_side);
            let _ = ctx
                .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                    &self.old,
                    from.origin.x,
                    from.origin.y,
                    from.size.x,
                    from.size.y,
                    to.origin.x,
                    to.origin.y,
                    to.size.x,
                    to.size.y,
                );
        }
    }
}

/// The board switches of a view: which mode's board it shows, and the switches that still
/// run, oldest first.
#[derive(Default)]
pub struct BoardSwitches {
    /// Game mode of the board drawn last.
    shown_mode: Option<ModeId>,
    running: Vec<Switch>,
    /// The latest frame of the board shown, as drawn before the switches covered it, kept
    /// while switches run so a new switch can start from the whole of it.
    uncovered: Option<HtmlCanvasElement>,
}

impl BoardSwitches {
    /// Notes that the next frame shows a board of `mode`, starting a switch at `now_ms` when
    /// the last frame showed a board of another mode.
    ///
    /// The first board a view shows, and boards of the same mode, such as a match and the
    /// menu of its mode, appear without one.
    pub fn show_mode(&mut self, mode: &ModeId, renderer: &Renderer, now_ms: f64) {
        let previous = self.shown_mode.replace(mode.clone());
        if previous.is_some_and(|previous| previous != *mode) {
            self.start(renderer, now_ms);
        }
    }

    /// Covers what `renderer` just drew with the boards the running switches have not yet
    /// replaced, region by region, and forgets the switches that are over.
    pub fn cover(&mut self, renderer: &Renderer, timing: BoardSwitchTiming, now_ms: f64) {
        self.running
            .retain(|switch| !timing.is_over(now_ms - switch.started_ms));
        if self.running.is_empty() {
            self.uncovered = None;
            return;
        }
        let Some(screen) = renderer.ctx.canvas() else {
            return;
        };
        self.keep_uncovered(&screen);
        // Each switch shows its old board from its own front on; older switches, whose
        // fronts are further down, are drawn last so their old boards stay below them.
        for switch in self.running.iter().rev() {
            switch.cover(&renderer.ctx, timing, now_ms);
        }
    }

    /// Starts a switch away from the board `renderer` showed on the last frame, at `now_ms`.
    fn start(&mut self, renderer: &Renderer, now_ms: f64) {
        // While switches run, the screen is covered by them, so the board shown is the copy
        // kept from before they covered it.
        let shown = self.uncovered.clone().or_else(|| renderer.ctx.canvas());
        if let Some(old) = shown.as_ref().and_then(copy_of) {
            self.running.push(Switch {
                old,
                started_ms: now_ms,
            });
        }
    }

    /// Keeps a copy of the frame on `screen` before it is covered, reusing the last copy when
    /// the screen kept its size.
    fn keep_uncovered(&mut self, screen: &HtmlCanvasElement) {
        let reusable = self
            .uncovered
            .as_ref()
            .filter(|uncovered| {
                uncovered.width() == screen.width() && uncovered.height() == screen.height()
            })
            .and_then(context_of);
        match reusable {
            Some(ctx) => {
                let _ = ctx.draw_image_with_html_canvas_element(screen, 0.0, 0.0);
            }
            None => self.uncovered = copy_of(screen),
        }
    }
}

/// Returns a new canvas holding what `source` shows, over the empty space around a board,
/// which stands in for whatever was never drawn.
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
