//! Rendering of a replay to a video of its board.
//!
//! The board only changes when something happens on it or a cooldown ring charges up a
//! step, so only those frames are drawn and encoded, rather than recording the screen; every
//! other frame repeats the one before, which keeps the file about as small as the moves
//! themselves. The browser encodes the frames, and the script muxing them into a WebM file is
//! only loaded once a video is first rendered.

use super::player::LoadedReplay;
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
const VIDEO_SIDE_PX: i32 = 1080;
/// Squares of room left around the board, half on each side, so names over the kings on its
/// edge stay in the picture.
const MARGIN_SQUARES: i32 = 1;
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

/// How videos are drawn, from the client config.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct VideoSettings {
    /// Size of a square at zoom 1, as on screen, so text on the board keeps its size next to
    /// the squares.
    tile_size_px: f64,
    /// Time between frames: one server tick, the finest step the board changes in.
    frame_ms: i64,
}

impl From<&GlobalClientConfig> for VideoSettings {
    fn from(globals: &GlobalClientConfig) -> Self {
        Self {
            tile_size_px: globals.tile_size_px,
            frame_ms: i64::from(globals.tick_interval_ms.max(1)),
        }
    }
}

/// Size of the video.
#[derive(Clone, Copy)]
struct VideoLayout {
    /// Side of the square video, in pixels: a whole, even number of pixels per square of the
    /// largest board and its margin, as encoders need even sides.
    side: i32,
}

impl VideoLayout {
    /// Fits a board of `board_size` and its margin into the video.
    fn fitting(board_size: BoardSize) -> Self {
        let squares = Self::squares_across(board_size);
        let tile = (VIDEO_SIDE_PX / squares).max(2) & !1;
        Self {
            side: tile * squares,
        }
    }

    /// Returns how many squares a board of `board_size` and its margin span.
    fn squares_across(board_size: BoardSize) -> i32 {
        board_size.as_i32().max(1) + MARGIN_SQUARES
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
    pub(super) fn new(replay: Rc<LoadedReplay>, rotated_180: bool, settings: VideoSettings) -> Self {
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
        let largest = timeline
            .keyframes_from_start()
            .iter()
            .map(|keyframe| keyframe.state.board_size)
            .max()
            .unwrap_or_default();
        let layout = VideoLayout::fitting(largest);
        let canvas: HtmlCanvasElement = gloo_utils::document()
            .create_element("canvas")?
            .unchecked_into();
        canvas.set_width(layout.side as u32);
        canvas.set_height(layout.side as u32);
        let renderer = Renderer::new(canvas.clone(), Rc::new(self.replay.header.catalog.clone()))
            .ok_or("Canvas unavailable")?;
        let writer: VideoWriter =
            JsFuture::from(create_video_writer(layout.side as u32, layout.side as u32))
                .await?
                .unchecked_into();

        let duration = timeline.duration().as_i64();
        let frame_ms = self.settings.frame_ms;
        let last_frame = (duration + frame_ms - 1) / frame_ms;
        let mut shown: Option<(usize, TimestampMs)> = None;
        let mut percent = None;
        for frame in 0..=last_frame {
            let elapsed = DurationMs::from_millis((frame * frame_ms).min(duration));
            let at = timeline.start() + elapsed;
            let index = timeline.index_at(at);
            let state = timeline.board(index);
            let changed = shown.is_none_or(|(shown_index, shown_at)| {
                shown_index != index || rings_change(state, shown_at, at)
            });
            if changed {
                self.draw_when_ready(&renderer, state, at, layout).await;
            }
            JsFuture::from(writer.add_frame(&canvas, millis(elapsed), changed)).await?;
            shown = Some((index, at));
            let done = (frame * 100 / last_frame.max(1)) as u32;
            if percent.replace(done) != Some(done) {
                on_progress(done);
            }
        }
        let video = JsFuture::from(writer.finish(millis(timeline.duration()))).await?;
        Ok(video.unchecked_into())
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
        let squares = VideoLayout::squares_across(state.board_size);
        let tile_size_px = self.settings.tile_size_px;
        let zoom = layout.side as f64 / (squares as f64 * tile_size_px);
        renderer.draw(RenderParams {
            state,
            player_id: PlayerId::nil(),
            selected_piece_id: None,
            dragged_piece: None,
            projection: &Projection::default(),
            failed_trails: &[],
            move_blurs: &[],
            camera_pos: board_center(state.board_size) * tile_size_px * zoom,
            canvas_size: Vec2::splat(layout.side as f64),
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

/// Returns true when the cooldown ring of a player's piece on `state` looks different at `to`
/// than at `from`.
fn rings_change(state: &GameState, from: TimestampMs, to: TimestampMs) -> bool {
    state
        .pieces
        .values()
        .filter(|piece| piece.owner_id.is_some())
        .flat_map(Renderer::cooldown_ring_changes)
        .any(|at| from < at && at <= to)
}

/// Returns `duration` in milliseconds, as the encoder script counts time.
fn millis(duration: DurationMs) -> f64 {
    duration.as_i64() as f64
}
