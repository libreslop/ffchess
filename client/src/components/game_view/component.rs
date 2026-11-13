//! Main in-game canvas view: wiring of state, input, and overlays.

use super::chat::GameChat;
use super::geometry::{is_ui_exempt_target, read_window_size};
use super::input::{PointerContext, mouse_leave_handler};
use super::loops::{
    CameraLoopInputs, WheelZoomInputs, use_animation_frame, use_camera_loop, use_wheel_zoom,
};
use super::render::{BoardFrame, use_board_render, use_piece_animations};
use super::shop_context::ActiveShopQuery;
use super::touch::TouchContext;
use super::types::{InputEnd, InputMove, InputStart, LatestStateSnapshot, ViewHandles};
use crate::app::GlobalClientConfig;
use crate::camera::CameraManager;
use crate::canvas::Renderer;
use crate::components::shop_ui::ShopUI;
use crate::math::{Vec2, vec2};
use crate::reducer::{GameAction, GameStateReducer, MsgSender};
use common::models::{PieceAction, ShopItemIndex};
use common::types::{DurationMs, PieceId, Score};
use gloo_events::EventListener;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Properties for the main game viewport.
#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
    pub globals: GlobalClientConfig,
    pub player_name: String,
}

/// Renders the board canvas, HUD, shop menu, and chat, and routes input to them.
#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let reducer = &props.reducer;
    let globals = &props.globals;
    let has_match_result = reducer.has_match_result();

    let canvas_ref = use_node_ref();
    let selected = use_state(|| None::<PieceId>);
    let drag_start = use_state(|| None);
    let latest = use_mut_ref(|| LatestStateSnapshot {
        reducer: reducer.clone(),
        is_dragging: false,
    });
    *latest.borrow_mut() = LatestStateSnapshot {
        reducer: reducer.clone(),
        is_dragging: drag_start.is_some(),
    };
    let handles = ViewHandles {
        canvas_ref: canvas_ref.clone(),
        camera: use_mut_ref(CameraManager::new),
        cam_state: use_state(|| Vec2::ZERO),
        zoom_state: use_state(|| 1.0f64),
        drag_start,
        did_pan: use_mut_ref(|| false),
        selected: selected.clone(),
        latest: latest.clone(),
        touch_gesture_active: use_mut_ref(|| false),
        last_tap: use_mut_ref(|| None),
    };
    let window_size = use_window_size();
    use_animation_frame();

    let renderer = use_renderer(&canvas_ref, reducer);
    let projection = use_memo(
        (
            reducer.state.clone(),
            reducer.premoves.clone(),
            reducer.catalog.clone(),
        ),
        |(state, premoves, catalog)| premoves.project(state, catalog),
    );
    use_camera_loop(CameraLoopInputs {
        camera: handles.camera.clone(),
        latest,
        canvas_ref: canvas_ref.clone(),
        cam_state: handles.cam_state.clone(),
        zoom_state: handles.zoom_state.clone(),
        globals: globals.clone(),
    });
    use_wheel_zoom(WheelZoomInputs {
        camera: handles.camera.clone(),
        canvas_ref: canvas_ref.clone(),
        input_blocked: has_match_result,
        camera_locked: reducer.camera_movement_disabled(),
        zoom_min: globals.camera_zoom_min,
        zoom_max: globals.camera_zoom_max,
        scroll_base: globals.scroll_zoom_base,
    });
    use_selection_cleanup(&selected, reducer);
    use_cancel_drag_on_match_end(&handles, has_match_result);

    let player_id = reducer.viewer_id();
    let camera = *handles.cam_state;
    let active_shop = ActiveShopQuery {
        predicted: &projection.state,
        premoves: &reducer.premoves,
        catalog: &reducer.catalog,
        player_id,
        camera,
        tile_size: globals.tile_size_px,
    }
    .resolve();

    let anims = use_piece_animations(reducer);
    use_board_render(BoardFrame {
        renderer,
        reducer: reducer.clone(),
        projection: projection.clone(),
        anims,
        active_shop: active_shop.clone(),
        selected: *selected,
        camera,
        zoom: *handles.zoom_state,
        canvas_size: window_size,
        tile_size_px: globals.tile_size_px,
    });

    let pointer = PointerContext {
        handles: handles.clone(),
        reducer: reducer.clone(),
        tx: props.tx.clone(),
        projection,
        tile_size_px: globals.tile_size_px,
    }
    .handlers();
    let touch = TouchContext {
        handles: handles.clone(),
        reducer: reducer.clone(),
        zoom_min: globals.camera_zoom_min,
        zoom_max: globals.camera_zoom_max,
    }
    .handlers(&pointer);
    let mouse_pos = |e: &MouseEvent| vec2(e.client_x() as f64, e.client_y() as f64);
    let on_mouse_down = pointer.start.reform(move |e: MouseEvent| InputStart {
        pos: mouse_pos(&e),
        is_right_click: e.button() == 2,
    });
    let on_mouse_move = pointer
        .moved
        .reform(move |e: MouseEvent| InputMove { pos: mouse_pos(&e) });
    let on_mouse_up = pointer.end.reform(move |e: MouseEvent| InputEnd {
        pos: mouse_pos(&e),
        is_right_click: e.button() == 2,
    });
    let unless_exempt = |callback: Callback<MouseEvent>| {
        Callback::from(move |e: MouseEvent| {
            if !is_ui_exempt_target(e.target()) {
                callback.emit(e);
            }
        })
    };

    let player_score = reducer
        .state
        .players
        .get(&player_id)
        .map_or_else(Score::zero, |player| player.score);
    let on_shop_buy = {
        let reducer = reducer.clone();
        let tx = props.tx.clone();
        let piece_id = active_shop.as_ref().map(|shop| shop.piece.id);
        Callback::from(move |item: usize| {
            if let Some(piece_id) = piece_id {
                reducer.dispatch(GameAction::QueueAction {
                    piece_id,
                    action: PieceAction::Buy {
                        item: ShopItemIndex(item),
                    },
                    tx: tx.clone(),
                });
            }
        })
    };
    let shop_menu = active_shop.as_ref().and_then(|shop| {
        let shop_config = reducer.catalog.shops.get(&shop.shop_id)?.clone();
        Some(html! {
            <ShopUI
                player_score={player_score}
                price_vars={reducer.catalog.price_vars(&reducer.state, player_id)}
                piece_on_shop={shop.piece.clone()}
                shop_config={shop_config}
                on_buy={on_shop_buy}
            />
        })
    });
    let countdown_secs = reducer
        .move_unlock_at
        .map(|unlock_at| unlock_at - reducer.server_now())
        .filter(|remaining| *remaining > DurationMs::zero())
        .map(|remaining| remaining.as_u64().div_ceil(1000));

    html! {
        <div class="fixed inset-0 bg-slate-100 overflow-hidden touch-none"
             onmousedown={unless_exempt(on_mouse_down)}
             onmousemove={unless_exempt(on_mouse_move)}
             onmouseup={unless_exempt(on_mouse_up)}
             onmouseleave={mouse_leave_handler(handles)}
             oncontextmenu={Callback::from(|e: MouseEvent| e.prevent_default())}
             ontouchstart={touch.start}
             ontouchmove={touch.moved}
             ontouchend={touch.end}
        >
            <canvas
                ref={canvas_ref}
                width={window_size.x.to_string()}
                height={window_size.y.to_string()}
                class="block w-full h-full"
            />

            if let Some(remaining_secs) = countdown_secs {
                <div
                    class="pointer-events-none absolute left-1/2 top-1/2"
                    style="transform: translate(-50%, -50%); color: #fde047; font-size: clamp(42px, 9vw, 88px); font-weight: 900; line-height: 1; text-shadow: 0 2px 0 rgba(0,0,0,0.55); z-index: 120;"
                >
                    {remaining_secs.to_string()}
                </div>
            }

            <div class="absolute top-4 left-4 flex flex-col gap-2 pointer-events-none">
                <div class="bg-white/90 backdrop-blur px-3 py-1.5 rounded-lg border border-slate-200"
                     style="box-shadow: 4px 4px 0 rgba(15, 23, 42, 0.2);">
                    <span class="text-xs font-bold text-slate-500 uppercase tracking-wider block">{"Score"}</span>
                    <span class="text-xl font-black text-slate-800 tabular-nums">{player_score.to_string()}</span>
                </div>
            </div>

            { shop_menu.unwrap_or_default() }

            <GameChat
                reducer={reducer.clone()}
                tx={props.tx.clone()}
                globals={globals.clone()}
                player_name={props.player_name.clone()}
            />
        </div>
    }
}

