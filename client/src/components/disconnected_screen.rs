//! Disconnected overlay component.

use yew::prelude::*;

/// Properties for the disconnected overlay.
#[derive(Properties, PartialEq)]
pub struct DisconnectedScreenProps {
    pub show: bool,
    pub disconnected: bool,
    pub title: Option<String>,
    pub msg: Option<String>,
}

#[function_component(DisconnectedScreen)]
/// Renders a full-screen disconnected overlay.
///
/// `props` controls visibility and message content. Returns rendered HTML.
pub fn disconnected_screen(props: &DisconnectedScreenProps) -> Html {
    if !props.show {
        return html! {};
    }

    let default_title = "DISCONNECTED";
    let default_msg = "The connection to the server was lost.";

    let title = props.title.as_deref().unwrap_or(default_title);
    let msg = props.msg.as_deref().unwrap_or(default_msg);

    html! {
        <div style={format!("position: absolute; inset: 0; background: #ef4444; z-index: 300; display: flex; align-items: center; justify-content: center; transition: opacity 0.3s ease-out; animation: simpleFadeIn 0.3s ease-out; opacity: {}; pointer-events: {};",
            if props.disconnected { "1" } else { "0" },
            if props.disconnected { "all" } else { "none" }
        )}>
            // Text scales with the viewport width so narrow screens never overflow, capped at
            // half the original 4em title.
            <div style="text-align: center; color: #fff; padding: 20px; box-sizing: border-box; max-width: 100%; overflow-wrap: anywhere;">
                <h1 style="color: #fff; margin: 0; font-size: min(2em, 7.5vw); letter-spacing: 2px; text-shadow: 2px 2px 0 rgba(0,0,0,0.6);">{title}</h1>
                <p style="margin: 10px 0 0; font-size: min(1em, 4vw); color: #fff; letter-spacing: 1px;">{msg}</p>
            </div>
        </div>
    }
}
