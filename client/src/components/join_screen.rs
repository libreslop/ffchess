//! Join flow UI for selecting mode, name, and kit.

use crate::canvas::color::piece_icon_colors;
use crate::ui_state::{CooldownSeconds, JoinStep};
use crate::utils::{event_targets_text_input, is_mobile};
use common::models::{GameModeClientConfig, ModeSummary};
use common::types::{KitId, ModeId, PLAYER_COLORS};
use gloo_events::EventListener;
use gloo_net::http::Request;
use rand::seq::SliceRandom;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

/// Height kept clear at the top and bottom of the menu: the chat's bottom offset plus its
/// input row and three most recent lines (12px text at line-height 1).
///
/// The chat renders beneath the menu, so when the window is too short the menu first gives up
/// the space above it, then the mode list scrolls, instead of the menu covering the chat.
const CHAT_CLEARANCE: &str = "12px + 4 * 12px";

/// Fixed height of one mode list row, divider included, so the visible row count is exact.
const MODE_ROW_HEIGHT_PX: f64 = 28.0;

/// Number of mode rows the list keeps visible however short the window gets.
///
/// The half row hints that the list scrolls, since its scrollbar is hidden.
const MIN_VISIBLE_MODE_ROWS: f64 = 2.5;

/// Hides the mode list scrollbar and focus outlines; the list still scrolls by wheel, touch,
/// and the arrow keys moving the selection.
const MODE_LIST_STYLE: &str = ".ff-mode-list { scrollbar-width: none; } .ff-mode-list::-webkit-scrollbar { display: none; } .ff-mode-list, .ff-mode-list * { outline: none; user-select: none; -webkit-user-select: none; }";

/// Properties for the join screen inputs.
#[derive(Properties, PartialEq)]
pub struct JoinScreenProps {
    pub player_name: String,
    pub on_name_input: Callback<InputEvent>,
    pub on_name_submit: Callback<SubmitEvent>,
    pub landing_cooldown: CooldownSeconds,
    pub join_step: JoinStep,
    pub on_join: Callback<KitId>,
    pub error: Option<common::protocol::GameError>,
    pub queue_status: Option<crate::reducer::QueueStatus>,
    pub is_loading: bool,
    pub mode: Option<GameModeClientConfig>,
    pub mode_options: Vec<ModeSummary>,
    pub selected_mode_id: ModeId,
    pub on_select_mode: Callback<ModeId>,
    pub on_cycle_mode: Callback<i32>,
    /// Modes that can be played while waiting in the queue.
    pub quick_play_modes: Vec<ModeSummary>,
    pub on_quick_play: Callback<ModeId>,
    /// Piece icons, by file stem under `assets/pieces`, that may sit beside the title.
    pub title_pieces: Vec<String>,
}

