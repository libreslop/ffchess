//! A replay fetched from the server and prepared for playback.

use super::history::MoveHistory;
use super::move_list::SharedHistory;
use super::piece_icons::{PieceIcons, SharedIcons};
use super::timeline::ReplayTimeline;
use common::models::{Replay, ReplayHeader};
use common::types::ReplayId;
use gloo_net::http::{Request, Response};
use std::rc::Rc;

/// A replay ready to be played back.
pub(super) struct LoadedReplay {
    pub(super) id: ReplayId,
    pub(super) header: ReplayHeader,
    pub(super) timeline: Rc<ReplayTimeline>,
    pub(super) history: SharedHistory,
    pub(super) icons: SharedIcons,
    /// The replay as the server sent it, saved as is when downloaded.
    pub(super) source: Rc<str>,
}

impl LoadedReplay {
    /// Fetches the replay `id` from the server, or `None` when it expired, never existed, or
    /// could not be read.
    pub(super) async fn fetch(id: ReplayId) -> Option<Self> {
        let response = Request::get(&format!("/api/replays/{id}"))
            .send()
            .await
            .ok()
            .filter(Response::ok)?;
        let source = response.text().await.ok()?;
        let replay = serde_json::from_str::<Replay>(&source).ok()?;
        let timeline = ReplayTimeline::new(&replay)?;
        let icons = PieceIcons::fetch(&replay.header.catalog).await;
        Some(Self {
            id,
            history: SharedHistory(Rc::new(MoveHistory::new(&timeline, &replay.header.catalog))),
            icons: SharedIcons(Rc::new(icons)),
            header: replay.header,
            timeline: Rc::new(timeline),
            source: source.into(),
        })
    }

    /// Returns the name of the replay's files, before their extension.
    pub(super) fn file_stem(&self) -> String {
        format!("{}-{}", self.header.mode.id.as_ref(), self.id)
    }
}

/// Shared handle to a loaded replay, compared by identity so re-renders stay cheap.
#[derive(Clone)]
pub(super) struct SharedReplay(pub(super) Rc<LoadedReplay>);

impl PartialEq for SharedReplay {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
