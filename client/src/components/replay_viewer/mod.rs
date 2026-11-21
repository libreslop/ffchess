//! Replay viewer: plays a finished match back with a scrubbable timeline.

mod controls;
mod history;
mod hold;
mod loaded;
mod move_list;
mod piece_icons;
mod playback;
mod player;
mod share_menu;
mod style;
mod timeline;
mod title_bar;
mod video;

use crate::app::GlobalClientConfig;
use crate::components::pixel_font::NATIVE_PX;
use common::types::ReplayId;
use loaded::{LoadedReplay, SharedReplay};
use player::ReplayPlayer;
use std::rc::Rc;
use style::REPLAY_CSS;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

/// Properties for the replay viewer.
#[derive(Properties, PartialEq)]
pub struct ReplayViewerProps {
    pub replay_id: ReplayId,
    pub globals: GlobalClientConfig,
    pub on_close: Callback<()>,
}

/// Progress of fetching a replay.
#[derive(Clone, PartialEq)]
enum ReplayLoad {
    Loading,
    /// The replay expired, never existed, or could not be read.
    Unavailable,
    Ready(SharedReplay),
}

impl ReplayLoad {
    /// Fetches a replay from the server.
    async fn fetch(replay_id: ReplayId) -> Self {
        LoadedReplay::fetch(replay_id)
            .await
            .map_or(Self::Unavailable, |replay| {
                Self::Ready(SharedReplay(Rc::new(replay)))
            })
    }
}

#[function_component(ReplayViewer)]
/// Loads a replay and plays it back over everything else on screen.
pub fn replay_viewer(props: &ReplayViewerProps) -> Html {
    let load = use_state(|| ReplayLoad::Loading);
    {
        let load = load.clone();
        use_effect_with(props.replay_id, move |&replay_id| {
            load.set(ReplayLoad::Loading);
            spawn_local(async move { load.set(ReplayLoad::fetch(replay_id).await) });
        });
    }

    match &*load {
        ReplayLoad::Loading => message_screen("Loading replay...", None),
        ReplayLoad::Unavailable => {
            message_screen("This replay is no longer available", Some(&props.on_close))
        }
        ReplayLoad::Ready(replay) => html! {
            <ReplayPlayer
                replay={replay.clone()}
                globals={props.globals.clone()}
                on_close={props.on_close.clone()}
            />
        },
    }
}

/// Renders a screen showing only `text`, with a button going back through `on_back` when given.
fn message_screen(text: &'static str, on_back: Option<&Callback<()>>) -> Html {
    html! {
        <div data-testid="replay-viewer" class="replay-message" style={format!("font-size: {NATIVE_PX}px;")}>
            <style>{REPLAY_CSS}</style>
            <div>{text}</div>
            if let Some(on_back) = on_back {
                <button class="replay-message-back" onclick={on_back.reform(|_| ())}>
                    {"Back"}
                </button>
            }
        </div>
    }
}
