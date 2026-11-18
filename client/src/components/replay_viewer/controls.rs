//! Playback controls of the replay viewer: play and pause, the scrub bar, and the speed.

use super::playback::PlaybackButton;
use common::types::DurationMs;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Properties for the playback controls.
#[derive(Properties, PartialEq)]
pub(super) struct ReplayControlsProps {
    /// Time into the replay shown now.
    pub(super) elapsed: DurationMs,
    pub(super) duration: DurationMs,
    pub(super) playing: bool,
    pub(super) speed_label: String,
    /// Button lit as pressed by a key.
    pub(super) pressed: Option<PlaybackButton>,
    pub(super) on_toggle: Callback<()>,
    /// Jumps to a time into the replay.
    pub(super) on_seek: Callback<DurationMs>,
    pub(super) on_speed: Callback<()>,
}

/// Returns how far `time` is into `duration`, in percent.
fn percent(time: DurationMs, duration: DurationMs) -> f64 {
    if duration <= DurationMs::zero() {
        return 0.0;
    }
    (time.as_i64() as f64 / duration.as_i64() as f64 * 100.0).clamp(0.0, 100.0)
}

/// Layout of the square play and pause button, which takes its colors from the
/// `replay-button` class.
const TOGGLE_STYLE: &str = "flex: none; width: 36px; padding: 0;";
/// Layout of the speed button, whose fixed width keeps it from resizing as its label changes.
const SPEED_STYLE: &str = "flex: none; width: 56px; padding: 0;";

#[function_component(ReplayControls)]
/// Renders the playback bar below the move list: play and pause, the speed,
/// and the scrub bar filling the rest of the row.
pub(super) fn replay_controls(props: &ReplayControlsProps) -> Html {
    let on_input = props.on_seek.reform(|event: InputEvent| {
        let value = event
            .target_dyn_into::<HtmlInputElement>()
            .and_then(|input| input.value().parse::<i64>().ok())
            .unwrap_or_default();
        DurationMs::from_millis(value)
    });
    let filled = percent(props.elapsed, props.duration);
    let button_class = |button| {
        classes!(
            "replay-button",
            (props.pressed == Some(button)).then_some("pressed")
        )
    };

    html! {
        <div
            data-testid="replay-controls"
            data-ui-exempt="true"
            style="flex: none; display: flex; align-items: center; gap: 4px; padding: 4px; border-top: 3px solid var(--wood-frame);"
        >
            <button
                data-testid="replay-toggle"
                title={if props.playing { "Pause (Space)" } else { "Play (Space)" }}
                onclick={props.on_toggle.reform(|_| ())}
                class={button_class(PlaybackButton::Toggle)}
                style={TOGGLE_STYLE}
            >
                <span class={classes!(
                    "replay-icon",
                    if props.playing { "replay-icon-pause" } else { "replay-icon-play" },
                )}></span>
            </button>
            <button
                data-testid="replay-speed"
                title="Playback speed"
                onclick={props.on_speed.reform(|_| ())}
                class={button_class(PlaybackButton::Speed)}
                style={SPEED_STYLE}
            >
                {props.speed_label.clone()}
            </button>
            <div style="position: relative; flex: 1; min-width: 0; height: 36px; display: flex; align-items: center;">
                // The track is drawn behind a transparent range input, which only draws its thumb.
                <div style="position: absolute; left: 0; right: 0; height: 10px; background: rgba(0, 0, 0, 0.3); border: 2px solid var(--wood-frame); box-sizing: border-box; pointer-events: none;">
                    <div style={format!("position: absolute; left: 0; top: 0; bottom: 0; width: {filled:.3}%; background: var(--wood-plate);")}></div>
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
        </div>
    }
}