/// Tracks the browser window size.
#[hook]
fn use_window_size() -> Vec2 {
    let size = use_state(read_window_size);
    {
        let size = size.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("window available");
            let listener =
                EventListener::new(&window, "resize", move |_| size.set(read_window_size()));
            || drop(listener)
        });
    }
    *size
}

/// Creates the canvas renderer once the canvas exists, recreating it when configs change.
#[hook]
fn use_renderer(
    canvas_ref: &NodeRef,
    reducer: &UseReducerHandle<GameStateReducer>,
) -> Option<Renderer> {
    let renderer = use_state(|| None::<Renderer>);
    {
        let renderer = renderer.clone();
        use_effect_with(
            (canvas_ref.clone(), reducer.catalog.clone()),
            move |(canvas_ref, catalog)| {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    renderer.set(Renderer::new(canvas, catalog.clone()));
                }
            },
        );
    }
    (*renderer).clone()
}

/// Drops the selection when the session changes or the selected piece is no longer ours.
#[hook]
fn use_selection_cleanup(
    selected: &UseStateHandle<Option<PieceId>>,
    reducer: &UseReducerHandle<GameStateReducer>,
) {
    {
        let selected = selected.clone();
        use_effect_with((reducer.player_id, reducer.has_match_result()), move |_| {
            selected.set(None)
        });
    }
    let player_id = reducer.viewer_id();
    let still_ours = selected.is_none_or(|piece_id| {
        reducer
            .state
            .pieces
            .get(&piece_id)
            .is_some_and(|piece| piece.owner_id == Some(player_id))
    });
    let selected = selected.clone();
    use_effect_with(still_ours, move |still_ours| {
        if !still_ours {
            selected.set(None);
        }
    });
}

/// Stops ongoing pans and zooms when the match ends so the end-screen camera can take over.
#[hook]
fn use_cancel_drag_on_match_end(handles: &ViewHandles, has_match_result: bool) {
    let handles = handles.clone();
    use_effect_with(has_match_result, move |has_match_result| {
        if *has_match_result {
            handles.cancel_drag();
        }
    });
}
