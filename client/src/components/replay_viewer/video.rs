//! Rendering of a replay to a video of its board.
//!
//! The board only changes when something happens on it or a cooldown ring charges up a
//! step, so only those frames are drawn and encoded, rather than recording the screen; every
//! other frame repeats the one before, which keeps the file about as small as the moves
//! themselves. The browser encodes the frames, and the script muxing them into a WebM file is
//! only loaded once a video is first rendered.

use super::loaded::LoadedReplay;
use super::timeline::ReplayTimeline;
use crate::app::GlobalClientConfig;
use crate::camera::board_center;
use crate::canvas::{RenderParams, Renderer};
use crate::math::Vec2;
use crate::premoves::Projection;
use common::models::GameState;
use common::types::{BoardSize, DurationMs, PlayerId, TimestampMs};
use gloo_timers::future::TimeoutFuture;
use js_sys::Promise;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, HtmlCanvasElement};

/// Side of the video the largest board is fitted into, in pixels.
const VIDEO_SIDE_PX: u32 = 1080;
/// Squares of room left around the board, half on each side, so names over the kings on its
/// edge stay in the picture.
const MARGIN_SQUARES: u32 = 1;
/// How long to wait for piece sprites to load before drawing a frame again, in milliseconds.
const SPRITE_WAIT_MS: u32 = 50;
/// How many times a frame waits for its sprites before it is drawn without them.
const SPRITE_WAIT_TRIES: u32 = 100;

#[wasm_bindgen(inline_js = "
export async function create_video_writer(width, height) {
    const { createVideoWriter } = await import('/assets/static/js/replay_video.js');
    return createVideoWriter(width, height);
}
")]
extern "C" {
    /// Loads the encoder script and starts a video of `width` by `height` pixels, resolving
    /// to its [`VideoWriter`].
    fn create_video_writer(width: u32, height: u32) -> Promise;

    /// Video being encoded by the encoder script.
    type VideoWriter;

    /// Adds the frame appearing `at_ms` into the video: what `canvas` shows when `changed`,
    /// otherwise a repeat of the frame before.
    #[wasm_bindgen(method, js_name = addFrame)]
    fn add_frame(
        this: &VideoWriter,
        canvas: &HtmlCanvasElement,
        at_ms: f64,
        changed: bool,
    ) -> Promise;

    /// Finishes a video lasting `duration_ms`, resolving to its file.
    #[wasm_bindgen(method)]
    fn finish(this: &VideoWriter, duration_ms: f64) -> Promise;
}

impl VideoWriter {
    /// Starts a video of the size of `layout`.
    async fn start(layout: VideoLayout) -> Result<Self, JsValue> {
        let writer = JsFuture::from(create_video_writer(layout.side_px, layout.side_px)).await?;
        Ok(writer.unchecked_into())
    }

    /// Adds the frame appearing `elapsed` into the video: what `canvas` shows when `changed`,
    /// otherwise a repeat of the frame before.
    async fn write_frame(
        &self,
        canvas: &HtmlCanvasElement,
        elapsed: DurationMs,
        changed: bool,
    ) -> Result<(), JsValue> {
        JsFuture::from(self.add_frame(canvas, elapsed.as_i64() as f64, changed)).await?;
        Ok(())
    }

    /// Finishes a video lasting `duration` and returns its file.
    async fn into_file(self, duration: DurationMs) -> Result<Blob, JsValue> {
        let file = JsFuture::from(self.finish(duration.as_i64() as f64)).await?;
        Ok(file.unchecked_into())
    }
}

/// How videos are drawn, from the client config.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct VideoSettings {
    /// Size of a square at zoom 1, as on screen, so text on the board keeps its size next to
    /// the squares.
    tile_size_px: f64,
    /// Time between frames: one server tick, the finest step the board changes in.
    frame: DurationMs,
    /// How much of the board before the match starts the video opens with.
    lead: DurationMs,
    /// How much of the board after the match ends the video closes with.
    tail: DurationMs,
}

