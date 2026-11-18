//! Replay viewer: plays a finished match back with a scrubbable timeline.

mod controls;
mod history;
mod move_list;
mod piece_icons;
mod playback;
mod player;
mod style;
mod timeline;

use crate::app::GlobalClientConfig;
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
        let Ok(replay) = response.json::<Replay>().await else {
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

    let message = |text: &'static str| {
        html! {
            <div data-testid="replay-viewer" style="position: fixed; inset: 0; z-index: 300; background: #f0f2f5; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px;">
                <h2 style="margin: 0; color: #64748b;">{text}</h2>
                <button onclick={props.on_close.reform(|_| ())}
                    style="padding: 8px 32px; font-size: 1.1em; cursor: pointer; background: #1e293b; color: #fff; border: 3px solid #0f172a; border-radius: 0; font-weight: 900; text-transform: uppercase; letter-spacing: 2px;">
                    {"Back"}
                </button>
            </div>
        }
    };
    match &*load {
        ReplayLoad::Loading => message("Loading replay..."),
        ReplayLoad::Unavailable => message("This replay is no longer available"),
        ReplayLoad::Ready(replay) => html! {
            <ReplayPlayer
                replay={replay.clone()}
                globals={props.globals.clone()}
                on_close={props.on_close.clone()}
            />
        },
    }
}
