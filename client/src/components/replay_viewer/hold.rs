//! Steps through the move history that repeat while their key or button is held.

use super::history::HistoryStep;
use super::playback::{PlaybackButton, PlaybackCommand, PlaybackState};
use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

/// How long a held step waits before it starts repeating, in milliseconds.
const REPEAT_DELAY_MS: u32 = 400;
/// How often a held step repeats once it started, in milliseconds.
const REPEAT_INTERVAL_MS: u32 = 80;

/// A step being held: made once when pressed, then again and again after a pause, the way a
/// held key types.
struct HeldStep {
    step: HistoryStep,
    /// Waits out the pause, then starts the repeat.
    _delay: Timeout,
    /// Repeats the step once the pause is over. It is kept apart from the delay, so the delay
    /// starting it never has to drop itself while it runs.
    _repeat: Rc<RefCell<Option<Interval>>>,
}

/// Starts and stops held steps, lighting the button of the one held.
#[derive(Clone)]
pub(super) struct StepRepeater {
    held: Rc<RefCell<Option<HeldStep>>>,
    playback: UseReducerDispatcher<PlaybackState>,
    pressed: UseStateSetter<Option<PlaybackButton>>,
}

impl PartialEq for StepRepeater {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.held, &other.held)
    }
}

impl StepRepeater {
    /// Makes `step` and keeps making it until released, taking over from any step held before.
    pub(super) fn press(&self, step: HistoryStep) {
        let command = move || PlaybackCommand::Step(step);
        self.playback.dispatch(command());
        self.pressed.set(Some(PlaybackButton::Step(step)));
        let repeat = Rc::new(RefCell::new(None));
        let delay = {
            let repeat = repeat.clone();
            let playback = self.playback.clone();
            Timeout::new(REPEAT_DELAY_MS, move || {
                let interval =
                    Interval::new(REPEAT_INTERVAL_MS, move || playback.dispatch(command()));
                repeat.replace(Some(interval));
            })
        };
        self.held.replace(Some(HeldStep {
            step,
            _delay: delay,
            _repeat: repeat,
        }));
    }

    /// Stops repeating `step`, if it is the step held.
    pub(super) fn release(&self, step: HistoryStep) {
        let holds_step = self
            .held
            .borrow()
            .as_ref()
            .is_some_and(|held| held.step == step);
        if holds_step {
            self.release_any();
        }
    }

    /// Stops repeating whichever step is held.
    pub(super) fn release_any(&self) {
        if self.held.take().is_some() {
            self.pressed.set(None);
        }
    }
}

/// Returns the repeater of held steps, which stops repeating once the viewer closes.
#[hook]
pub(super) fn use_step_repeater(
    playback: UseReducerDispatcher<PlaybackState>,
    pressed: UseStateSetter<Option<PlaybackButton>>,
) -> StepRepeater {
    let held = use_mut_ref(|| None);
    StepRepeater {
        held,
        playback,
        pressed,
    }
}
