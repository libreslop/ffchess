//! Join flow UI for selecting mode, name, and kit.

use crate::ui_state::{CooldownSeconds, JoinStep};
use crate::utils::is_mobile;
use common::models::{GameModeClientConfig, ModeSummary};
use common::types::{KitId, ModeId};
use yew::prelude::*;

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
}

#[function_component(JoinScreen)]
/// Renders the join screen form and kit selection UI.
///
/// `props` supplies the join state and callbacks. Returns rendered HTML.
pub fn join_screen(props: &JoinScreenProps) -> Html {
    let mobile = is_mobile();
    let is_disabled = props.is_loading || props.error.is_some() || props.queue_status.is_some();

    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
            <div style={format!("position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 90%; max-width: 400px; padding: {};", if mobile { "20px 10px" } else { "30px" })}>
                if props.join_step.is_enter_name() {
                    <h1 style={format!("margin-top: 0; color: #fff; font-size: {}; letter-spacing: 4px; text-shadow: 3px 3px 0 rgba(0,0,0,0.6);", if mobile { "3em" } else { "4em" })}>{"FFCHESS"}</h1>
                    <div style="margin-bottom: 16px; max-height: 220px; overflow-y: auto; border: 2px solid #cbd5e1; background: transparent;">
                        { for props.mode_options.iter().map(|m| {
                            let on_select = props.on_select_mode.clone();
                            let id = m.id.clone();
                            let selected = m.id == props.selected_mode_id;
                            html!{
                                <button
                                    onclick={Callback::from(move |_| on_select.emit(id.clone()))}
                                    style={format!(
                                        "display: flex; width: 100%; justify-content: space-between; align-items: center; text-align: left; padding: 10px 12px; border: none; border-bottom: 1px solid rgba(255,255,255,0.08); background: {}; color: #fff; font-weight: {}; cursor: pointer;",
                                        if selected { "rgba(255,255,255,0.08)" } else { "transparent" },
                                        if selected { "700" } else { "500" },
                                    )}
                                >
                                    <span>{ m.display_name.clone() }</span>
                                    <span style="opacity: 0.8; font-variant-numeric: tabular-nums;">{ format!("{}/{}", m.players, m.max_players) }</span>
                                </button>
                            }
                        })}
                    </div>
                    <form onsubmit={props.on_name_submit.clone()}>
                        <div style="display: flex; flex-direction: column; gap: 15px; align-items: center;">
                            <input type="text" name="player_name" value={props.player_name.clone()} oninput={props.on_name_input.clone()} placeholder="This is a tale of..." autofocus=true
                                style="padding: 12px 20px; border-radius: 0; border: 2px solid #cbd5e1; width: 100%; box-sizing: border-box; font-size: 1.2em; outline: none; background: #fff; text-align: center;"/>
                            <button type="submit" disabled={props.landing_cooldown.is_active() || props.is_loading}
                                style={format!("padding: 10px 40px; background: {}; color: #fff; border: 3px solid {}; border-radius: 0; font-weight: 900; cursor: {}; font-size: 1.2em; width: auto; text-transform: uppercase; letter-spacing: 1px;",
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
                        <h3 style="color: #fff; margin-bottom: 25px; text-transform: uppercase; letter-spacing: 2px; text-shadow: 2px 2px 0 rgba(0,0,0,0.5);">{"CHOOSE YOUR ARMY"}</h3>

                        if let Some(error) = &props.error {
                            <div style="margin-bottom: 20px; color: #ef4444; background: rgba(255,255,255,0.9); padding: 10px 20px; border-radius: 4px; font-weight: bold;">
                                { error.to_string() }
                            </div>
                        }

                        if let Some(queue) = &props.queue_status {
                            <div style="width: 100%; max-width: 440px; background: rgba(0,0,0,0.45); border: 2px solid rgba(255,255,255,0.6); padding: 16px;">
                                <div style="color: #fff; font-size: 1.2em; font-weight: 900; letter-spacing: 1px; margin-bottom: 8px;">{"IN QUEUE"}</div>
                                <div style="color: #cbd5e1; margin-bottom: 6px;">
                                    { format!("Position: {}", queue.position_in_queue.as_u32()) }
                                </div>
                                <div style="color: #cbd5e1;">
                                    { format!(
                                        "Players queued: {}/{}",
                                        queue.queued_players.as_u32(),
                                        queue.required_players.as_u32()
                                    ) }
                                </div>
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
                                                    "width: 150px; height: 150px; position: relative; padding: 12px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; transition: all 0.2s; opacity: {}; display: flex; flex-direction: column; align-items: center; justify-content: center;",
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
        </>
    }
}
