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

/// Style shared by the square control buttons.
const BUTTON_STYLE: &str = "flex: none; min-width: 44px; height: 36px; padding: 0 10px; background: #1e293b; color: #fff; border: 3px solid #0f172a; border-radius: 0; font-weight: 900; font-size: 0.95em; cursor: pointer; text-transform: uppercase; letter-spacing: 1px;";

#[function_component(ReplayControls)]
/// Renders the playback bar along the bottom of the replay viewer.
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
            style="position: absolute; left: 50%; bottom: 16px; transform: translateX(-50%); width: min(720px, calc(100% - 32px)); box-sizing: border-box; display: flex; align-items: center; gap: 10px; padding: 8px; background: rgba(255, 255, 255, 0.92); border: 3px solid #0f172a; box-shadow: 4px 4px 0 rgba(15, 23, 42, 0.25); z-index: 120; font-family: monospace;"
        >
            <button
                data-testid="replay-toggle"
                title={if props.playing { "Pause (Space)" } else { "Play (Space)" }}
                onclick={props.on_toggle.reform(|_| ())}
                style={BUTTON_STYLE}
            >
                {if props.playing { "❚❚" } else { "▶" }}
            </button>
            <div style="position: relative; flex: 1; min-width: 0; height: 36px; display: flex; align-items: center;">
                // The track is drawn behind a transparent range input so capture marks show.
                <div style="position: absolute; left: 0; right: 0; height: 10px; background: #cbd5e1; border: 2px solid #0f172a; box-sizing: border-box; pointer-events: none;">
                    <div style={format!("position: absolute; left: 0; top: 0; bottom: 0; width: {filled:.3}%; background: #334155;")}></div>
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
            <span style="flex: none; font-size: 0.85em; font-weight: bold; color: #0f172a; white-space: nowrap; font-variant-numeric: tabular-nums;">
                {format!("{} / {}", clock_label(props.elapsed), clock_label(props.duration))}
            </span>
            <button
                data-testid="replay-speed"
                title="Playback speed"
                onclick={props.on_speed.reform(|_| ())}
                style={BUTTON_STYLE}
            >
                {props.speed_label.clone()}
            </button>
        </div>
    }
}

/// Styles the scrub bar thumb, which inline styles cannot reach.
pub(super) const SCRUBBER_CSS: &str = "
    .replay-scrubber { -webkit-appearance: none; appearance: none; }
    .replay-scrubber::-webkit-slider-runnable-track { background: transparent; height: 36px; }
    .replay-scrubber::-moz-range-track { background: transparent; height: 36px; }
    .replay-scrubber::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 24px; margin-top: 6px; background: #fff; border: 3px solid #0f172a; border-radius: 0; }
    .replay-scrubber::-moz-range-thumb { width: 8px; height: 18px; background: #fff; border: 3px solid #0f172a; border-radius: 0; }
";

