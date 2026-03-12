use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DisconnectedScreenProps {
    pub show: bool,
    pub disconnected: bool,
}

#[function_component(DisconnectedScreen)]
pub fn disconnected_screen(props: &DisconnectedScreenProps) -> Html {
    if !props.show {
        return html! {};
    }

    html! {
        <div style={format!("position: absolute; inset: 0; background: #ef4444; z-index: 300; display: flex; align-items: center; justify-content: center; transition: opacity 0.3s ease-out; animation: simpleFadeIn 0.3s ease-out; opacity: {}; pointer-events: {};", 
            if props.disconnected { "1" } else { "0" },
            if props.disconnected { "all" } else { "none" }
        )}>
            <div style="text-align: center; color: #fff; padding: 20px;">
                <h1 style="color: #fff; margin: 0; font-size: 4em; letter-spacing: 4px; text-shadow: 0 4px 8px rgba(0,0,0,0.5);">{"DISCONNECTED"}</h1>
                <p style="margin: 20px 0 0; font-size: 1.2em; color: #fff; letter-spacing: 1px;">{"The connection to the server was lost."}</p>
            </div>
        </div>
    }
}