#[function_component(JoinScreen)]
/// Renders the join screen form and kit selection UI.
///
/// `props` supplies the join state and callbacks. Returns rendered HTML.
pub fn join_screen(props: &JoinScreenProps) -> Html {
    let mobile = is_mobile();
    let is_disabled = props.is_loading || props.queue_status.is_some();
    let vertical_padding = if mobile { "20px" } else { "30px" };
    let mode_list_min_height =
        (props.mode_options.len() as f64).min(MIN_VISIBLE_MODE_ROWS) * MODE_ROW_HEIGHT_PX;
    let selected_mode = props
        .mode_options
        .iter()
        .position(|mode| mode.id == props.selected_mode_id);
    let quick_play_active = props.queue_status.is_some() && !props.quick_play_modes.is_empty();
    let quick_play_highlight = use_quick_play_keys(
        quick_play_active,
        &props.quick_play_modes,
        &props.on_quick_play,
    );

    html! {
        <>
            <style>{MODE_LIST_STYLE}</style>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
            // Equal spacers centre the menu; when space runs out the top one collapses first,
            // while the bottom one keeps the chat clear.
            <div style="position: absolute; inset: 0; z-index: 100; display: flex; flex-direction: column; align-items: center; pointer-events: none;">
            <div style={format!("flex: 1 1000 calc({CHAT_CLEARANCE}); min-height: 0;")}></div>
            <div style={format!(
                "flex: 0 1 auto; min-height: 0; pointer-events: auto; text-align: center; width: 90%; max-width: 400px; padding: {} {}; display: flex; flex-direction: column;",
                vertical_padding, if mobile { "10px" } else { "30px" }
            )}>
                if props.join_step.is_enter_name() {
                    <Title pieces={props.title_pieces.clone()} pixel_px={if mobile { 3 } else { 4 }} />
                    <ModeList
                        modes={props.mode_options.clone()}
                        selected={selected_mode}
                        on_select={props.on_select_mode.clone()}
                        style={format!("flex: 0 1 auto; min-height: {mode_list_min_height}px; margin-bottom: 12px; max-height: 220px; overflow-y: auto; border: 2px solid #cbd5e1; background: transparent;")}
                    />
                    <form style="flex-shrink: 0;" onsubmit={props.on_name_submit.clone()}>
                        <div style="display: flex; flex-direction: column; gap: 12px; align-items: center;">
                            <input type="text" name="player_name" value={props.player_name.clone()} oninput={props.on_name_input.clone()} placeholder="This is a tale of..." autofocus=true
                                autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
                                onkeydown={{
                                    let on_cycle_mode = props.on_cycle_mode.clone();
                                    Callback::from(move |e: KeyboardEvent| {
                                        if e.key() == "ArrowUp" {
                                            e.prevent_default();
                                            on_cycle_mode.emit(-1);
                                        } else if e.key() == "ArrowDown" {
                                            e.prevent_default();
                                            on_cycle_mode.emit(1);
                                        }
                                    })
                                }}
                                style="padding: 8px 20px; border-radius: 0; border: 2px solid #cbd5e1; width: 100%; box-sizing: border-box; font-size: 1.2em; outline: none; background: #fff; text-align: center;"/>
                            <button type="submit" disabled={props.landing_cooldown.is_active() || props.is_loading}
                                style={format!("padding: 6px 40px; background: {}; color: #fff; border: 3px solid {}; border-radius: 0; font-weight: 900; cursor: {}; font-size: 1.2em; width: auto; text-transform: uppercase; letter-spacing: 1px;",
                                    if props.landing_cooldown.is_active() || props.is_loading { "#94a3b8" } else { "#3b82f6" },
                                    if props.landing_cooldown.is_active() || props.is_loading { "#64748b" } else { "#1e3a8a" },
                                    if props.landing_cooldown.is_active() || props.is_loading { "not-allowed" } else { "pointer" })}>
                                if props.landing_cooldown.is_active() {
                                    {format!("Wait ({}s)", props.landing_cooldown.as_u32())}
                                } else {
                                    {"Play!"}
                                }
                            </button>
                        </div>
                    </form>
                } else {
                    <div style="animation: fadeIn 0.3s ease-out; display: flex; flex-direction: column; align-items: center;">
                        if props.queue_status.is_none() {
                            <h3 style="color: #fff; margin: 0 0 16px; text-transform: uppercase; letter-spacing: 2px; text-shadow: 2px 2px 0 rgba(0,0,0,0.5);">{"CHOOSE YOUR ARMY"}</h3>
                        }

                        if let Some(error) = &props.error {
                            <div style="margin-bottom: 16px; color: #ef4444; background: rgba(255,255,255,0.9); padding: 6px 16px; border-radius: 4px; font-weight: bold;">
                                { error.to_string() }
                            </div>
                        }

                        if let Some(queue) = &props.queue_status {
                            // The quick-play list hangs below the message out of flow, so the
                            // message alone is what gets centred on screen.
                            <div style="position: relative; width: 100%;">
                            // 20px is the font's native size, where its pixels land on whole screen pixels.
                            <div style="color: #e2e8f0; font-size: 20px; text-shadow: 2px 2px 0 rgba(0,0,0,0.5);">
                                { queue_waiting_message(queue) }
                            </div>
                            if !props.quick_play_modes.is_empty() {
                                <div data-testid="quick-play-list" style="position: absolute; top: 100%; left: 0; right: 0; margin-top: 56px;">
                                    <div style="color: rgba(255,255,255,0.8); font-size: 0.85em; font-weight: 700; text-transform: uppercase; letter-spacing: 2px; margin-bottom: 8px;">
                                        {"Quick play"}
                                    </div>
                                    <ModeList
                                        modes={props.quick_play_modes.clone()}
                                        selected={quick_play_highlight}
                                        on_select={props.on_quick_play.clone()}
                                        style="max-height: 220px; overflow-y: auto; border: 2px solid #cbd5e1;"
                                    />
                                </div>
                            }
                            </div>
                        } else {
                            <div style="display: flex; gap: 12px; width: 100%; justify-content: center; flex-wrap: wrap;">
                                if let Some(mode) = &props.mode {
                                    { for mode.kits.iter().enumerate().map(|(idx, kit)| {
                                        let kit_name = kit.name.clone();
                                        let on_click = props.on_join.reform(move |_| kit_name.clone());
                                        html! {
                                            <button
                                                disabled={is_disabled}
                                                onclick={on_click}
                                                style={format!(
                                                    "width: 120px; height: 120px; position: relative; padding: 10px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; transition: all 0.2s; opacity: {}; display: flex; flex-direction: column; align-items: center; justify-content: center;",
                                                    if is_disabled { "not-allowed" } else { "pointer" },
                                                    if is_disabled { "0.5" } else { "1.0" }
                                                )}
                                            >
                                                <span style="font-weight: 900; font-size: 1.1em; line-height: 1.1; text-align: center; margin-bottom: 8px;">{ kit.name.as_ref().to_uppercase() }</span>
                                                <span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1; line-height: 1.2; text-align: center; display: -webkit-box; -webkit-line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden;">{ &kit.description }</span>
                                                <span style="position: absolute; bottom: 6px; right: 10px; font-size: 1em; font-weight: 900; opacity: 0.5;">
                                                    { idx + 1 }
                                                </span>
                                            </button>
                                        }
                                    })}
                                } else {
                                    <div style="color: #fff;">{"Loading kits..."}</div>
                                }
                        </div>
                            }
                    </div>
                }
            </div>
            <div style={format!("flex: 1 1000 calc({CHAT_CLEARANCE}); min-height: calc({CHAT_CLEARANCE});")}></div>
            </div>
            if props.join_step.is_enter_name() {
                <a href="https://github.com/libreslop/ffchess" target="_blank" rel="noopener noreferrer"
                   style="position: absolute; top: 12px; left: 12px; z-index: 100; opacity: 0.5; display: block;">
                    <img src="/assets/static/img/github.svg" alt="GitHub" style="width: 24px; height: 24px; filter: invert(1); display: block;" />
                </a>
            }
        </>
    }
}

