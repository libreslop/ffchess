//! UI rendering for the root application component.

use crate::app::GlobalClientConfig;
use crate::app::focus::Slot;
use crate::components::{
    DisconnectedScreen, EndScreen, EndScreenKind, ErrorToast, FatalNotification, GameView,
    JoinScreen, Leaderboard, LeaveButton, MatchBrowser, NestedQueue, QueueBanner, SpectateEnded,
};
use crate::reducer::{GameStateReducer, MsgSender};
use crate::ui_state::{Backdrop, CooldownSeconds, JoinStep};
use common::models::ModeSummary;
use common::protocol::SpectateTarget;
use common::types::{KitId, ModeId};
use yew::prelude::*;

/// Render inputs for the root application view.
pub struct AppViewProps {
    pub global_cfg: GlobalClientConfig,
    /// Connection slot of the shown game; switching it mounts a fresh game view.
    pub slot: Slot,
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: Option<MsgSender>,
    pub show_disconnected: bool,
    pub has_interacted: bool,
    pub is_joined: bool,
    pub force_join_overlay: bool,
    pub player_name: String,
    pub join_step: JoinStep,
    pub on_name_input: Callback<InputEvent>,
    pub on_name_submit: Callback<SubmitEvent>,
    pub on_join: Callback<KitId>,
    pub landing_cooldown: CooldownSeconds,
    pub is_joining: bool,
    pub mode_options: Vec<ModeSummary>,
    pub selected_mode_id: ModeId,
    pub on_select_mode: Callback<ModeId>,
    pub on_cycle_mode: Callback<i32>,
    pub on_rejoin: Callback<MouseEvent>,
    pub rejoin_cooldown: CooldownSeconds,
    pub on_leave: Callback<MouseEvent>,
    pub on_spectate: Callback<SpectateTarget>,
    pub on_menu_spectate: Callback<MouseEvent>,
    pub quick_play_modes: Vec<ModeSummary>,
    pub on_quick_play: Callback<ModeId>,
    pub nested_queue: Option<NestedQueue>,
}

