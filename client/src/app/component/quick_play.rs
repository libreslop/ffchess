//! Quick-play games nested inside a matchmaking queue.
//!
//! While the base mode queues, the player can play a quick-play mode on a second connection.
//! The queued match takes the focus back once it starts, and stopping the queue leaves the
//! quick-play game as the only one.

use super::callbacks::SlotSession;
use crate::app::focus::{FocusAction, ModeFocus};
use crate::ui_state::{JoinStep, RejoinFlow};
use common::types::ModeId;
use wasm_bindgen::JsValue;
use yew::hook;
use yew::prelude::*;

/// Inputs of the quick-play mode picker.
pub struct QuickPlayInputs {
    pub focus: UseReducerHandle<ModeFocus>,
    pub join_step: UseStateHandle<JoinStep>,
    pub rejoin_flow: UseStateHandle<RejoinFlow>,
    pub has_interacted: UseStateHandle<bool>,
    /// Whether the base mode currently waits in a queue.
    pub base_queued: bool,
}

/// Builds the callback that starts a quick-play game while the base mode queues.
///
/// The player already entered a name for the queue, so the game opens on kit selection.
pub fn build_on_quick_play(inputs: QuickPlayInputs) -> Callback<ModeId> {
    Callback::from(move |mode_id: ModeId| {
        if !inputs.base_queued || inputs.focus.is_nested() {
            return;
        }
        inputs.has_interacted.set(true);
        inputs.rejoin_flow.set(RejoinFlow::Inactive);
        inputs.join_step.set(JoinStep::SelectKit);
        inputs.focus.dispatch(FocusAction::Nest(mode_id));
    })
}

/// Builds the callback of the banner's stop button, which leaves the queue and keeps the
/// quick-play game.
pub fn build_on_stop_queuing(
    base: SlotSession,
    focus: UseReducerHandle<ModeFocus>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        base.abandon();
        promote_nested(&focus);
    })
}

/// Moves the focus away from a nested quick-play game once its queue resolves.
///
/// When the queued match starts the quick-play game is left for it. When the queue is gone
/// without a match (its connection dropped), a quick-play game still in play becomes the base
/// mode; one that lost its connection too goes back to the base mode's menu.
#[hook]
pub fn use_nested_queue_effect(
    focus: UseReducerHandle<ModeFocus>,
    base: SlotSession,
    nested: SlotSession,
) {
    let base_matched = base.reducer.active_player_id().is_some();
    let base_queued = base.reducer.queue_status.is_some();
    let nested_playing =
        nested.reducer.active_player_id().is_some() && !nested.reducer.disconnected;
    use_effect_with(
        (focus.is_nested(), base_matched, base_queued, nested_playing),
        move |&(is_nested, base_matched, base_queued, nested_playing)| {
            // Nothing changes while the queue is still waiting for a match.
            if !is_nested || (base_queued && !base_matched) {
                return;
            }
            if base_matched {
                nested.abandon();
                focus.dispatch(FocusAction::Unnest);
            } else if nested_playing {
                promote_nested(&focus);
            } else {
                focus.dispatch(FocusAction::Unnest);
            }
        },
    );
}

/// Makes the nested game the base mode, pointing the URL at it without navigating.
fn promote_nested(focus: &UseReducerHandle<ModeFocus>) {
    if let Some(mode_id) = focus.nested_mode() {
        replace_mode_hash(mode_id);
    }
    focus.dispatch(FocusAction::PromoteNested);
}

/// Rewrites the URL hash to `mode_id`.
///
/// Replacing the history entry fires no `hashchange` or `popstate` event, so the navigation
/// effect does not leave the game the URL now points at.
fn replace_mode_hash(mode_id: &ModeId) {
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&format!("#{mode_id}")));
    }
}