impl From<&GlobalClientConfig> for VideoSettings {
    fn from(globals: &GlobalClientConfig) -> Self {
        Self {
            tile_size_px: globals.tile_size_px,
            frame: DurationMs::from_millis(i64::from(globals.tick_interval_ms.max(1))),
            lead: DurationMs::from_millis(i64::from(globals.replay_video_lead_ms)),
            tail: DurationMs::from_millis(i64::from(globals.replay_video_tail_ms)),
        }
    }
}

/// Size of the video.
#[derive(Clone, Copy)]
struct VideoLayout {
    /// Side of the square video, in pixels: a whole, even number of pixels per square of the
    /// largest board and its margin, as encoders need even sides.
    side_px: u32,
}

impl VideoLayout {
    /// Fits a board of `board_size` and its margin into the video.
    fn fitting(board_size: BoardSize) -> Self {
        let squares = Self::squares_across(board_size);
        let tile_px = (VIDEO_SIDE_PX / squares).max(2) & !1;
        Self {
            side_px: tile_px * squares,
        }
    }

    /// Returns the zoom that fits a board of `board_size` and its margin into the video, when
    /// a square spans `tile_size_px` at zoom 1.
    fn zoom_for(self, board_size: BoardSize, tile_size_px: f64) -> f64 {
        f64::from(self.side_px) / (f64::from(Self::squares_across(board_size)) * tile_size_px)
    }

    /// Returns how many squares a board of `board_size` and its margin span.
    fn squares_across(board_size: BoardSize) -> u32 {
        board_size.as_i32().max(1).unsigned_abs() + MARGIN_SQUARES
    }

    /// Returns a canvas of the size of the video.
    fn canvas(self) -> Result<HtmlCanvasElement, JsValue> {
        let canvas: HtmlCanvasElement = gloo_utils::document()
            .create_element("canvas")?
            .unchecked_into();
        canvas.set_width(self.side_px);
        canvas.set_height(self.side_px);
        Ok(canvas)
    }
}

/// When the frames of a video appear: one every frame interval, from a little before the
/// match starts to a little after it ends.
struct FrameSchedule {
    /// Server time the first frame shows.
    first: TimestampMs,
    duration: DurationMs,
    frame: DurationMs,
}

impl FrameSchedule {
    /// Schedules the frames of a video of `timeline`.
    fn new(timeline: &ReplayTimeline, settings: VideoSettings) -> Self {
        let first = timeline.start() - settings.lead;
        Self {
            first,
            duration: timeline.end() + settings.tail - first,
            frame: settings.frame,
        }
    }

    /// Returns the number of the last frame, which shows the very end of the video.
    fn last_frame(&self) -> i64 {
        let (duration, frame) = (self.duration.as_i64(), self.frame.as_i64());
        (duration + frame - 1) / frame
    }

    /// Returns how far into the video each frame appears.
    fn frames(&self) -> impl Iterator<Item = DurationMs> {
        let (duration, frame) = (self.duration.as_i64(), self.frame.as_i64());
        (0..=self.last_frame())
            .map(move |number| DurationMs::from_millis((number * frame).min(duration)))
    }
}

/// A board drawn on the video: its index on the timeline, and the moment drawn.
#[derive(Clone, Copy)]
struct DrawnBoard {
    index: usize,
    at: TimestampMs,
}

impl DrawnBoard {
    /// Returns true when the board with `index` at `at` looks just like this one.
    fn looks_like(self, timeline: &ReplayTimeline, index: usize, at: TimestampMs) -> bool {
        self.index == index && !Renderer::cooldown_rings_change(timeline.board(index), self.at, at)
    }
}

/// Tells a callback the whole percentage of frames done, whenever it grows.
struct Progress<F> {
    on_progress: F,
    last_frame: i64,
    reported: Option<u32>,
}

impl<F: Fn(u32)> Progress<F> {
    /// Reports frames of a video whose last frame is `last_frame` to `on_progress`.
    fn new(last_frame: i64, on_progress: F) -> Self {
        Self {
            on_progress,
            last_frame: last_frame.max(1),
            reported: None,
        }
    }

