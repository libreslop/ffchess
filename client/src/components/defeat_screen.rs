use crate::utils::is_mobile;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DefeatScreenProps {
    pub score: u64,
    pub kills: u32,
    pub captured: u32,
    pub survival_secs: u64,
    pub on_rejoin: Callback<MouseEvent>,
    pub rejoin_cooldown: i32,
}

#[function_component(DefeatScreen)]
pub fn defeat_screen(props: &DefeatScreenProps) -> Html {
    let mobile = is_mobile();
    let title_size = if mobile { "3em" } else { "4em" };
    let score_size = if mobile { "2.5em" } else { "3em" };

    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90; animation: simpleFadeIn 0.3s ease-out;"></div>
            <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 90%; max-width: 400px; color: #fff;">
                <div style="animation: fadeIn 0.3s ease-out;">
                    <h1 style={format!("color: #ef4444; margin-top: 0; font-size: {}; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);", title_size)}>{"DEFEAT"}</h1>

                    <div style="margin: 30px 0; display: flex; flex-direction: column; gap: 15px;">
                        <div style="padding: 15px;">
                            <span style="display: block; font-size: 0.9em; text-transform: uppercase; color: #cbd5e1; margin-bottom: 5px; letter-spacing: 1px;">{"Final Score"}</span>
                            <span style={format!("font-size: {}; font-weight: 900; color: #fff;", score_size)}>{props.score}</span>
                        </div>

                        <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px;">
                            <div style="padding: 10px;">
                                <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Kills"}</span>
                                <span style="font-size: 1.5em; font-weight: bold;">{props.kills}</span>
                            </div>
                            <div style="padding: 10px;">
                                <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Takes"}</span>
                                <span style="font-size: 1.5em; font-weight: bold;">{props.captured}</span>
                            </div>
                            <div style="padding: 10px;">
                                <span style="display: block; font-size: 0.7em; text-transform: uppercase; color: #cbd5e1; letter-spacing: 1px;">{"Survived"}</span>
                                <span style="font-size: 1.5em; font-weight: bold;">{format!("{}m {}s", props.survival_secs / 60, props.survival_secs % 60)}</span>
                            </div>
                        </div>
                    </div>

                    <button onclick={props.on_rejoin.clone()} disabled={props.rejoin_cooldown > 0}
                        style={format!("padding: 15px 40px; font-size: 1.2em; cursor: {}; background: {}; color: white; border: 3px solid {}; border-radius: 0; font-weight: 900; width: auto; transition: all 0.2s; text-transform: uppercase; letter-spacing: 2px;",
                            if props.rejoin_cooldown > 0 { "not-allowed" } else { "pointer" },
                            if props.rejoin_cooldown > 0 { "rgba(148, 163, 184, 0.2)" } else { "rgba(30, 41, 59, 0.4)" },
                            if props.rejoin_cooldown > 0 { "#94a3b8" } else { "#fff" })}>
                        if props.rejoin_cooldown > 0 {
                            {format!("Wait ({}s)", props.rejoin_cooldown)}
                        } else {
                            {"PLAY AGAIN"}
                        }
                    </button>
                    <p style="margin-top: 25px; font-size: 0.8em; color: #cbd5e1; letter-spacing: 1px;">{"Tip: Press ENTER to play again"}</p>
                </div>
            </div>
        </>
    }
}