/// "FFCHESS" in Upheaval, one unit per font pixel, filled with `${color}` inside a black
/// 1px outline.
const TITLE_SVG: &str = include_str!("../../../assets/static/img/title.svg");

/// Width and height of the title SVG, in font pixels.
const TITLE_SIZE: (u32, u32) = (85, 12);

/// Width and height of a piece icon, in pixels.
const PIECE_ICON_SIZE: u32 = 16;

/// Shadow cast by the title and its piece icon.
const TITLE_SHADOW: &str = "drop-shadow(3px 3px 0 rgba(0,0,0,0.6))";

/// Properties for the title.
#[derive(Properties, PartialEq)]
struct TitleProps {
    /// Piece icons, by file stem under `assets/pieces`, one of which sits beside the title.
    pieces: Vec<String>,
    /// On-screen size of one title and icon pixel.
    pixel_px: u32,
}

#[function_component(Title)]
/// Renders the title beside a random piece from `pieces`, both in one random player color.
///
/// The title and icon share one pixel grid, centered on each other; their heights differ by an
/// even number of pixels, so centering keeps the grids aligned.
fn title(props: &TitleProps) -> Html {
    let pieces = props.pieces.clone();
    let (piece, color) = (*use_state(move || {
        let mut rng = rand::thread_rng();
        let piece = pieces.choose(&mut rng).cloned();
        let color = *PLAYER_COLORS
            .choose(&mut rng)
            .expect("player palette is not empty");
        (piece, color)
    }))
    .clone();
    let icon_src = use_state(|| None::<String>);
    {
        let icon_src = icon_src.clone();
        use_effect_with(piece.clone(), move |piece| {
            if let Some(piece) = piece.clone() {
                spawn_local(async move {
                    let path = format!("/assets/pieces/{piece}.svg");
                    let Ok(response) = Request::get(&path).send().await else {
                        return;
                    };
                    if response.ok()
                        && let Ok(template) = response.text().await
                    {
                        icon_src.set(Some(tinted_piece_data_url(&template, color)));
                    }
                });
            }
        });
    }

    let px = props.pixel_px;
    let title_src =
        svg_data_url(&TITLE_SVG.replace("${color}", &piece_icon_colors(color).secondary));
    let image_style =
        format!("display: block; image-rendering: pixelated; filter: {TITLE_SHADOW};");
    let icon_side = PIECE_ICON_SIZE * px;
    html! {
        <h1 style={format!("flex-shrink: 0; margin: 0 0 {}px; display: flex; align-items: center; justify-content: center; gap: {}px;", 6 * px, px)}>
            // The icon's space is held while it loads, so the title does not jump sideways.
            if piece.is_some() {
                <div style={format!("width: {icon_side}px; height: {icon_side}px; flex-shrink: 0;")}>
                    if let Some(src) = &*icon_src {
                        <img src={src.clone()} alt="" style={format!("{image_style} width: 100%; height: 100%;")} />
                    }
                </div>
            }
            <img src={title_src} alt="FFCHESS" style={format!(
                "{image_style} width: {}px; height: {}px;",
                TITLE_SIZE.0 * px, TITLE_SIZE.1 * px
            )} />
        </h1>
    }
}

