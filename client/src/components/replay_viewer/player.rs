//! Playback of a loaded replay on its own board.

use super::controls::ReplayControls;
use super::history::HistoryStep;
use super::hold::{StepRepeater, use_step_repeater};
use super::loaded::{LoadedReplay, SharedReplay};
use super::move_list::{MoveList, MoveNav};
use super::playback::{PlaybackButton, PlaybackCommand, PlaybackState};
use super::share_menu::ShareTarget;
use super::style::REPLAY_CSS;
use super::title_bar::ReplayTitleBar;
use super::video::VideoSettings;
use crate::app::GlobalClientConfig;
use crate::components::{GameView, Leaderboard, LeaveButton};
use crate::reducer::{GameAction, GameStateReducer, InitPayload};
use crate::ui_state::Backdrop;
use crate::utils::{SessionStore, performance_now};
use common::types::{PlayerId, SessionSecret};
use gloo_events::{EventListener, EventListenerOptions, EventListenerPhase};
use gloo_timers::callback::{Interval, Timeout};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::prelude::*;

/// How often the scrub bar follows a playing replay, in milliseconds.
const FOLLOW_INTERVAL_MS: u32 = 50;
/// How far the wheel turns for one step through the move history, in pixels.
const WHEEL_STEP_PX: f64 = 100.0;
/// Pixels counted for each line the wheel turns, when it reports lines.
const WHEEL_LINE_PX: f64 = 40.0;
/// How long a button stays lit after the wheel pressed it, in milliseconds.
const WHEEL_PRESS_MS: u32 = 120;

/// Properties for the replay player.
#[derive(Properties, PartialEq)]
pub(super) struct ReplayPlayerProps {
    pub(super) replay: SharedReplay,
    pub(super) globals: GlobalClientConfig,
    pub(super) on_close: Callback<()>,
}

#[function_component(ReplayPlayer)]
/// Plays a replay back on a board of its own, watched like a spectator watches a live match.
pub(super) fn replay_player(props: &ReplayPlayerProps) -> Html {
    let replay = props.replay.0.clone();
    let board = use_reducer(GameStateReducer::default);
    let playback = {
        let timeline = replay.timeline.clone();
        let history = replay.history.0.clone();
        use_reducer(move || PlaybackState::new(timeline, history))
    };
    let now_local = use_state(performance_now);
    let pressed = use_state(|| None::<PlaybackButton>);
    let board_ref = use_node_ref();

    use_replay_board(&board, &replay);
    let timeline = &replay.timeline;
    let position = playback.position(*now_local);
    let index = timeline.index_at(position);
    {
        let board = board.clone();
        let replay = replay.clone();
        use_effect_with(index, move |&index| {
            let state = replay.timeline.board(index).clone();
            board.dispatch(GameAction::ShowReplayFrame(Box::new(state)));
        });
    }
    {
        let board = board.dispatcher();
        use_effect_with(position, move |&position| {
            board.dispatch(GameAction::SetReplayTime(position));
        });
    }
    use_follow_playback(playback.playback.is_playing(), now_local.clone());
    {
        let ran_out = playback.playback.ran_out(timeline, *now_local);
        let playback = playback.dispatcher();
        use_effect_with(ran_out, move |&ran_out| {
            if ran_out {
                playback.dispatch(PlaybackCommand::Pause);
            }
        });
    }
    let repeater = use_step_repeater(playback.dispatcher(), pressed.setter());
    use_playback_keys(
        playback.dispatcher(),
        pressed.setter(),
        repeater.clone(),
        props.on_close.clone(),
    );
    // Where the board can be zoomed, the wheel zooms it instead.
    let wheel_steps = replay.header.mode.disable_camera_movement;
    use_wheel_steps(
        &board_ref,
        wheel_steps,
        playback.dispatcher(),
        pressed.setter(),
    );

    let show_scores = board.mode.as_ref().is_none_or(|mode| mode.show_scoreboard);
    let on_command = |command: fn() -> PlaybackCommand| {
        let playback = playback.dispatcher();
        Callback::from(move |_| playback.dispatch(command()))
    };
    let on_seek = {
        let playback = playback.dispatcher();
        Callback::from(move |elapsed| playback.dispatch(PlaybackCommand::Seek(elapsed)))
    };
    let on_flip = {
        let board = board.clone();
        Callback::from(move |_| {
            board.dispatch(GameAction::SetOrientation(board.orientation().flipped()));
        })
    };
    let share = ShareTarget {
        replay: props.replay.clone(),
        orientation: board.orientation(),
        video: VideoSettings::from(&props.globals),
    };
    // Kept across renders, so the move list only re-renders when the shown move changes.
    let on_show = {
        let playback = playback.dispatcher();
        use_callback((), move |index, _| {
            playback.dispatch(PlaybackCommand::Show(index))
        })
    };

    html! {
        <div data-testid="replay-viewer" class="replay-layout">
            <style>{REPLAY_CSS}</style>
            <div class="replay-board" ref={board_ref}>
                <GameView
                    reducer={board.clone()}
                    tx={None}
                    globals={props.globals.clone()}
                    player_name={String::new()}
                />
            </div>
            <div class="replay-hud">
                <Leaderboard
                    players={board.state.players.values().cloned().collect::<Vec<_>>()}
                    self_id={PlayerId::nil()}
                    show_scores={show_scores}
                    spectators={Vec::<String>::new()}
                />
                <LeaveButton
                    on_leave={props.on_close.reform(|_| ())}
                    backdrop={Backdrop::LightBoard}
                    title="Close replay"
                />
            </div>
            <div class="replay-panel" data-ui-exempt="true">
                <ReplayTitleBar share={share} on_flip={on_flip} />
                <MoveList
                    history={replay.history.clone()}
                    icons={replay.icons.clone()}
                    current={playback.current_entry(*now_local)}
                    on_show={on_show}
                />
                <ReplayControls
                    elapsed={timeline.elapsed(position)}
                    duration={timeline.duration()}
                    playing={playback.playback.is_playing()}
                    speed_label={playback.playback.speed().label()}
                    pressed={*pressed}
                    on_toggle={on_command(|| PlaybackCommand::Toggle)}
                    on_seek={on_seek}
                    on_speed={on_command(|| PlaybackCommand::NextSpeed)}
                />
                <MoveNav repeater={repeater} pressed={*pressed} />
            </div>
        </div>
    }
}

