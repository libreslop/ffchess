//! UI rendering for the root application component.

use crate::app::GlobalClientConfig;
use crate::components::{
    DisconnectedScreen, EndScreen, EndScreenKind, ErrorToast, FatalNotification, GameView,
    JoinScreen, Leaderboard,
};
use crate::reducer::{GameStateReducer, MsgSender, QueueStatus};
use crate::ui_state::{CooldownSeconds, JoinStep};
use common::models::{GameModeClientConfig, ModeSummary};
use common::types::{KitId, ModeId, PlayerId};
use yew::prelude::*;

/// Render inputs for the root application view.
pub struct AppViewProps {
    pub global_cfg: GlobalClientConfig,
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: Option<MsgSender>,
    pub show_disconnected: bool,
    pub has_interacted: bool,
    pub has_match_result: bool,
    pub is_joined: bool,
    pub is_victory: bool,
    pub force_join_overlay: bool,
    pub player_id: PlayerId,
    pub player_name: String,
    pub join_step: JoinStep,
    pub on_name_input: Callback<InputEvent>,
    pub on_name_submit: Callback<SubmitEvent>,
    pub on_join: Callback<KitId>,
    pub landing_cooldown: CooldownSeconds,
    pub queue_status: Option<QueueStatus>,
    pub is_joining: bool,
    pub mode: Option<GameModeClientConfig>,
    pub mode_options: Vec<ModeSummary>,
    pub selected_mode_id: ModeId,
    pub on_select_mode: Callback<ModeId>,
    pub on_cycle_mode: Callback<i32>,
    pub on_rejoin: Callback<MouseEvent>,
    pub rejoin_cooldown: CooldownSeconds,
}

/// Renders the root application UI.
pub fn render_app(props: AppViewProps) -> Html {
    let global_cfg = props.global_cfg;
    let reducer = props.reducer;
    let tx = props.tx;
    let has_match_result = props.has_match_result;
    let is_joined = props.is_joined;
    let is_victory = props.is_victory;
    let force_join_overlay = props.force_join_overlay;
    let player_id = props.player_id;
    let show_scoreboard = props
        .mode
        .as_ref()
        .map(|mode| mode.show_scoreboard)
        .unwrap_or(true);

    html! {
        <div style="margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            <style>{"
                @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes simpleFadeIn { from { opacity: 0; } to { opacity: 1; } }
                @keyframes fadeInOut { 0% { opacity: 0; transform: translate(-50%, 20px); } 15% { opacity: 1; transform: translate(-50%, 0); } 85% { opacity: 1; transform: translate(-50%, 0); } 100% { opacity: 0; transform: translate(-50%, -20px); } }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
            "}</style>

            if let Some(sender) = tx.clone() {
                <GameView
                    key="stable-game-view"
                    reducer={reducer.clone()}
                    tx={sender}
                    render_interval_ms={global_cfg.render_interval_ms}
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
                    disconnected={reducer.disconnected && !reducer.fatal_error && (is_joined || props.queue_status.is_some()) && !has_match_result}
                    title={reducer.disconnected_title.clone()}
                    msg={reducer.disconnected_msg.clone()}
                />
            }

            <FatalNotification
                show={reducer.fatal_error}
                title={reducer.disconnected_title.clone()}
                msg={reducer.disconnected_msg.clone()}
            />

            if has_match_result && !force_join_overlay {
                <EndScreen
                    kind={if is_victory { EndScreenKind::Victory } else { EndScreenKind::Defeat }}
                    title={reducer.victory_title.clone()}
                    message={reducer.victory_msg.clone()}
                    score={reducer.last_score}
                    kills={reducer.last_kills}
                    captured={reducer.last_captured}
                    survival_secs={reducer.last_survival_secs}
                    on_rejoin={props.on_rejoin.clone()}
                    rejoin_cooldown={props.rejoin_cooldown}
                />
            } else if is_joined && !force_join_overlay {
                    <div data-testid="in-game-hud">
                        if show_scoreboard {
                            <Leaderboard players={reducer.state.players.values().cloned().collect::<Vec<_>>()} self_id={player_id} />
                        }
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
            } else if (tx.is_some() && !has_match_result) || force_join_overlay {
                <JoinScreen
                    player_name={props.player_name.clone()}
                    on_name_input={props.on_name_input.clone()}
                    on_name_submit={props.on_name_submit.clone()}
                    landing_cooldown={props.landing_cooldown}
                    join_step={props.join_step}
                    on_join={props.on_join.clone()}
                    error={reducer.error.clone()}
                    queue_status={props.queue_status.clone()}
                    is_loading={props.is_joining}
                    mode={props.mode.clone()}
                    mode_options={props.mode_options.clone()}
                    selected_mode_id={props.selected_mode_id.clone()}
                    on_select_mode={props.on_select_mode.clone()}
                    on_cycle_mode={props.on_cycle_mode.clone()}
                />
            }

            if let Some(error) = &reducer.error {
                if is_joined && !has_match_result {
                    <ErrorToast error={error.clone()} />
                }
            }
        </div>
    }
}
