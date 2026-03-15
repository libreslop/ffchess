//! Error toast component for transient gameplay errors.

use common::protocol::GameError;
use yew::prelude::*;

/// Properties for the transient error toast.
#[derive(Properties, PartialEq)]
pub struct ErrorToastProps {
    pub error: GameError,
}

#[function_component(ErrorToast)]
/// Renders a transient error toast message.
///
/// `props` provides the error to display. Returns rendered HTML.
pub fn error_toast(props: &ErrorToastProps) -> Html {
    html! {
        <div key={format!("{:?}", props.error)} style="position: absolute; top: 20px; left: 50%; transform: translateX(-50%); background: rgba(239, 68, 68, 0.9); color: white; padding: 10px 20px; border-radius: 8px; font-weight: bold; z-index: 1000; pointer-events: none; animation: fadeInOut 3s forwards;">
            { props.error.to_string() }
        </div>
    }
}
