//! Replay viewer: plays a finished match back with a scrubbable timeline.

mod controls;
mod history;
mod hold;
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
use common::models::Replay;
use common::types::ReplayId;
use gloo_net::http::Request;
use history::MoveHistory;
use move_list::SharedHistory;
use piece_icons::{PieceIcons, SharedIcons};
use player::{LoadedReplay, ReplayPlayer, SharedReplay};
use std::rc::Rc;
use timeline::ReplayTimeline;
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
        let Ok(response) = Request::get(&format!("/api/replays/{replay_id}"))
            .send()
            .await
        else {
            return Self::Unavailable;
        };
        if !response.ok() {
            return Self::Unavailable;
        }
        let Ok(source) = response.text().await else {
            return Self::Unavailable;
        };
        let Ok(replay) = serde_json::from_str::<Replay>(&source) else {
            return Self::Unavailable;
        };
        let Some(timeline) = ReplayTimeline::new(&replay) else {
            return Self::Unavailable;
        };
        let icons = PieceIcons::fetch(&replay.header.catalog).await;
        Self::Ready(SharedReplay(Rc::new(LoadedReplay {
            history: SharedHistory(Rc::new(MoveHistory::new(&timeline, &replay.header.catalog))),
            icons: SharedIcons(Rc::new(icons)),
            header: replay.header,
            timeline: Rc::new(timeline),
            source: source.into(),
        })))
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

    // Shown in the colors of the board's surroundings, darkened like the menu darkens the
    // board, with the text of the queue waiting screen.
    let message = |text: &'static str, can_close: bool| {
        html! {
            <div data-testid="replay-viewer" style={format!("position: fixed; inset: 0; z-index: 300; background: linear-gradient(rgba(0,0,0,0.6), rgba(0,0,0,0.6)), #413931; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px; color: #e2e8f0; font-size: {NATIVE_PX}px; text-shadow: 2px 2px 0 rgba(0,0,0,0.5);")}>
                <div>{text}</div>
                if can_close {
                    <button onclick={props.on_close.reform(|_| ())}
                        style="padding: 6px 24px; background: transparent; color: #e2e8f0; border: 3px solid #cbd5e1; border-radius: 0; font-weight: 900; font-size: 1em; cursor: pointer; text-transform: uppercase; letter-spacing: 1px;">
                        {"Back"}
                    </button>
                }
            </div>
        }
    };
    match &*load {
        ReplayLoad::Loading => message("Loading replay...", false),
        ReplayLoad::Unavailable => message("This replay is no longer available", true),
        ReplayLoad::Ready(replay) => html! {
            <ReplayPlayer
                replay_id={props.replay_id}
                replay={replay.clone()}
                globals={props.globals.clone()}
                on_close={props.on_close.clone()}
            />
        },
    }
}