/// Shows the rules and first board of the replay once, watched as a spectator, from the side
/// of the board the viewer played the match on.
#[hook]
fn use_replay_board(board: &UseReducerHandle<GameStateReducer>, replay: &Rc<LoadedReplay>) {
    let board = board.clone();
    let replay = replay.clone();
    use_effect_with((), move |_| {
        board.dispatch(GameAction::LoadReplay(Box::new(InitPayload {
            player_id: PlayerId::nil(),
            session_secret: SessionSecret::nil(),
            move_unlock_at: None,
            state: replay.timeline.board(0).clone(),
            mode: replay.header.mode.clone(),
            catalog: replay.header.catalog.clone(),
            chat_room_key: String::new(),
            chat_history: Vec::new(),
            spectators: Vec::new(),
        })));
        let (player_id, _) = SessionStore::new(&replay.header.mode.id).load();
        let orientation =
            player_id.and_then(|player_id| replay.timeline.orientation_for(player_id));
        board.dispatch(GameAction::SetOrientation(orientation.unwrap_or_default()));
    });
}

/// Re-renders regularly while the replay plays, so the board and scrub bar follow it.
#[hook]
fn use_follow_playback(playing: bool, now_local: UseStateHandle<f64>) {
    use_effect_with(playing, move |&playing| {
        now_local.set(performance_now());
        let interval = playing
            .then(|| Interval::new(FOLLOW_INTERVAL_MS, move || now_local.set(performance_now())));
        move || drop(interval)
    });
}

/// Returns the command a key gives playback and the button it presses.
fn key_binding(key: &str) -> Option<(PlaybackCommand, PlaybackButton)> {
    let step = |step| (PlaybackCommand::Step(step), PlaybackButton::Step(step));
    Some(match key {
        " " => (PlaybackCommand::Toggle, PlaybackButton::Toggle),
        "ArrowLeft" => step(HistoryStep::PreviousMove),
        "ArrowRight" => step(HistoryStep::NextMove),
        "ArrowUp" => step(HistoryStep::PreviousRow),
        "ArrowDown" => step(HistoryStep::NextRow),
        // `=` and `_` share their keys with `+` and `-`, so neither needs Shift.
        "+" | "=" => (PlaybackCommand::Faster, PlaybackButton::Speed),
        "-" | "_" => (PlaybackCommand::Slower, PlaybackButton::Speed),
        _ => return None,
    })
}