    /// Notes that the frame `number` is done.
    fn reach(&mut self, number: i64) {
        let percent = (number * 100 / self.last_frame) as u32;
        if self.reported.replace(percent) != Some(percent) {
            (self.on_progress)(percent);
        }
    }
}

/// A replay being rendered to video, seen from one side of the board.
pub(super) struct VideoRender {
    replay: Rc<LoadedReplay>,
    rotated_180: bool,
    settings: VideoSettings,
}

impl VideoRender {
    /// Prepares the video of `replay`, upside down when `rotated_180`.
    pub(super) fn new(
        replay: Rc<LoadedReplay>,
        rotated_180: bool,
        settings: VideoSettings,
    ) -> Self {
        Self {
            replay,
            rotated_180,
            settings,
        }
    }

    /// Renders the video, telling `on_progress` the percentage of frames done whenever it
    /// grows.
    ///
    /// Frames come at a constant rate, one per server tick, so players can seek anywhere. A
    /// frame showing the same picture as the one before is only marked as a repeat, which
    /// costs the file a few bytes.
    pub(super) async fn run(self, on_progress: impl Fn(u32)) -> Result<Blob, JsValue> {
        let timeline = &self.replay.timeline;
        let layout = VideoLayout::fitting(self.largest_board());
        let canvas = layout.canvas()?;
        let renderer = Renderer::new(canvas.clone(), Rc::new(self.replay.header.catalog.clone()))
            .ok_or("Canvas unavailable")?;
        let writer = VideoWriter::start(layout).await?;
        let schedule = FrameSchedule::new(timeline, self.settings);
        let mut progress = Progress::new(schedule.last_frame(), on_progress);
        let mut drawn: Option<DrawnBoard> = None;
        for (number, elapsed) in (0..).zip(schedule.frames()) {
            let at = schedule.first + elapsed;
            let index = timeline.index_at(at);
            let changed = drawn.is_none_or(|drawn| !drawn.looks_like(timeline, index, at));
            if changed {
                let state = timeline.board(index);
                self.draw_when_ready(&renderer, state, at, layout).await;
                drawn = Some(DrawnBoard { index, at });
            }
            writer.write_frame(&canvas, elapsed, changed).await?;
            progress.reach(number);
        }
        writer.into_file(schedule.duration).await
    }

    /// Returns the size of the largest board of the replay.
    fn largest_board(&self) -> BoardSize {
        self.replay
            .timeline
            .keyframes_from_start()
            .iter()
            .map(|keyframe| keyframe.state.board_size)
            .max()
            .unwrap_or_default()
    }

    /// Draws `state` at `at`, again until its piece sprites have loaded or the wait runs out.
    async fn draw_when_ready(
        &self,
        renderer: &Renderer,
        state: &GameState,
        at: TimestampMs,
        layout: VideoLayout,
    ) {
        for _ in 0..SPRITE_WAIT_TRIES {
            self.draw(renderer, state, at, layout);
            if renderer.sprites_ready(state) {
                return;
            }
            TimeoutFuture::new(SPRITE_WAIT_MS).await;
        }
        self.draw(renderer, state, at, layout);
    }

    /// Draws `state` at `at` fitted to the video, as a spectator sees it, with every player's
    /// cooldowns.
    fn draw(&self, renderer: &Renderer, state: &GameState, at: TimestampMs, layout: VideoLayout) {
        let tile_size_px = self.settings.tile_size_px;
        let zoom = layout.zoom_for(state.board_size, tile_size_px);
        renderer.draw(RenderParams {
            state,
            player_id: PlayerId::nil(),
            selected_piece_id: None,
            dragged_piece: None,
            projection: &Projection::default(),
            failed_trails: &[],
            move_blurs: &[],
            camera_pos: board_center(state.board_size) * tile_size_px * zoom,
            canvas_size: Vec2::splat(f64::from(layout.side_px)),
            zoom,
            tile_size_px,
            mode: Some(&self.replay.header.mode),
            board_rotated_180: self.rotated_180,
            active_shop_highlight_pos: None,
            disable_fog_of_war: true,
            now: at,
            show_all_cooldowns: true,
        });
    }
}
