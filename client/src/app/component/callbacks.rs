//! Callback builders for the root application component.

use crate::app::ws::ReducerRef;
use crate::reducer::{DisconnectNotice, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{
    DeathRecord, SessionStore, is_mobile, local_now, request_fullscreen, store_name,
};
use common::protocol::ClientMessage;
use common::types::{KitId, ModeId};
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

/// Moves the join flow to kit selection, joining right away when only one kit exists.
pub fn advance_to_kit_selection(
    join_step: &UseStateHandle<JoinStep>,
    single_kit: Option<KitId>,
    on_join: &Callback<KitId>,
) {
    join_step.set(JoinStep::SelectKit);
    if let Some(kit_id) = single_kit {
        on_join.emit(kit_id);
    }
}

/// Handles needed to leave the current session and return to the landing screen.
#[derive(Clone)]
pub struct LeaveInputs {
    pub join_step: UseStateHandle<JoinStep>,
    pub rejoin_flow: UseStateHandle<RejoinFlow>,
}

impl LeaveInputs {
    /// Leaves the current match or queue and shows the landing screen.
    ///
    /// The server makes players who leave the board wait out the respawn cooldown, so the
    /// leave is recorded as a death for the landing screen to count down.
    pub fn leave(&self, reducer: &UseReducerHandle<GameStateReducer>, tx: Option<&MsgSender>) {
        if reducer.has_session()
            && let Some(sender) = tx
        {
            sender.send(ClientMessage::Leave);
        }
        if reducer.is_on_board()
            && let Some(mode) = reducer.mode.as_ref()
        {
            SessionStore::new(&mode.id).record_death(DeathRecord {
                died_at: local_now(),
                cooldown: reducer.respawn_cooldown(),
            });
        }
        self.rejoin_flow.set(RejoinFlow::Inactive);
        reducer.dispatch(GameAction::Reset);
        self.join_step.set(JoinStep::EnterName);
    }
}

/// Builds the callback of the in-game leave button.
pub fn build_on_leave(
    inputs: LeaveInputs,
    reducer: UseReducerHandle<GameStateReducer>,
    tx: Option<MsgSender>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| inputs.leave(&reducer, tx.as_ref()))
}

/// Inputs of the rejoin callback.
pub struct RejoinInputs {
    pub rejoin_cooldown: Rc<RefCell<CooldownSeconds>>,
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub join_step: UseStateHandle<JoinStep>,
    pub has_interacted: UseStateHandle<bool>,
    pub rejoin_flow: UseStateHandle<RejoinFlow>,
    pub single_kit: Option<KitId>,
    pub on_join: Callback<KitId>,
}

/// Builds the callback used to rejoin after a match ends.
pub fn build_on_rejoin(inputs: RejoinInputs) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        if !inputs.rejoin_cooldown.borrow().is_zero() {
            return;
        }
        inputs.has_interacted.set(true);
        if inputs.reducer.disconnected {
            return;
        }
        let single_kit = inputs.single_kit.clone().or_else(|| {
            inputs
                .reducer
                .mode
                .as_ref()
                .and_then(|mode| (mode.kits.len() == 1).then(|| mode.kits[0].name.clone()))
        });
        inputs.rejoin_flow.set(RejoinFlow::Active);
        inputs.reducer.dispatch(GameAction::ResetForRejoin);
        advance_to_kit_selection(&inputs.join_step, single_kit, &inputs.on_join);
    })
}

/// Inputs of the join callback.
pub struct JoinInputs {
    pub tx: UseStateHandle<Option<MsgSender>>,
    pub player_name: UseStateHandle<String>,
    pub reducer: ReducerRef,
    pub is_joining: UseStateHandle<bool>,
    pub has_interacted: UseStateHandle<bool>,
    pub current_mode_id: UseStateHandle<ModeId>,
}

/// Builds the callback used to send join requests.
pub fn build_on_join(inputs: JoinInputs) -> Callback<KitId> {
    Callback::from(move |kit_name: KitId| {
        let reducer = inputs.reducer.borrow().clone();
        if *inputs.is_joining || reducer.queue_status.is_some() {
            return;
        }
        if reducer.disconnected || reducer.fatal_error {
            reducer.dispatch(GameAction::SetDisconnected(DisconnectNotice::none()));
        }
        inputs.has_interacted.set(true);
        if is_mobile() {
            request_fullscreen();
        }
        let trimmed_name = inputs.player_name.trim();
        if !trimmed_name.is_empty() {
            store_name(trimmed_name);
        }
        let Some(sender) = (*inputs.tx).as_ref() else {
            inputs.is_joining.set(false);
            return;
        };
        let (player_id, session_secret) = SessionStore::new(&inputs.current_mode_id).load();
        let sent = sender.send(ClientMessage::Join {
            name: (*inputs.player_name).clone(),
            kit_name,
            player_id,
            session_secret,
        });
        inputs.is_joining.set(sent);
    })
}

/// Builds the callback used when typing a player name.
pub fn build_on_name_input(player_name: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |e: InputEvent| {
        player_name.set(
            e.target_unchecked_into::<web_sys::HtmlInputElement>()
                .value(),
        );
    })
}

/// Inputs of the name submission callback.
pub struct NameSubmitInputs {
    pub join_step: UseStateHandle<JoinStep>,
    pub player_name: UseStateHandle<String>,
    pub landing_cooldown: UseStateHandle<CooldownSeconds>,
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub has_interacted: UseStateHandle<bool>,
    pub single_kit: Option<KitId>,
    pub on_join: Callback<KitId>,
}

/// Builds the callback used when submitting the name entry form.
pub fn build_on_name_submit(inputs: NameSubmitInputs) -> Callback<SubmitEvent> {
    Callback::from(move |e: SubmitEvent| {
        e.prevent_default();
        inputs.has_interacted.set(true);
        if inputs.reducer.disconnected || inputs.landing_cooldown.is_active() {
            return;
        }
        store_name(inputs.player_name.trim());
        advance_to_kit_selection(
            &inputs.join_step,
            inputs.single_kit.clone(),
            &inputs.on_join,
        );
    })
}
