//! List of running matches of a queue mode to pick one to spectate.

use crate::reducer::MsgSender;
use common::models::MatchSummary;
use common::protocol::ClientMessage;
use common::types::{ModeId, TimestampMs};
use gloo_timers::callback::Interval;
use yew::prelude::*;

/// How often the list asks the server for the running matches.
const REFRESH_MS: u32 = 2000;

/// Properties for the match browser.
#[derive(Properties, PartialEq)]
pub struct MatchBrowserProps {
    /// Running matches, or `None` until the server first answered.
    pub matches: Option<Vec<MatchSummary>>,
    pub tx: MsgSender,
    /// Current server time, for the time each match has been running.
    pub now: TimestampMs,
    pub on_select: Callback<ModeId>,
}

#[function_component(MatchBrowser)]
/// Renders the running matches over the dimmed board, refreshing them while shown.
pub fn match_browser(props: &MatchBrowserProps) -> Html {
    {
        let tx = props.tx.clone();
        use_effect_with((), move |_| {
            tx.send(ClientMessage::ListMatches);
            let interval = Interval::new(REFRESH_MS, move || {
                tx.send(ClientMessage::ListMatches);
            });
            move || drop(interval)
        });
    }

    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90;"></div>
            // The fade animates `transform`, so it runs on an inner box to keep the centering.
            <div data-testid="match-browser" style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; width: 90%; max-width: 400px; text-align: center;">
                <div style="animation: fadeIn 0.3s ease-out;">
                    <h3 style="color: #fff; margin: 0 0 16px; text-transform: uppercase; letter-spacing: 2px; text-shadow: 2px 2px 0 rgba(0,0,0,0.5);">{"Live matches"}</h3>
                    {
                        match &props.matches {
                            None => html! { <div style="color: #cbd5e1;">{"Loading matches..."}</div> },
                            Some(matches) if matches.is_empty() => html! {
                                <div style="color: #cbd5e1;">{"No matches are being played right now"}</div>
                            },
                            Some(matches) => html! {
                                <div style="max-height: 320px; overflow-y: auto; border: 2px solid #cbd5e1;">
                                    { for matches.iter().map(|summary| match_row(summary, props.now, &props.on_select)) }
                                </div>
                            },
                        }
                    }
                </div>
            </div>
        </>
    }
}

/// Renders one match, emitting its id to `on_select` when clicked.
fn match_row(summary: &MatchSummary, now: TimestampMs, on_select: &Callback<ModeId>) -> Html {
    let id = summary.id.clone();
    let on_click = on_select.reform(move |_: MouseEvent| id.clone());
    let running_secs = (now - summary.started_at).as_u64() / 1000;
    html! {
        <button
            onclick={on_click}
            style="display: flex; width: 100%; justify-content: space-between; align-items: center; gap: 12px; text-align: left; box-sizing: border-box; padding: 6px 12px; border: none; border-bottom: 1px solid rgba(255,255,255,0.08); background: transparent; color: #fff; font-weight: 500; cursor: pointer;"
        >
            <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                { for summary.players.iter().enumerate().map(|(index, player)| html! {
                    <>
                        if index > 0 {
                            <span style="color: rgba(255,255,255,0.6);">{" vs "}</span>
                        }
                        <span style={format!("color: {}; font-weight: bold;", player.color.as_ref())}>{&player.name}</span>
                    </>
                })}
            </span>
            <span style="flex-shrink: 0; font-variant-numeric: tabular-nums; color: rgba(255,255,255,0.8); font-size: 0.9em;">
                {format!("{}:{:02} · {} watching", running_secs / 60, running_secs % 60, summary.spectators)}
            </span>
        </button>
    }
}
