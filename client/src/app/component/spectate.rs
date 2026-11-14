//! Spectating a board instead of playing on it.

use super::callbacks::SlotSession;
use crate::reducer::GameAction;
use crate::ui_state::{JoinStep, RejoinFlow};
use crate::utils::SessionStore;
use common::protocol::{ClientMessage, SpectateTarget};
use common::types::ModeId;
use yew::prelude::*;

/// Inputs of the spectate callbacks.
pub struct SpectateInputs {
    pub session: SlotSession,
    /// Mode of the focused connection, whose stored player id identifies this client.
    pub mode_id: ModeId,
    pub player_name: UseStateHandle<String>,
    pub join_step: UseStateHandle<JoinStep>,
    pub rejoin_flow: UseStateHandle<RejoinFlow>,
    pub has_interacted: UseStateHandle<bool>,
}

/// Callbacks starting to spectate from the different screens.
pub struct SpectateCallbacks {
    /// Spectates a given board.
    pub on_spectate: Callback<SpectateTarget>,
    /// The menu's spectate button: watches a shared world right away, or lists the matches of
    /// a queue mode to pick one.
    pub on_menu_spectate: Callback<MouseEvent>,
}

/// Builds the spectate callbacks.
pub fn build_spectate_callbacks(inputs: SpectateInputs) -> SpectateCallbacks {
    let on_spectate = {
        let session = inputs.session.clone();
        let player_name = inputs.player_name.clone();
        let rejoin_flow = inputs.rejoin_flow.clone();
        let has_interacted = inputs.has_interacted.clone();
        let mode_id = inputs.mode_id.clone();
        Callback::from(move |target: SpectateTarget| {
            let Some(sender) = &session.tx else {
                return;
            };
            has_interacted.set(true);
            rejoin_flow.set(RejoinFlow::Inactive);
            session.reducer.dispatch(GameAction::BeginSpectate);
            // The server refuses to show a game this client plays in from another tab.
            let (player_id, _) = SessionStore::new(&mode_id).load();
            sender.send(ClientMessage::Spectate {
                name: (*player_name).clone(),
                target,
                player_id,
            });
        })
    };
    let on_menu_spectate = {
        let on_spectate = on_spectate.clone();
        let reducer = inputs.session.reducer.clone();
        let join_step = inputs.join_step.clone();
        Callback::from(move |_: MouseEvent| {
            if reducer.is_queue_mode() {
                join_step.set(JoinStep::SelectMatch);
            } else {
                on_spectate.emit(SpectateTarget::Own);
            }
        })
    };
    SpectateCallbacks {
        on_spectate,
        on_menu_spectate,
    }
}
