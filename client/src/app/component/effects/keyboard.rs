//! Effects related to keyboard-driven join and rejoin flows.

use crate::ui_state::{CooldownSeconds, JoinStep};
use crate::utils::set_stored_name;
use common::models::KitSummary;
use common::types::KitId;
use gloo_events::EventListener;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::hook;
use yew::prelude::*;

/// Inputs for keyboard shortcut handling on the landing and end screens.
pub struct KeyboardShortcutEffectInputs {
    pub is_joined: bool,
    pub is_dead: bool,
    pub is_victory: bool,
    pub join_step: UseStateHandle<JoinStep>,
    pub landing_cooldown: UseStateHandle<CooldownSeconds>,
    pub disconnected: bool,
    pub queueing: bool,
    pub kits: Vec<KitSummary>,
    pub single_kit: Option<KitId>,
    pub player_name: UseStateHandle<String>,
    pub has_interacted: UseStateHandle<bool>,
    pub on_join: Callback<KitId>,
    pub on_cycle_mode: Callback<i32>,
    pub on_rejoin: Callback<MouseEvent>,
    pub rc_ref: Rc<RefCell<CooldownSeconds>>,
}

/// Handles keyboard shortcuts for joining and rejoining games.
#[hook]
pub fn use_keyboard_shortcuts_effect(inputs: KeyboardShortcutEffectInputs) {
    let KeyboardShortcutEffectInputs {
        is_joined,
        is_dead,
        is_victory,
        join_step,
        landing_cooldown,
        disconnected,
        queueing,
        kits,
        single_kit,
        player_name,
        has_interacted,
        on_join,
        on_cycle_mode,
        on_rejoin,
        rc_ref,
    } = inputs;
    let join_step = join_step.clone();
    let player_name = player_name.clone();
    let landing_cooldown = landing_cooldown.clone();
    let has_interacted = has_interacted.clone();
    let on_join = on_join.clone();
    let on_cycle_mode = on_cycle_mode.clone();
    let on_rejoin = on_rejoin.clone();
    let rc_ref = rc_ref.clone();

    use_effect_with(
        (
            is_joined,
            is_dead,
            is_victory,
            *join_step,
            *landing_cooldown,
            disconnected,
            queueing,
            kits.clone(),
            single_kit.clone(),
            on_join.clone(),
            on_cycle_mode.clone(),
            on_rejoin.clone(),
        ),
        move |&(
            joined,
            dead,
            victory,
            step,
            lc,
            disc,
            queueing,
            ref kits,
            ref single_kit,
            ref on_join,
            ref on_cycle_mode,
            ref on_rejoin,
        )| {
            let on_join = on_join.clone();
            let on_cycle_mode = on_cycle_mode.clone();
            let on_rejoin = on_rejoin.clone();
            let rc_ref = rc_ref.clone();
            let kits = kits.clone();
            let single_kit = single_kit.clone();

            let listener = EventListener::new(&web_sys::window().unwrap(), "keydown", move |e| {
                let e = e.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                if event_targets_text_input(e) {
                    return;
                }
                let key = e.key();
                if !joined
                    && !dead
                    && !victory
                    && step.is_enter_name()
                    && !disc
                    && !queueing
                    && (key == "ArrowUp" || key == "ArrowDown")
                {
                    e.prevent_default();
                    on_cycle_mode.emit(if key == "ArrowUp" { -1 } else { 1 });
                    return;
                }
                if key == "Enter" {
                    if !joined && !dead && !victory {
                        try_submit_join(JoinShortcutContext {
                            step,
                            landing_cooldown: lc,
                            disconnected: disc,
                            player_name: &player_name,
                            single_kit: &single_kit,
                            join_step: &join_step,
                            on_join: &on_join,
                            has_interacted: &has_interacted,
                        });
                    } else if (dead || victory) && rc_ref.borrow().is_zero() && !disc {
                        on_rejoin.emit(MouseEvent::new("click").unwrap());
                        has_interacted.set(true);
                    }
                } else {
                    try_submit_kit_hotkey(KitHotkeyContext {
                        key: &key,
                        joined,
                        dead,
                        victory,
                        step,
                        queueing,
                        disconnected: disc,
                        kits: &kits,
                        on_join: &on_join,
                        has_interacted: &has_interacted,
                    });
                }
            });
            move || drop(listener)
        },
    );
}

fn event_targets_text_input(event: &web_sys::KeyboardEvent) -> bool {
    let Some(target) = event.target() else {
        return false;
    };
    let Ok(element) = target.dyn_into::<web_sys::Element>() else {
        return false;
    };
    element
        .closest("input, textarea, [contenteditable='true'], [data-chat-input]")
        .ok()
        .flatten()
        .is_some()
}

fn try_submit_join(context: JoinShortcutContext<'_>) {
    if !context.step.is_enter_name() || !context.landing_cooldown.is_zero() || context.disconnected
    {
        return;
    }

    let name = (**context.player_name).trim().to_string();
    set_stored_name(&name);
    if let Some(kit_id) = context.single_kit.clone() {
        context.join_step.set(JoinStep::SelectKit);
        context.on_join.emit(kit_id);
    } else {
        context.join_step.set(JoinStep::SelectKit);
    }
    context.has_interacted.set(true);
}

fn try_submit_kit_hotkey(context: KitHotkeyContext<'_>) {
    if context.joined
        || context.dead
        || context.victory
        || !context.step.is_select_kit()
        || context.queueing
        || context.disconnected
    {
        return;
    }

    let Ok(num) = context.key.parse::<usize>() else {
        return;
    };
    if num == 0 || num > context.kits.len() {
        return;
    }
    if let Some(kit) = context.kits.get(num - 1) {
        context.on_join.emit(kit.name.clone());
        context.has_interacted.set(true);
    }
}

/// Inputs needed to submit the Enter-key join action.
struct JoinShortcutContext<'a> {
    step: JoinStep,
    landing_cooldown: CooldownSeconds,
    disconnected: bool,
    player_name: &'a UseStateHandle<String>,
    single_kit: &'a Option<KitId>,
    join_step: &'a UseStateHandle<JoinStep>,
    on_join: &'a Callback<KitId>,
    has_interacted: &'a UseStateHandle<bool>,
}

/// Inputs needed to resolve a numeric kit hotkey.
struct KitHotkeyContext<'a> {
    key: &'a str,
    joined: bool,
    dead: bool,
    victory: bool,
    step: JoinStep,
    queueing: bool,
    disconnected: bool,
    kits: &'a [KitSummary],
    on_join: &'a Callback<KitId>,
    has_interacted: &'a UseStateHandle<bool>,
}
