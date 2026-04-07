//! Shared end-of-match overlay for victory and defeat outcomes.

use crate::utils::is_mobile;
use common::types::Score;
use yew::prelude::*;

/// End result variant shown in the shared end screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EndScreenKind {
    Victory,
    Defeat,
}

/// Properties for the end-of-match overlay.
#[derive(Properties, PartialEq)]
pub struct EndScreenProps {
    pub kind: EndScreenKind,
    pub title: Option<String>,
    pub message: Option<String>,
    pub score: Score,
    pub kills: u32,
    pub captured: u32,
    pub survival_secs: u64,
    pub on_rejoin: Callback<MouseEvent>,
    pub rejoin_cooldown: i32,
}

#[function_component(EndScreen)]
/// Renders the end-of-match summary and rejoin button.
pub fn end_screen(props: &EndScreenProps) -> Html {
    let mobile = is_mobile();
    let title_size = if mobile { "3em" } else { "4em" };
    let score_size = if mobile { "2.5em" } else { "3em" };
    let is_victory = props.kind == EndScreenKind::Victory;
    let title = match (props.kind, props.title.as_deref()) {
        (EndScreenKind::Victory, Some(t)) if !t.trim().is_empty() => t.to_uppercase(),
        (EndScreenKind::Victory, _) => "VICTORY".to_string(),
        (EndScreenKind::Defeat, _) => "DEFEAT".to_string(),
    };
    let title_color = if is_victory { "#22c55e" } else { "#ef4444" };
    let message_color = if is_victory { "#bbf7d0" } else { "#fecaca" };

    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90; animation: simpleFadeIn 0.3s ease-out;"></div>
            <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: 90%; max-width: 400px; color: #fff;">
                <div style="animation: fadeIn 0.3s ease-out;">
                    <h1 style={format!("color: {}; margin-top: 0; font-size: {}; letter-spacing: 4px; text-shadow: 3px 3px 0 rgba(0,0,0,0.6); text-transform: uppercase;", title_color, title_size)}>{title}</h1>

                    if let Some(message) = props.message.clone() {
                        if !message.trim().is_empty() {
                            <p style={format!("margin: 0 0 20px 0; font-size: 0.95em; color: {}; letter-spacing: 0.5px;", message_color)}>{message}</p>
                        }
                    }

                    <div style="margin: 30px 0; display: flex; flex-direction: column; gap: 15px;">
                        <div style="padding: 15px;">
                            <span style="display: block; font-size: 0.9em; text-transform: uppercase; color: #cbd5e1; margin-bottom: 5px; letter-spacing: 1px;">{"Final Score"}</span>
                            <span style={format!("font-size: {}; font-weight: 900; color: #fff;", score_size)}>{props.score.to_string()}</span>
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
