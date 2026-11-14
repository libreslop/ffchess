//! Button label that counts a cooldown down without changing the button's width.

use crate::ui_state::CooldownSeconds;
use yew::prelude::*;

/// Properties for a countdown label.
#[derive(Properties, PartialEq)]
pub struct CountdownLabelProps {
    /// Seconds left before the button is usable.
    pub cooldown: CooldownSeconds,
    /// Longest cooldown the label may show, used to reserve the widest countdown.
    pub max: CooldownSeconds,
    /// Text shown once the cooldown is over.
    pub ready: AttrValue,
}

/// Returns the widest countdown with as many digits as `max`.
///
/// Every digit is a `0`, the widest digit of the UI font, so no countdown of that many digits
/// is wider.
fn widest_wait_label(max: CooldownSeconds) -> String {
    format!("Wait ({}s)", "0".repeat(max.as_u32().max(1).to_string().len()))
}

#[function_component(CountdownLabel)]
/// Renders "Wait (Ns)" while `cooldown` is active and `ready` afterwards.
///
/// Every label the button can show shares one grid cell, the inactive ones hidden, so the
/// label keeps the width of the widest one while the countdown ticks down.
pub fn countdown_label(props: &CountdownLabelProps) -> Html {
    let current = if props.cooldown.is_active() {
        format!("Wait ({}s)", props.cooldown.as_u32())
    } else {
        props.ready.to_string()
    };
    html! {
        <span style="display: inline-grid;">
            <span style="grid-area: 1 / 1; visibility: hidden;">{props.ready.clone()}</span>
            <span style="grid-area: 1 / 1; visibility: hidden;">{widest_wait_label(props.max.max(props.cooldown))}</span>
            <span style="grid-area: 1 / 1;">{current}</span>
        </span>
    }
}
