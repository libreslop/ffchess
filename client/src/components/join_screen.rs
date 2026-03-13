use common::models::KitType;
use crate::utils::is_mobile;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct JoinScreenProps {
    pub player_name: String,
    pub on_name_input: Callback<InputEvent>,
    pub on_name_submit: Callback<SubmitEvent>,
    pub landing_cooldown: i32,
    pub join_step: i32,
    pub on_join: Callback<KitType>,
    pub error: Option<common::protocol::GameError>,
    pub is_loading: bool,
}

#[function_component(JoinScreen)]
pub fn join_screen(props: &JoinScreenProps) -> Html {
    let mobile = is_mobile();
    let is_disabled = props.is_loading || props.error.is_some();

    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
            <div style={format!("position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 90%; max-width: 400px; padding: {};", if mobile { "20px 10px" } else { "30px" })}>
                if props.join_step == 0 {
                    <h1 style={format!("margin-top: 0; color: #fff; font-size: {}; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);", if mobile { "3em" } else { "4em" })}>{"FFCHESS"}</h1>
                    <form onsubmit={props.on_name_submit.clone()}>
                        <div style="display: flex; flex-direction: column; gap: 15px; align-items: center;">
                            <input type="text" name="player_name" value={props.player_name.clone()} oninput={props.on_name_input.clone()} placeholder="This is a tale of..." autofocus=true
                                style="padding: 12px 20px; border-radius: 0; border: 2px solid #cbd5e1; width: 100%; box-sizing: border-box; font-size: 1.2em; outline: none; background: #fff; text-align: center;"/>
                            <button type="submit" disabled={props.landing_cooldown > 0 || props.is_loading}
                                style={format!("padding: 10px 40px; background: {}; color: #fff; border: 3px solid {}; border-radius: 0; font-weight: 900; cursor: {}; font-size: 1.2em; width: auto; text-transform: uppercase; letter-spacing: 1px;",
                                    if props.landing_cooldown > 0 || props.is_loading { "#94a3b8" } else { "#3b82f6" },
                                    if props.landing_cooldown > 0 || props.is_loading { "#64748b" } else { "#1e3a8a" },
                                    if props.landing_cooldown > 0 || props.is_loading { "not-allowed" } else { "pointer" })}>
                                if props.landing_cooldown > 0 {
                                    {format!("Wait ({}s)", props.landing_cooldown)}
                                } else {
                                    {"Play!"}
                                }
                            </button>
                        </div>
                    </form>
                } else {
                    <div style="animation: fadeIn 0.3s ease-out; display: flex; flex-direction: column; align-items: center;">
                        <h3 style="color: #fff; margin-bottom: 25px; text-transform: uppercase; letter-spacing: 2px; text-shadow: 0 2px 4px rgba(0,0,0,0.3);">{"CHOOSE YOUR ARMY"}</h3>

                        if let Some(error) = &props.error {
                            <div style="margin-bottom: 20px; color: #ef4444; background: rgba(255,255,255,0.9); padding: 10px 20px; border-radius: 4px; font-weight: bold;">
                                { error.to_string() }
                            </div>
                        }

                        <div style="display: grid; grid-template-columns: 1fr; gap: 12px; width: 100%;">
                            <button disabled={is_disabled} onclick={props.on_join.reform(|_| KitType::Standard)} style={format!("padding: 15px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s; opacity: {};", if is_disabled { "not-allowed" } else { "pointer" }, if is_disabled { "0.5" } else { "1.0" })}>
                                {"STANDARD"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"2 Pawns, 2 Knights"}</span>
                            </button>
                            <button disabled={is_disabled} onclick={props.on_join.reform(|_| KitType::Shield)} style={format!("padding: 15px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s; opacity: {};", if is_disabled { "not-allowed" } else { "pointer" }, if is_disabled { "0.5" } else { "1.0" })}>
                                {"SHIELD"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"6 Pawns"}</span>
                            </button>
                            <button disabled={is_disabled} onclick={props.on_join.reform(|_| KitType::Scout)} style={format!("padding: 15px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s; opacity: {};", if is_disabled { "not-allowed" } else { "pointer" }, if is_disabled { "0.5" } else { "1.0" })}>
                                {"SCOUT"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"1 Pawn, 2 Bishops"}</span>
                            </button>
                            <button disabled={is_disabled} onclick={props.on_join.reform(|_| KitType::Tank)} style={format!("padding: 15px; cursor: {}; border-radius: 0; border: 2px solid rgba(255,255,255,0.5); background: rgba(255,255,255,0.1); color: #fff; font-weight: bold; transition: all 0.2s; opacity: {};", if is_disabled { "not-allowed" } else { "pointer" }, if is_disabled { "0.5" } else { "1.0" })}>
                                {"TANK"}<br/><span style="font-weight: normal; font-size: 0.8em; color: #cbd5e1;">{"1 Rook"}</span>
                            </button>
                        </div>
                    </div>
                }
            </div>
        </>
    }
}
