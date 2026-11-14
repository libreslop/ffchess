//! Overlay shown once the spectated match is over.

use super::pixel_font::{NATIVE_PX, title_pixel_px};
use yew::prelude::*;

/// Properties for the spectate-ended overlay.
#[derive(Properties, PartialEq)]
pub struct SpectateEndedProps {
    pub on_return: Callback<MouseEvent>,
}

#[function_component(SpectateEnded)]
/// Renders "game ended" over the final board with a button back to the menu.
pub fn spectate_ended(props: &SpectateEndedProps) -> Html {
    // One step below the end screen titles keeps the longer title on one line on phones.
    let pixel_px = title_pixel_px().saturating_sub(1).max(1);
    html! {
        <>
            <div style="position: absolute; inset: 0; background: rgba(0,0,0,0.6); z-index: 90; animation: simpleFadeIn 0.3s ease-out;"></div>
            // The fade animates `transform`, so it runs on an inner box to keep the centering.
            <div data-testid="spectate-ended" style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 100; text-align: center; width: max-content; max-width: 95vw; color: #fff;">
                <div style="animation: fadeIn 0.3s ease-out;">
                    <h1 style={format!(
                        "color: #e2e8f0; margin: 0 0 24px; font-size: {}px; font-weight: normal; line-height: 1; white-space: nowrap; filter: drop-shadow(3px 3px 0 rgba(0,0,0,0.6)); text-transform: uppercase;",
                        pixel_px * NATIVE_PX,
                    )}>{"Game ended"}</h1>
                    <button onclick={props.on_return.clone()}
                        style="padding: 8px 40px; font-size: 1.2em; cursor: pointer; background: rgba(30, 41, 59, 0.4); color: white; border: 3px solid #fff; border-radius: 0; font-weight: 900; text-transform: uppercase; letter-spacing: 2px;">
                        {"Return to menu"}
                    </button>
                </div>
            </div>
        </>
    }
}