/// Returns a data URL of the piece SVG `template` tinted with the player color `color`.
fn tinted_piece_data_url(template: &str, color: &str) -> String {
    let colors = piece_icon_colors(color);
    svg_data_url(
        &template
            .replace("${primary}", &colors.primary)
            .replace("${secondary}", &colors.secondary)
            .replace("${tertiary}", &colors.tertiary),
    )
}

/// Returns a data URL holding `svg`.
fn svg_data_url(svg: &str) -> String {
    let encoded = js_sys::encode_uri_component(svg)
        .as_string()
        .unwrap_or_default();
    format!("data:image/svg+xml;utf8,{encoded}")
}

/// Properties for a scrolling list of modes.
#[derive(Properties, PartialEq)]
struct ModeListProps {
    modes: Vec<ModeSummary>,
    /// Index of the highlighted row, kept fully scrolled into view.
    selected: Option<usize>,
    on_select: Callback<ModeId>,
    style: AttrValue,
}

#[function_component(ModeList)]
/// Renders a list of mode rows, scrolling just far enough to show the selected row whole.
fn mode_list(props: &ModeListProps) -> Html {
    let list_ref = use_node_ref();
    {
        let list_ref = list_ref.clone();
        use_effect_with(props.selected, move |selected| {
            if let (Some(index), Some(list)) = (*selected, list_ref.cast::<web_sys::Element>()) {
                scroll_row_into_view(&list, index);
            }
        });
    }

    html! {
        // Taking no focus keeps typing in the name input and leaves no focus outline behind.
        <div ref={list_ref} class="ff-mode-list" tabindex="-1" style={props.style.clone()}
            onmousedown={Callback::from(|e: MouseEvent| e.prevent_default())}>
            { for props.modes.iter().enumerate().map(|(index, mode)| {
                mode_row(mode, props.selected == Some(index), &props.on_select)
            })}
        </div>
    }
}

/// Clamps the scroll of `list` so the row at `index` is fully visible, moving it as little
/// as possible.
fn scroll_row_into_view(list: &web_sys::Element, index: usize) {
    let row_height = MODE_ROW_HEIGHT_PX as i32;
    let row_top = index as i32 * row_height;
    let row_bottom = row_top + row_height;
    let scroll_top = list
        .scroll_top()
        .max(row_bottom - list.client_height())
        .min(row_top);
    list.set_scroll_top(scroll_top);
}

