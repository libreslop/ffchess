//! Which modes the client is connected to and which one the player sees.

use common::types::ModeId;
use std::rc::Rc;
use yew::prelude::*;

/// One of the two websocket connections the client can hold at once.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Slot {
    #[default]
    First,
    Second,
}

impl Slot {
    /// Returns the other slot.
    pub const fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }

    /// Returns a stable name of the slot, used as a component key.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Second => "second",
        }
    }
}

/// A value kept for each connection slot.
#[derive(Clone)]
pub struct PerSlot<T> {
    first: T,
    second: T,
}

impl<T> PerSlot<T> {
    /// Pairs the values of the first and second slot.
    pub const fn new(first: T, second: T) -> Self {
        Self { first, second }
    }

    /// Returns the value of `slot`.
    pub const fn get(&self, slot: Slot) -> &T {
        match slot {
            Slot::First => &self.first,
            Slot::Second => &self.second,
        }
    }
}

/// The base mode picked from the menu, plus a quick-play game nested inside it while the base
/// mode's queue waits for players.
///
/// Each mode keeps its own connection slot, so the queue survives while the nested game runs,
/// and the nested game survives when it replaces the queue as the base mode.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ModeFocus {
    base_slot: Slot,
    base_mode: ModeId,
    nested_mode: Option<ModeId>,
}

/// Changes of the mode focus.
pub enum FocusAction {
    /// Shows a mode picked from the menu or the URL, dropping any nested game.
    Navigate(ModeId),
    /// Plays a quick-play mode on top of the queued base mode.
    Nest(ModeId),
    /// Ends the nested game, going back to the base mode.
    Unnest,
    /// Drops the base mode, leaving the nested game as the only one.
    PromoteNested,
}

impl Reducible for ModeFocus {
    type Action = FocusAction;

    /// Applies a focus change and returns the next focus.
    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            FocusAction::Navigate(mode_id) => {
                next.base_mode = mode_id;
                next.nested_mode = None;
            }
            FocusAction::Nest(mode_id) => next.nested_mode = Some(mode_id),
            FocusAction::Unnest => next.nested_mode = None,
            FocusAction::PromoteNested => {
                if let Some(nested_mode) = next.nested_mode.take() {
                    next.base_slot = next.base_slot.other();
                    next.base_mode = nested_mode;
                }
            }
        }
        next.into()
    }
}

impl ModeFocus {
    /// Focuses `base_mode` with nothing nested.
    pub const fn new(base_mode: ModeId) -> Self {
        Self {
            base_slot: Slot::First,
            base_mode,
            nested_mode: None,
        }
    }

    /// Returns the mode picked from the menu.
    pub const fn base_mode(&self) -> &ModeId {
        &self.base_mode
    }

    /// Returns the slot connected to the base mode.
    pub const fn base_slot(&self) -> Slot {
        self.base_slot
    }

    /// Returns the quick-play mode played while the base mode queues.
    pub const fn nested_mode(&self) -> Option<&ModeId> {
        self.nested_mode.as_ref()
    }

    /// Returns true while a quick-play game runs on top of the base mode.
    pub const fn is_nested(&self) -> bool {
        self.nested_mode.is_some()
    }

    /// Returns the slot of the mode the player sees.
    pub const fn focused_slot(&self) -> Slot {
        if self.is_nested() {
            self.base_slot.other()
        } else {
            self.base_slot
        }
    }

    /// Returns the mode the player sees.
    pub fn focused_mode(&self) -> &ModeId {
        self.nested_mode.as_ref().unwrap_or(&self.base_mode)
    }

    /// Returns the mode `slot` connects to, or `None` when the slot is idle.
    pub fn mode_of(&self, slot: Slot) -> Option<ModeId> {
        if slot == self.base_slot {
            Some(self.base_mode.clone())
        } else {
            self.nested_mode.clone()
        }
    }
}
