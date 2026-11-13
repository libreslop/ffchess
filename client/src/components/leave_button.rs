//! Button that leaves the current match, queue or end screen.

use crate::ui_state::Backdrop;
use yew::prelude::*;

/// Properties for the leave button.
#[derive(Properties, PartialEq)]
pub struct LeaveButtonProps {
    pub on_leave: Callback<MouseEvent>,
    /// What sits behind the button, which picks the icon color like it does for chat text.
    pub backdrop: Backdrop,
}

#[function_component(LeaveButton)]
/// Renders the leave icon in the top left corner, where the main menu shows the GitHub link.
pub fn leave_button(props: &LeaveButtonProps) -> Html {
    // The icon file is used as a mask so it can take the backdrop's foreground color.
    html! {
        <button
            data-testid="leave-button"
            data-ui-exempt="true"
            title="Leave game"
            aria-label="Leave game"
            onclick={props.on_leave.clone()}
            style={format!(
                "position: absolute; top: 12px; left: 12px; z-index: 110; padding: 0; background: transparent; border: none; cursor: pointer; display: block; filter: drop-shadow(1px 1px 0 {});",
                props.backdrop.foreground_shadow(),
            )}
        >
            <span style={format!(
                "display: block; width: 24px; height: 24px; background-color: {}; -webkit-mask: url(/assets/static/img/leave.svg) center / contain no-repeat; mask: url(/assets/static/img/leave.svg) center / contain no-repeat;",
                props.backdrop.foreground(),
            )}></span>
        </button>
    }
}
