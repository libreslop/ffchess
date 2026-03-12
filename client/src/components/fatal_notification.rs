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

    {
        let is_visible = is_visible.clone();
        let is_animating_out = is_animating_out.clone();
        let show = props.show;
        use_effect_with(show, move |&show| {
            if show {
                is_visible.set(true);
                is_animating_out.set(false);
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

    let title = props.title.as_deref().unwrap_or("ALERT");
    let msg = props.msg.as_deref().unwrap_or("A critical error occurred.");
    let animation_name = if *is_animating_out {
        "fadeOutUp"
    } else {
        "fadeInDown"
    };

    html! {
        <div style={format!("position: absolute; top: 20px; left: 50%; transform: translateX(-50%); width: 90%; max-width: 500px; background: #ef4444; color: white; padding: 20px; border-radius: 12px; box-shadow: 0 10px 25px rgba(0,0,0,0.3); z-index: 1000; animation: {} 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;", animation_name)}>
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
                    <h3 style="margin: 0; font-size: 1.1em; letter-spacing: 1px; text-transform: uppercase;">{title}</h3>
                    <p style="margin: 5px 0 0; font-size: 0.95em; opacity: 0.9; line-height: 1.4;">{msg}</p>
                </div>
            </div>
            <div style="margin-top: 15px; padding-top: 15px; border-top: 1px solid rgba(255,255,255,0.2); font-size: 0.85em; opacity: 0.8; text-align: center;">
                {"Close other tabs to play here."}
            </div>
        </div>
    }
}
