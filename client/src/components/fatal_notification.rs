use gloo_timers::callback::Timeout;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FatalNotificationProps {
    pub show: bool,
    pub title: Option<String>,
    pub msg: Option<String>,
}

#[function_component(FatalNotification)]
pub fn fatal_notification(props: &FatalNotificationProps) -> Html {
    let is_visible = use_state(|| props.show);
    let is_animating_out = use_state(|| false);
    let latched_title = use_state(|| props.title.clone());
    let latched_msg = use_state(|| props.msg.clone());

    {
        let is_visible = is_visible.clone();
        let is_animating_out = is_animating_out.clone();
        let latched_title = latched_title.clone();
        let latched_msg = latched_msg.clone();
        let show = props.show;
        let title = props.title.clone();
        let msg = props.msg.clone();

        use_effect_with((show, title, msg), move |(show, title, msg)| {
            if *show {
                is_visible.set(true);
                is_animating_out.set(false);
                if title.is_some() {
                    latched_title.set(title.clone());
                }
                if msg.is_some() {
                    latched_msg.set(msg.clone());
                }
            } else if *is_visible {
                is_animating_out.set(true);
                let is_visible = is_visible.clone();
                let is_animating_out = is_animating_out.clone();
                let handle = Timeout::new(400, move || {
                    is_visible.set(false);
                    is_animating_out.set(false);
                });
                return Box::new(move || drop(handle)) as Box<dyn FnOnce()>;
            }
            Box::new(|| ()) as Box<dyn FnOnce()>
        });
    }

    if !*is_visible {
        return html! {};
    }

    let title = latched_title.as_deref().unwrap_or("ALERT");
    let msg = latched_msg
        .as_deref()
        .unwrap_or("A critical error occurred.");
    let animation_name = if *is_animating_out {
        "fadeOutUp"
    } else {
        "fadeInDown"
    };

    // Pointer events are disabled so the banner never blocks UI interactions (e.g., kit selection buttons).
    html! {
        <div style={format!("position: absolute; top: 20px; left: 50%; transform: translateX(-50%); width: 90%; max-width: 500px; background: #ef4444; color: white; padding: 20px; border-radius: 0; border: 3px solid #7f1d1d; box-shadow: 0 10px 25px rgba(0,0,0,0.3); z-index: 1000; animation: {} 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards; pointer-events: none;", animation_name)}>
            <style>{"
                @keyframes fadeInDown {
                    from { opacity: 0; transform: translate(-50%, -20px); }
                    to { opacity: 1; transform: translate(-50%, 0); }
                }
                @keyframes fadeOutUp {
                    from { opacity: 1; transform: translate(-50%, 0); }
                    to { opacity: 0; transform: translate(-50%, -20px); }
                }
            "}</style>
            <div style="display: flex; align-items: center; gap: 15px;">
                <div style="font-size: 2em;">{"⚠️"}</div>
                <div>
                    <h3 style="margin: 0; font-size: 1.1em; letter-spacing: 1px;">{title}</h3>
                    <p style="margin: 5px 0 0; font-size: 0.95em; opacity: 0.9; line-height: 1.4;">{msg}</p>
                </div>
            </div>
        </div>
    }
}
