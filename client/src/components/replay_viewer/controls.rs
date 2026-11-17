//! Playback controls of the replay viewer: play and pause, the scrub bar, and the speed.

use common::types::DurationMs;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Properties for the playback controls.
#[derive(Properties, PartialEq)]
pub(super) struct ReplayControlsProps {
    /// Time into the replay shown now.
    pub(super) elapsed: DurationMs,
    pub(super) duration: DurationMs,
    /// Times into the replay a piece was taken.
    pub(super) captures: Vec<DurationMs>,
    pub(super) playing: bool,
    pub(super) speed_label: String,
    pub(super) on_toggle: Callback<()>,
    /// Jumps to a time into the replay.
    pub(super) on_seek: Callback<DurationMs>,
    pub(super) on_speed: Callback<()>,
}

/// Formats a time into the replay as `m:ss`.
fn clock_label(time: DurationMs) -> String {
    let secs = time.as_u64() / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Returns how far `time` is into `duration`, in percent.
fn percent(time: DurationMs, duration: DurationMs) -> f64 {
    if duration <= DurationMs::zero() {
        return 0.0;
    }
    (time.as_i64() as f64 / duration.as_i64() as f64 * 100.0).clamp(0.0, 100.0)
}

/// Layout shared by the control buttons, which take their colors from the `replay-button`
/// class.
const BUTTON_STYLE: &str = "flex: none; min-width: 44px; padding: 0 10px;";

#[function_component(ReplayControls)]
/// Renders the playback bar along the bottom of the replay panel: the scrub bar over a row
/// of play and pause, the time and the speed.
pub(super) fn replay_controls(props: &ReplayControlsProps) -> Html {
    let on_input = props.on_seek.reform(|event: InputEvent| {
        let value = event
            .target_dyn_into::<HtmlInputElement>()
            .and_then(|input| input.value().parse::<i64>().ok())
            .unwrap_or_default();
        DurationMs::from_millis(value)
    });
    let filled = percent(props.elapsed, props.duration);
    let markers = props.captures.iter().map(|capture| {
        html! {
            <span style={format!(
                "position: absolute; top: 0; bottom: 0; left: {:.3}%; width: 2px; margin-left: -1px; background: #ef4444;",
                percent(*capture, props.duration),
            )}></span>
        }
    });

    html! {
        <div
            data-testid="replay-controls"
            data-ui-exempt="true"
            style="flex: none; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; padding: 4px 8px 8px; border-top: 3px solid var(--wood-frame);"
        >
            <div style="position: relative; flex: 1 0 100%; height: 36px; display: flex; align-items: center;">
                // The track is drawn behind a transparent range input so capture marks show.
                <div style="position: absolute; left: 0; right: 0; height: 10px; background: rgba(0, 0, 0, 0.3); border: 2px solid var(--wood-frame); box-sizing: border-box; pointer-events: none;">
                    <div style={format!("position: absolute; left: 0; top: 0; bottom: 0; width: {filled:.3}%; background: var(--wood-plate);")}></div>
                    { for markers }
                </div>
                <input
                    data-testid="replay-scrubber"
                    type="range"
                    min="0"
                    max={props.duration.as_i64().to_string()}
                    step="1"
                    value={props.elapsed.as_i64().to_string()}
                    oninput={on_input}
                    aria-label="Replay position"
                    class="replay-scrubber"
                    style="position: relative; width: 100%; height: 36px; margin: 0; background: transparent; cursor: pointer;"
                />
            </div>
            <button
                data-testid="replay-toggle"
                title={if props.playing { "Pause (Space)" } else { "Play (Space)" }}
                onclick={props.on_toggle.reform(|_| ())}
                class="replay-button"
                style={BUTTON_STYLE}
            >
                {if props.playing { "❚❚" } else { "▶" }}
            </button>
            <span style="flex: 1; font-size: 20px; text-align: center; white-space: nowrap;">
                {format!("{} / {}", clock_label(props.elapsed), clock_label(props.duration))}
            </span>
            <button
                data-testid="replay-speed"
                title="Playback speed"
                onclick={props.on_speed.reform(|_| ())}
                class="replay-button"
                style={BUTTON_STYLE}
            >
                {props.speed_label.clone()}
            </button>
        </div>
    }
}
