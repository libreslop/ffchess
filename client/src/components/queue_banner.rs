//! Banner showing the queue a quick-play game is nested in.

use crate::reducer::QueueStatus;
use crate::utils::local_now;
use common::types::DurationMs;
use gloo_timers::callback::Interval;
use yew::prelude::*;

/// How often the waited time is redrawn, in milliseconds.
const CLOCK_REFRESH_MS: u32 = 1000;

/// Side length of the stop queuing icon, in pixels.
const STOP_ICON_PX: u32 = 14;

/// Color of the stop queuing icon.
const STOP_ICON_COLOR: &str = "#ef4444";

/// Trims a text box to the glyphs, which fill only the bottom of the font's line box, so
/// centering the box centers the visible text at any font size.
const TRIM_TO_GLYPHS: &str = "text-box: trim-both cap alphabetic;";

/// Every decimal digit, for clock positions that can show any of them.
const ANY_DIGIT: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// Digits the tens of seconds can show.
const TENS_OF_SECONDS: &[char] = &['0', '1', '2', '3', '4', '5'];

/// Properties for the queue banner.
#[derive(Properties, PartialEq)]
pub struct QueueBannerProps {
    /// Display name of the queued mode.
    pub mode_name: String,
    pub status: QueueStatus,
    pub on_stop: Callback<MouseEvent>,
}

#[function_component(QueueBanner)]
/// Renders the queue progress and waited time at the top of the screen, followed by an icon
/// that stops queuing.
pub fn queue_banner(props: &QueueBannerProps) -> Html {
    let now = use_state(local_now);
    {
        let now = now.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(CLOCK_REFRESH_MS, move || now.set(local_now()));
            move || drop(interval)
        });
    }
    let status = &props.status;

    html! {
        <div
            data-testid="queue-banner"
            data-ui-exempt="true"
            style="position: absolute; top: 12px; left: 50%; transform: translateX(-50%); z-index: 120; display: flex; gap: 10px; align-items: center; padding: 6px 12px; background: rgba(15,23,42,0.85); border: 2px solid #cbd5e1; color: #fff; font-size: 0.9em; white-space: nowrap; animation: simpleFadeIn 0.2s ease-out;"
        >
            <span style={TRIM_TO_GLYPHS}>{"Queuing in "}<strong>{ &props.mode_name }</strong></span>
            <span style={format!("{TRIM_TO_GLYPHS} font-variant-numeric: tabular-nums; color: #cbd5e1;")}>
                { queue_count(status) }
            </span>
            <span style={format!("{TRIM_TO_GLYPHS} font-variant-numeric: tabular-nums; color: #cbd5e1; font-size: 0.8em;")}>
                { clock(status.waited(*now)) }
            </span>
            <button
                data-testid="stop-queuing-button"
                title="Stop queuing"
                aria-label="Stop queuing"
                onclick={props.on_stop.clone()}
                style="padding: 0; background: transparent; border: none; cursor: pointer; display: block;"
            >
                // The icon file is used as a mask so it can be drawn in red.
                <span style={format!(
                    "display: block; width: {STOP_ICON_PX}px; height: {STOP_ICON_PX}px; background-color: {STOP_ICON_COLOR}; -webkit-mask: url(/assets/static/img/leave.svg) center / contain no-repeat; mask: url(/assets/static/img/leave.svg) center / contain no-repeat;"
                )}></span>
            </button>
        </div>
    }
}

/// Renders the queued player count, as wide as the widest count the queue can reach.
fn queue_count(status: &QueueStatus) -> Html {
    let required = status.required_players;
    let widest = (0..=required.as_u32()).map(|queued| format!("{queued}/{required}"));
    fixed_width(format!("{}/{}", status.queued_players, required), widest)
}

/// Renders a waited duration as minutes and seconds.
///
/// Each digit keeps the width of the widest digit its position can show, so the clock only
/// grows when the minutes gain a digit.
fn clock(waited: DurationMs) -> Html {
    let secs = waited.as_u64() / 1000;
    let minutes = (secs / 60).to_string();
    let seconds = format!("{:02}", secs % 60);
    let minute_digits = minutes.chars().map(|shown| (shown, ANY_DIGIT));
    let second_digits = seconds.chars().zip([TENS_OF_SECONDS, ANY_DIGIT]);
    let digit = |(shown, candidates): (char, &[char])| {
        fixed_width(shown.to_string(), candidates.iter().map(char::to_string))
    };
    html! {
        <>
            { for minute_digits.map(digit) }
            {":"}
            { for second_digits.map(digit) }
        </>
    }
}

/// Renders `shown` in a box as wide as the widest of `candidates`.
///
/// The candidates are laid invisibly into the same grid cell, so the box measures them in
/// the actual font instead of assuming equal digit widths.
fn fixed_width(shown: String, candidates: impl Iterator<Item = String>) -> Html {
    let hidden = "grid-area: 1 / 1; visibility: hidden;";
    html! {
        <span style="display: inline-grid; justify-items: center;">
            <span style="grid-area: 1 / 1;">{ shown }</span>
            { for candidates.map(|candidate| html! {
                <span style={hidden} aria-hidden="true">{ candidate }</span>
            })}
        </span>
    }
}
