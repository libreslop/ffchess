//! Timers driving the view: the per-frame redraw, the camera step, and wheel zoom.

use super::types::{FpsCounter, LatestStateSnapshot};
use crate::app::GlobalClientConfig;
use crate::camera::{CameraManager, CameraUpdateParams, update_camera};
use crate::math::{Vec2, vec2};
use crate::reducer::GameAction;
use crate::utils::performance_now;
use gloo_events::EventListener;
use gloo_render::{AnimationFrame, request_animation_frame};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::hook;
use yew::prelude::*;

/// Shortest allowed camera step interval, in milliseconds.
const MIN_CAMERA_STEP_MS: u32 = 8;

/// Re-renders the view on every animation frame so time-based visuals stay smooth.
///
/// Returns the timestamp of the latest frame.
#[hook]
pub(super) fn use_animation_frame() -> f64 {
    let frame = use_state(|| 0.0f64);
    {
        let frame = frame.clone();
        use_effect_with((), move |_| {
            /// Requests the next frame and stores its handle so dropping the cell stops the loop.
            fn schedule(cell: Rc<RefCell<Option<AnimationFrame>>>, frame: UseStateHandle<f64>) {
                let next_cell = cell.clone();
                let handle = request_animation_frame(move |timestamp| {
                    frame.set(timestamp);
                    schedule(next_cell, frame);
                });
                *cell.borrow_mut() = Some(handle);
            }
            let cell = Rc::new(RefCell::new(None));
            schedule(cell.clone(), frame);
            move || drop(cell.borrow_mut().take())
        });
    }
    *frame
}

/// Inputs of the camera stepping loop.
pub(super) struct CameraLoopInputs {
    pub(super) camera: Rc<RefCell<CameraManager>>,
    pub(super) latest: Rc<RefCell<LatestStateSnapshot>>,
    pub(super) canvas_ref: NodeRef,
    pub(super) cam_state: UseStateHandle<Vec2>,
    pub(super) zoom_state: UseStateHandle<f64>,
    pub(super) globals: GlobalClientConfig,
}

/// Steps the camera on a fixed interval and reports the frame rate.
#[hook]
pub(super) fn use_camera_loop(inputs: CameraLoopInputs) {
    let interval_ms = inputs.globals.render_interval_ms.max(MIN_CAMERA_STEP_MS);
    use_effect_with(interval_ms, move |&interval_ms| {
        let mut fps = FpsCounter::new(performance_now());
        let interval = gloo_timers::callback::Interval::new(interval_ms, move || {
            let (reducer, is_dragging) = {
                let latest = inputs.latest.borrow();
                (latest.reducer.clone(), latest.is_dragging)
            };
            if let Some(fps) = fps.frame(performance_now()) {
                reducer.dispatch(GameAction::SetFPS(fps));
            }

            let mut camera = inputs.camera.borrow_mut();
            let globals = &inputs.globals;
            let changed = update_camera(
                &mut camera,
                CameraUpdateParams {
                    state: &reducer.state,
                    player_id: reducer.player_id,
                    canvas_ref: &inputs.canvas_ref,
                    is_dragging,
                    mode: reducer.mode.as_ref(),
                    piece_count: reducer.state.pieces_of(reducer.viewer_id()).count(),
                    phase: reducer.phase,
                    is_victory: reducer.is_victory(),
                    victory_focus_target: reducer
                        .victory
                        .as_ref()
                        .map(|victory| victory.focus_target)
                        .unwrap_or_default(),
                    zoom_min: globals.camera_zoom_min,
                    zoom_max: globals.camera_zoom_max,
                    zoom_lerp: globals.zoom_lerp,
                    inertia_decay: globals.inertia_decay,
                    velocity_cutoff: globals.velocity_cutoff,
                    pan_idle_ms: globals.pan_idle_ms,
                    pan_lerp_alive: globals.pan_lerp_alive,
                    pan_lerp_dead: globals.pan_lerp_dead,
                    tile_size_px: globals.tile_size_px,
                    death_zoom: globals.death_zoom,
                    board_rotated_180: reducer.board_rotated_180(),
                },
            );
            if changed {
                inputs.zoom_state.set(camera.zoom);
                inputs.cam_state.set(camera.camera);
            }
        });
        move || drop(interval)
    });
}

/// Inputs of the mouse-wheel zoom listener.
pub(super) struct WheelZoomInputs {
    pub(super) camera: Rc<RefCell<CameraManager>>,
    pub(super) canvas_ref: NodeRef,
    pub(super) input_blocked: bool,
    pub(super) camera_locked: bool,
    pub(super) zoom_min: f64,
    pub(super) zoom_max: f64,
    pub(super) scroll_base: f64,
}

/// Zooms the camera with the mouse wheel towards the cursor.
#[hook]
pub(super) fn use_wheel_zoom(inputs: WheelZoomInputs) {
    let WheelZoomInputs {
        camera,
        canvas_ref,
        input_blocked,
        camera_locked,
        zoom_min,
        zoom_max,
        scroll_base,
    } = inputs;
    use_effect_with(
        (canvas_ref, input_blocked, camera_locked),
        move |(canvas_ref, input_blocked, camera_locked)| {
            let (input_blocked, camera_locked) = (*input_blocked, *camera_locked);
            let listener = canvas_ref.cast::<web_sys::HtmlElement>().map(|canvas| {
                EventListener::new(&canvas, "wheel", move |event| {
                    let Some(event) = event.dyn_ref::<web_sys::WheelEvent>() else {
                        return;
                    };
                    if input_blocked {
                        return;
                    }
                    event.prevent_default();
                    let mut camera = camera.borrow_mut();
                    if camera.input_locked || camera_locked {
                        return;
                    }
                    let factor = scroll_base.max(1.01).powf(-event.delta_y() / 100.0);
                    camera.mouse_pos = vec2(event.client_x() as f64, event.client_y() as f64);
                    camera.target_zoom = (camera.target_zoom * factor).clamp(zoom_min, zoom_max);
                })
            });
            move || drop(listener)
        },
    );
}
