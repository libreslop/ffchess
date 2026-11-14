//! Banner showing the queue a quick-play game is nested in.

use super::pixel_font::TRIM_TO_GLYPHS;
use crate::reducer::QueueStatus;
use crate::utils::local_now;
use common::types::DurationMs;
use gloo_timers::callback::{Interval, Timeout};
use yew::prelude::*;

/// How often the waited time is redrawn, in milliseconds.
const CLOCK_REFRESH_MS: u32 = 1000;

/// How long the banner takes to float in or out, in milliseconds.
const FLOAT_MS: u32 = 400;

/// Side length of the stop queuing icon, in pixels.
const STOP_ICON_PX: u32 = 14;

/// Color of the stop queuing icon.
const STOP_ICON_COLOR: &str = "#ef4444";

/// Every decimal digit, for clock positions that can show any of them.
const ANY_DIGIT: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// Digits the tens of seconds can show.
const TENS_OF_SECONDS: &[char] = &['0', '1', '2', '3', '4', '5'];

/// The queue a quick-play game is nested in, shown in a banner over the game.
#[derive(Clone, PartialEq)]
pub struct NestedQueue {
    /// Display name of the queued mode.
    pub mode_name: String,
    pub status: QueueStatus,
    pub on_stop: Callback<MouseEvent>,
}

/// Properties for the queue banner.
#[derive(Properties, PartialEq)]
pub struct QueueBannerProps {
    /// The queue to show, or `None` to float the banner away.
    pub queue: Option<NestedQueue>,
}

#[function_component(QueueBanner)]
/// Renders the queue progress and waited time at the top of the screen, followed by an icon
/// that stops queuing.
///
/// The banner floats in from the top when a queue appears. When it goes away the last queue
/// shown stays up while the banner floats back out.
pub fn queue_banner(props: &QueueBannerProps) -> Html {
    let now = use_state(local_now);
    {
        let now = now.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(CLOCK_REFRESH_MS, move || now.set(local_now()));
            move || drop(interval)
        });
    }
    let last_shown = use_mut_ref(|| None::<NestedQueue>);
    if props.queue.is_some() {
        *last_shown.borrow_mut() = props.queue.clone();
    }
    let force_update = use_force_update();
    {
        let last_shown = last_shown.clone();
        use_effect_with(props.queue.is_some(), move |shown| {
            let hide = (!shown).then(|| {
                Timeout::new(FLOAT_MS, move || {
                    last_shown.borrow_mut().take();
                    force_update.force_update();
                })
            });
            move || drop(hide)
        });
    }
    let leaving = props.queue.is_none();
    let Some(queue) = last_shown.borrow().clone() else {
        return html! {};
    };
    let status = &queue.status;
    let (animation, pointer_events) = if leaving {
        ("floatOutToTop", "none")
    } else {
        ("floatInFromTop", "auto")
    };

    html! {
        <div
            data-testid="queue-banner"
            data-ui-exempt="true"
            style={format!("position: relative; z-index: 120; pointer-events: {pointer_events}; display: flex; gap: 10px; align-items: center; padding: 6px 12px; background: rgba(15,23,42,0.85); border: 2px solid #cbd5e1; color: #fff; font-size: 0.9em; white-space: nowrap; animation: {animation} {FLOAT_MS}ms cubic-bezier(0.16, 1, 0.3, 1) forwards;")}
        >
            <span style={TRIM_TO_GLYPHS}>{"Queuing in "}<strong>{ &queue.mode_name }</strong></span>
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
                onclick={queue.on_stop.clone()}
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
