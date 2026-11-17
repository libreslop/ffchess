//! Playback of a loaded replay on its own board.

use super::controls::ReplayControls;
use super::history::HistoryStep;
use super::move_list::{MoveList, SharedHistory};
use super::playback::{PlaybackCommand, PlaybackState};
use super::style::REPLAY_CSS;
use super::timeline::ReplayTimeline;
use crate::app::GlobalClientConfig;
use crate::components::{GameView, Leaderboard, LeaveButton};
use crate::reducer::{GameAction, GameStateReducer, InitPayload};
use crate::ui_state::Backdrop;
use crate::utils::performance_now;
use common::models::ReplayHeader;
use common::types::{PlayerId, SessionSecret};
use gloo_events::{EventListener, EventListenerOptions};
use gloo_timers::callback::Interval;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::prelude::*;

/// How often the scrub bar follows a playing replay, in milliseconds.
const FOLLOW_INTERVAL_MS: u32 = 50;

/// A replay ready to be played back.
pub(super) struct LoadedReplay {
    pub(super) header: ReplayHeader,
    pub(super) timeline: Rc<ReplayTimeline>,
    pub(super) history: SharedHistory,
}

/// Shared handle to a loaded replay, compared by identity so re-renders stay cheap.
#[derive(Clone)]
pub(super) struct SharedReplay(pub(super) Rc<LoadedReplay>);

impl PartialEq for SharedReplay {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

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
    use_playback_keys(playback.dispatcher(), props.on_close.clone());

    let show_scores = board.mode.as_ref().is_none_or(|mode| mode.show_scoreboard);
    let on_command = |command: fn() -> PlaybackCommand| {
        let playback = playback.dispatcher();
        Callback::from(move |_| playback.dispatch(command()))
    };
    let on_seek = {
        let playback = playback.dispatcher();
        Callback::from(move |elapsed| playback.dispatch(PlaybackCommand::Seek(elapsed)))
    };
    // Kept across renders, so the move list only re-renders when the shown move changes.
    let on_show = {
        let playback = playback.dispatcher();
        use_callback((), move |index, _| {
            playback.dispatch(PlaybackCommand::Show(index))
        })
    };
    let on_step = {
        let playback = playback.dispatcher();
        use_callback((), move |step, _| {
            playback.dispatch(PlaybackCommand::Step(step))
        })
    };

    html! {
        <div data-testid="replay-viewer" class="replay-layout">
            <style>{REPLAY_CSS}</style>
            <div class="replay-board">
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
                <div class="replay-title">{format!("Replay · {}", replay.header.mode.display_name)}</div>
                <MoveList
                    history={replay.history.clone()}
                    current={playback.current_entry(*now_local)}
                    on_show={on_show}
                    on_step={on_step}
                />
                <ReplayControls
                    elapsed={timeline.elapsed(position)}
                    duration={timeline.duration()}
                    captures={timeline.capture_times()}
                    playing={playback.playback.is_playing()}
                    speed_label={playback.playback.speed().label()}
                    on_toggle={on_command(|| PlaybackCommand::Toggle)}
                    on_seek={on_seek}
                    on_speed={on_command(|| PlaybackCommand::NextSpeed)}
                />
            </div>
        </div>
    }
}

/// Shows the rules and first board of the replay once, watched as a spectator.
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

/// Controls playback with the keyboard: Space plays and pauses, the arrows step through the
/// move history, and Escape closes the viewer.
///
/// The listener runs before any other and keeps every key to itself, so the screens beneath
/// the viewer do not react to keys meant for it.
#[hook]
fn use_playback_keys(playback: UseReducerDispatcher<PlaybackState>, on_close: Callback<()>) {
    use_effect_with((), move |_| {
        let window = web_sys::window().expect("window available");
        let options = EventListenerOptions::run_in_capture_phase();
        let listener = EventListener::new_with_options(&window, "keydown", options, move |event| {
            let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            event.stop_propagation();
            let command = match event.key().as_str() {
                " " => Some(PlaybackCommand::Toggle),
                "ArrowLeft" => Some(PlaybackCommand::Step(HistoryStep::PreviousMove)),
                "ArrowRight" => Some(PlaybackCommand::Step(HistoryStep::NextMove)),
                "ArrowUp" => Some(PlaybackCommand::Step(HistoryStep::PreviousRow)),
                "ArrowDown" => Some(PlaybackCommand::Step(HistoryStep::NextRow)),
                "Escape" => {
                    on_close.emit(());
                    None
                }
                _ => return,
            };
            event.prevent_default();
            if let Some(command) = command {
                playback.dispatch(command);
            }
        });
        move || drop(listener)
    });
}