/// Renders the root application UI.
pub fn render_app(props: AppViewProps) -> Html {
    let global_cfg = props.global_cfg;
    let reducer = props.reducer;
    let tx = props.tx;
    let has_match_result = reducer.has_match_result();
    let is_joined = props.is_joined;
    let is_victory = reducer.is_victory();
    let force_join_overlay = props.force_join_overlay;
    let player_id = reducer.viewer_id();
    let queue_status = reducer.queue_status.clone();
    let show_scoreboard = reducer
        .mode
        .as_ref()
        .is_none_or(|mode| mode.show_scoreboard);
    let victory = reducer.victory.clone();
    let is_spectating = reducer.is_spectating();
    // Every screen past the name entry form can be left.
    let on_main_menu =
        !is_joined && !is_spectating && queue_status.is_none() && props.join_step.is_enter_name();
    let leaderboard = html! {
        <Leaderboard
            players={reducer.state.players.values().cloned().collect::<Vec<_>>()}
            self_id={player_id}
            show_scores={show_scoreboard}
            spectators={reducer.spectators.clone()}
        />
    };

    html! {
        <div style="margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            <style>{"
                @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes simpleFadeIn { from { opacity: 0; } to { opacity: 1; } }
                @keyframes fadeInOut { 0% { opacity: 0; transform: translateY(20px); } 15% { opacity: 1; transform: translateY(0); } 85% { opacity: 1; transform: translateY(0); } 100% { opacity: 0; transform: translateY(-20px); } }
                @keyframes floatInFromTop { from { opacity: 0; transform: translateY(-20px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes floatOutToTop { from { opacity: 1; transform: translateY(0); } to { opacity: 0; transform: translateY(-20px); } }
                @keyframes floatInFromBottom { from { opacity: 0; transform: translateY(40px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
            "}</style>

            if let Some(sender) = tx.clone() {
                <GameView
                    key={props.slot.as_str()}
                    reducer={reducer.clone()}
                    tx={sender}
                    globals={global_cfg.clone()}
                    player_name={props.player_name.clone()}
                />
            } else if !props.show_disconnected || !props.has_interacted {
                <div style="position: absolute; inset: 0; background: #f0f2f5; display: flex; align-items: center; justify-content: center; z-index: 200;">
                    <div style="text-align: center;">
                        <h2 style="color: #64748b;">{"Connecting to server..."}</h2>
                        <div style="width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top: 4px solid #2563eb; border-radius: 50%; margin: 20px auto; animation: spin 1s linear infinite;"></div>
                    </div>
                </div>
            }

            if props.show_disconnected {
                <DisconnectedScreen
                    show={true}
                    disconnected={reducer.disconnected && !reducer.fatal_error && (is_joined || queue_status.is_some()) && !has_match_result}
                    title={reducer.disconnected_title.clone()}
                    msg={reducer.disconnected_msg.clone()}
                />
            }

            if has_match_result && !force_join_overlay {
                <EndScreen
                    kind={if is_victory { EndScreenKind::Victory } else { EndScreenKind::Defeat }}
                    title={victory.as_ref().map(|victory| victory.title.clone())}
                    message={victory.as_ref().map(|victory| victory.message.clone())}
                    score={reducer.stats.score}
                    kills={reducer.stats.kills}
                    captured={reducer.stats.captured}
                    survival_secs={reducer.stats.survival_secs}
                    on_rejoin={props.on_rejoin.clone()}
                    rejoin_cooldown={props.rejoin_cooldown}
                    on_spectate={props.on_spectate.reform(|_| SpectateTarget::Own)}
                />
            } else if is_spectating {
                <div data-testid="spectator-hud">
                    { leaderboard.clone() }
                    if reducer.spectate_ended() {
                        <SpectateEnded on_return={props.on_leave.clone()} />
                    }
                </div>
            } else if is_joined && !force_join_overlay {
                    <div data-testid="in-game-hud">
                        { leaderboard }
                        <div
                            data-testid="stats-overlay"
                            class="pointer-events-none"
                            style="
                                position: fixed;
                                right: 4px;
                                bottom: 4px;
                                padding: 0;
                                background: transparent;
                                color: #000;
                                font-family: monospace;
                                font-size: 11px;
                                line-height: 1.2;
                                text-align: right;
                                z-index: 50;
                            "
                        >
                            <div>{format!("FPS: {}", reducer.fps)}</div>
                            <div>{format!("Ping: {}ms", reducer.ping_ms)}</div>
                            <div>{format!("Board: {}x{}", reducer.state.board_size, reducer.state.board_size)}</div>
                        </div>
                    </div>
            } else if let (Some(sender), true) = (tx.clone(), props.join_step.is_select_match()) {
                <MatchBrowser
                    matches={reducer.match_list.clone()}
                    tx={sender}
                    now={reducer.server_now()}
                    on_select={props.on_spectate.reform(SpectateTarget::Match)}
                />
            } else if (tx.is_some() && !has_match_result) || force_join_overlay {
                <JoinScreen
                    player_name={props.player_name.clone()}
                    on_name_input={props.on_name_input.clone()}
                    on_name_submit={props.on_name_submit.clone()}
                    landing_cooldown={props.landing_cooldown}
                    join_step={props.join_step}
                    on_join={props.on_join.clone()}
                    on_spectate={props.on_menu_spectate.clone()}
                    error={reducer.error.as_ref().map(|notice| notice.error.clone())}
                    queue_status={queue_status.clone()}
                    is_loading={props.is_joining}
                    mode={reducer.mode.clone()}
                    mode_options={props.mode_options.clone()}
                    selected_mode_id={props.selected_mode_id.clone()}
                    on_select_mode={props.on_select_mode.clone()}
                    on_cycle_mode={props.on_cycle_mode.clone()}
                    quick_play_modes={props.quick_play_modes.clone()}
                    quick_play_reveal_ms={global_cfg.quick_play_reveal_ms}
                    title_pieces={global_cfg.title_pieces.clone()}
                    on_quick_play={props.on_quick_play.clone()}
                />
            }

            if tx.is_some() && !on_main_menu {
                <LeaveButton on_leave={props.on_leave.clone()} backdrop={Backdrop::of(&reducer)} />
            }

            // Stacks the top banners so errors sit below the queue banner. The column has no
            // z-index or transform, so each banner keeps its own place in the stacking order.
            <div style="position: absolute; top: 12px; left: 0; right: 0; display: flex; flex-direction: column; align-items: center; gap: 8px; pointer-events: none;">
                <QueueBanner queue={props.nested_queue} />

                <FatalNotification
                    show={reducer.fatal_error}
                    title={reducer.disconnected_title.clone()}
                    msg={reducer.disconnected_msg.clone()}
                />

                if let Some(notice) = &reducer.error {
                    if is_joined && !has_match_result {
                        <ErrorToast key={notice.serial} error={notice.error.clone()} />
                    }
                }
            </div>
        </div>
    }
}