/// Controls playback with the keyboard: Space plays and pauses wherever focus is, the arrows
/// step through the move history, again and again while held, `+` and `-` change the speed,
/// and Escape closes the viewer. The button a key stands for stays lit while the key is held.
///
/// Arrows repeat at the pace of the step buttons rather than of the keyboard, so the keyboard's
/// own repeats are ignored.
///
/// The listener runs before any other and keeps every key to itself, so the screens beneath
/// the viewer do not react to keys meant for it.
#[hook]
fn use_playback_keys(
    playback: UseReducerDispatcher<PlaybackState>,
    pressed: UseStateSetter<Option<PlaybackButton>>,
    repeater: StepRepeater,
    on_close: Callback<()>,
) {
    use_effect_with((), move |_| {
        let window = web_sys::window().expect("window available");
        // Not passive, so the keys can be kept from scrolling and from pressing buttons.
        let options = EventListenerOptions {
            phase: EventListenerPhase::Capture,
            passive: false,
        };
        let press = {
            let pressed = pressed.clone();
            let repeater = repeater.clone();
            EventListener::new_with_options(&window, "keydown", options, move |event| {
                let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                event.stop_propagation();
                if event.key() == "Escape" {
                    event.prevent_default();
                    on_close.emit(());
                    return;
                }
                let Some((command, button)) = key_binding(&event.key()) else {
                    return;
                };
                event.prevent_default();
                if event.repeat() {
                    return;
                }
                match command {
                    PlaybackCommand::Step(step) => repeater.press(step),
                    command => {
                        playback.dispatch(command);
                        pressed.set(Some(button));
                    }
                }
            })
        };
        // A focused button clicks itself when Space is released, which would press the last
        // button clicked on top of the play and pause done on the key going down.
        let release = {
            let repeater = repeater.clone();
            EventListener::new_with_options(&window, "keyup", options, move |event| {
                let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                let Some((command, _)) = key_binding(&event.key()) else {
                    return;
                };
                event.prevent_default();
                match command {
                    PlaybackCommand::Step(step) => repeater.release(step),
                    _ => pressed.set(None),
                }
            })
        };
        // Keys let go of while the window is in the background never come up here.
        let leave = EventListener::new(&window, "blur", move |_| repeater.release_any());
        move || drop((press, release, leave))
    });
}

/// Steps through the move history with the mouse wheel over the board when `enabled`, down to
/// the next move and up to the previous one like the arrow keys, instead of zooming the board.
#[hook]
fn use_wheel_steps(
    board_ref: &NodeRef,
    enabled: bool,
    playback: UseReducerDispatcher<PlaybackState>,
    pressed: UseStateSetter<Option<PlaybackButton>>,
) {
    use_effect_with((board_ref.clone(), enabled), move |(board_ref, enabled)| {
        // Caught on the way down to the board, so the board never sees it.
        let options = EventListenerOptions {
            phase: EventListenerPhase::Capture,
            passive: false,
        };
        let board = board_ref
            .cast::<web_sys::HtmlElement>()
            .filter(|_| *enabled);
        let listener = board.map(|board| {
            let mut turned = 0.0;
            let mut unpress: Option<Timeout> = None;
            EventListener::new_with_options(&board, "wheel", options, move |event| {
                let Some(event) = event.dyn_ref::<web_sys::WheelEvent>() else {
                    return;
                };
                event.stop_propagation();
                event.prevent_default();
                let delta = match event.delta_mode() {
                    web_sys::WheelEvent::DOM_DELTA_LINE => event.delta_y() * WHEEL_LINE_PX,
                    web_sys::WheelEvent::DOM_DELTA_PAGE => event.delta_y() * WHEEL_STEP_PX,
                    _ => event.delta_y(),
                };
                // Turning back forgets what was turned the other way.
                if turned * delta < 0.0 {
                    turned = 0.0;
                }
                turned += delta;
                if turned.abs() < WHEEL_STEP_PX {
                    return;
                }
                let step = if turned > 0.0 {
                    HistoryStep::NextMove
                } else {
                    HistoryStep::PreviousMove
                };
                turned = 0.0;
                playback.dispatch(PlaybackCommand::Step(step));
                pressed.set(Some(PlaybackButton::Step(step)));
                let pressed = pressed.clone();
                // Replacing the timeout cancels the one before, so the button stays lit
                // while the wheel keeps turning.
                unpress.replace(Timeout::new(WHEEL_PRESS_MS, move || pressed.set(None)));
            })
        });
        move || drop(listener)
    });
}
