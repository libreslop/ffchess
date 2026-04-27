//! Callback builders for the root application component.

use crate::reducer::{GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::{
    get_stored_id, get_stored_secret, is_mobile, request_fullscreen, set_stored_name,
};
use common::protocol::ClientMessage;
use common::types::{KitId, ModeId};
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

/// Builds the callback used to rejoin after a match ends.
pub fn build_on_rejoin(
    rc_ref: Rc<RefCell<CooldownSeconds>>,
    reducer: UseReducerHandle<GameStateReducer>,
    join_step: UseStateHandle<JoinStep>,
    has_interacted: UseStateHandle<bool>,
    rejoin_flow: UseStateHandle<RejoinFlow>,
    single_kit: Option<KitId>,
    on_join: Callback<KitId>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        if rc_ref.borrow().is_zero() {
            has_interacted.set(true);
            if reducer.disconnected {
                return;
            }
            let rejoin_single_kit = single_kit.clone().or_else(|| {
                reducer
                    .mode
                    .as_ref()
                    .and_then(|mode| (mode.kits.len() == 1).then(|| mode.kits[0].name.clone()))
            });
            rejoin_flow.set(RejoinFlow::Active);
            reducer.dispatch(GameAction::Reset);
            if let Some(kit_id) = rejoin_single_kit {
                join_step.set(JoinStep::SelectKit);
                on_join.emit(kit_id);
            } else {
                join_step.set(JoinStep::SelectKit);
            }
        }
    })
}

/// Builds the callback used to send join requests.
pub fn build_on_join(
    tx: UseStateHandle<Option<MsgSender>>,
    player_name: UseStateHandle<String>,
    reducer_ref: Rc<RefCell<UseReducerHandle<GameStateReducer>>>,
    is_joining: UseStateHandle<bool>,
    has_interacted: UseStateHandle<bool>,
    current_mode_id: UseStateHandle<ModeId>,
) -> Callback<KitId> {
    Callback::from(move |kit_name: KitId| {
        let current_reducer = reducer_ref.borrow().clone();
        if *is_joining || current_reducer.queue_status.is_some() {
            return;
        }
        if current_reducer.disconnected || current_reducer.fatal_error {
            current_reducer.dispatch(GameAction::SetDisconnected {
                disconnected: false,
                is_fatal: false,
                title: None,
                msg: None,
            });
        }
        has_interacted.set(true);
        if is_mobile() {
            request_fullscreen();
        }
        let trimmed_name = (*player_name).trim().to_string();
        if !trimmed_name.is_empty() {
            set_stored_name(&trimmed_name);
        }
        if let Some(sender) = (*tx).as_ref() {
            let mode_id = (*current_mode_id).clone();
            let stored_id = get_stored_id(&mode_id);
            let stored_secret = get_stored_secret(&mode_id);
            let join_result = sender.0.try_send(ClientMessage::Join {
                name: (*player_name).clone(),
                kit_name,
                player_id: stored_id,
                session_secret: stored_secret,
            });
            is_joining.set(join_result.is_ok());
        } else {
            is_joining.set(false);
        }
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

/// Builds the callback used when submitting the name entry form.
pub fn build_on_name_submit(
    join_step: UseStateHandle<JoinStep>,
    player_name: UseStateHandle<String>,
    landing_cooldown: UseStateHandle<CooldownSeconds>,
    reducer: UseReducerHandle<GameStateReducer>,
    has_interacted: UseStateHandle<bool>,
    single_kit: Option<KitId>,
    on_join: Callback<KitId>,
) -> Callback<SubmitEvent> {
    Callback::from(move |e: SubmitEvent| {
        e.prevent_default();
        has_interacted.set(true);
        if reducer.disconnected || landing_cooldown.is_active() {
            return;
        }
        let name = (*player_name).trim().to_string();
        set_stored_name(&name);
        if let Some(kit_id) = single_kit.clone() {
            join_step.set(JoinStep::SelectKit);
            on_join.emit(kit_id);
        } else {
            join_step.set(JoinStep::SelectKit);
        }
    })
}