/// Moves a quick-play highlight with the arrow keys and plays the highlighted mode on Enter,
/// while `active`.
///
/// Returns the highlighted row while active, which starts on the first row each time the
/// list becomes active.
#[hook]
fn use_quick_play_keys(
    active: bool,
    modes: &[ModeSummary],
    on_quick_play: &Callback<ModeId>,
) -> Option<usize> {
    let highlighted = use_state(|| 0);
    {
        let highlighted = highlighted.clone();
        use_effect_with(active, move |_| highlighted.set(0));
    }
    {
        let highlighted = highlighted.clone();
        use_effect_with(
            (active, modes.to_vec(), *highlighted, on_quick_play.clone()),
            move |(active, modes, current, on_quick_play)| {
                let listener = active.then(|| {
                    let modes = modes.clone();
                    let current = *current;
                    let on_quick_play = on_quick_play.clone();
                    let window = web_sys::window().expect("window available");
                    EventListener::new(&window, "keydown", move |e| {
                        let Some(e) = e.dyn_ref::<web_sys::KeyboardEvent>() else {
                            return;
                        };
                        if event_targets_text_input(e) {
                            return;
                        }
                        match e.key().as_str() {
                            key @ ("ArrowUp" | "ArrowDown") => {
                                e.prevent_default();
                                highlighted.set(step_highlight(
                                    current,
                                    key == "ArrowUp",
                                    modes.len(),
                                ));
                            }
                            "Enter" => {
                                if let Some(mode) = modes.get(current) {
                                    // Also stops Enter from clicking a focused row a second time.
                                    e.prevent_default();
                                    on_quick_play.emit(mode.id.clone());
                                }
                            }
                            _ => {}
                        }
                    })
                });
                move || drop(listener)
            },
        );
    }
    active.then_some(*highlighted)
}

/// Returns the row an arrow key moves the highlight to in a list of `len` rows, stopping at
/// either end.
fn step_highlight(current: usize, up: bool, len: usize) -> usize {
    let last = len.saturating_sub(1);
    if up {
        current.saturating_sub(1).min(last)
    } else {
        (current + 1).min(last)
    }
}

/// Renders one row of a mode list, emitting the mode id to `on_select` when clicked.
fn mode_row(mode: &ModeSummary, selected: bool, on_select: &Callback<ModeId>) -> Html {
    let id = mode.id.clone();
    let on_click = on_select.reform(move |_: MouseEvent| id.clone());
    html! {
        <button
            onclick={on_click}
            tabindex="-1"
            style={format!(
                "display: flex; width: 100%; justify-content: space-between; align-items: center; text-align: left; box-sizing: border-box; height: {}px; padding: 0 12px; border: none; border-bottom: 1px solid rgba(255,255,255,0.08); background: {}; color: #fff; font-weight: 500; cursor: pointer;",
                MODE_ROW_HEIGHT_PX,
                if selected { "rgba(255,255,255,0.08)" } else { "transparent" },
            )}
        >
            <span>{ mode.display_name.clone() }</span>
            <span style={format!(
                "font-variant-numeric: tabular-nums; color: {};",
                if mode.players.as_u32() >= 1 { "#22c55e" } else { "rgba(255,255,255,0.8)" }
            )}>
                { mode_population_label(mode) }
            </span>
        </button>
    }
}

/// Returns the mode population label shown in the mode list row.
fn mode_population_label(mode: &ModeSummary) -> String {
    if mode.is_queue_mode() {
        format!("{} playing", mode.players)
    } else {
        format!("{}/{}", mode.players, mode.max_players)
    }
}

/// Returns the queue waiting sentence with proper singular/plural grammar.
fn queue_waiting_message(queue: &crate::reducer::QueueStatus) -> String {
    let missing = queue.players_needed();
    let noun = if missing.is_one() {
        "player"
    } else {
        "players"
    };
    format!("Waiting for {} more {}", missing, noun)
}
