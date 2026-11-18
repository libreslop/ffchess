//! Move history panel of the replay viewer, and the buttons to step through it.

use super::history::{EntryIndex, HistoryEntry, HistoryStep, MoveHistory};
use super::piece_icons::SharedIcons;
use super::playback::PlaybackButton;
use crate::canvas::color::NPC_PIECE_COLOR;
use std::rc::Rc;
use web_sys::HtmlElement;
use yew::prelude::*;

/// Shared handle to a move history, compared by identity so re-renders stay cheap.
#[derive(Clone)]
pub(super) struct SharedHistory(pub(super) Rc<MoveHistory>);

impl PartialEq for SharedHistory {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Properties for the move list.
#[derive(Properties, PartialEq)]
pub(super) struct MoveListProps {
    pub(super) history: SharedHistory,
    pub(super) icons: SharedIcons,
    /// Entry of the board shown now.
    pub(super) current: EntryIndex,
    /// Shows the board after a move picked from the list.
    pub(super) on_show: Callback<EntryIndex>,
}

/// Properties for the buttons stepping through the move history.
#[derive(Properties, PartialEq)]
pub(super) struct MoveNavProps {
    pub(super) on_step: Callback<HistoryStep>,
    /// Button lit as pressed by a key or the wheel.
    pub(super) pressed: Option<PlaybackButton>,
}

/// Buttons stepping through the history, in the order shown, with the turn of their upward arrow in
/// degrees and their title.
const NAV_BUTTONS: [(HistoryStep, u32, &str); 4] = [
    (HistoryStep::PreviousRow, 0, "Previous row (↑)"),
    (HistoryStep::PreviousMove, 270, "Previous move (←)"),
    (HistoryStep::NextMove, 90, "Next move (→)"),
    (HistoryStep::NextRow, 180, "Next row (↓)"),
];

#[function_component(MoveList)]
/// Lists the moves of the replay one row per stretch of time, highlighting the one shown.
pub(super) fn move_list(props: &MoveListProps) -> Html {
    let list_ref = use_node_ref();
    let current_ref = use_node_ref();
    use_keep_in_view(&list_ref, &current_ref, props.current);

    let history = &props.history.0;
    let icons = &props.icons.0;
    // A tick is one box that wraps its moves inside it, so the tick shown stays a single
    // highlight however many lines it takes.
    let entry = |index: EntryIndex, entry: &HistoryEntry| {
        let is_current = index == props.current;
        // The icon in its owner's color names both the piece and its player.
        let moves = entry.moves.iter().map(|piece_move| {
            let color = piece_move.color.as_ref();
            html! {
                <span class="replay-move">
                    if let Some(icon) = icons.icon(&piece_move.piece_type, color) {
                        <img class="replay-piece" src={icon} alt="" />
                    } else {
                        <span
                            class="replay-swatch"
                            style={format!(
                                "background: {};",
                                color.map_or(NPC_PIECE_COLOR, |color| color.as_ref()),
                            )}
                        ></span>
                    }
                    {piece_move.notation.clone()}
                </span>
            }
        });
        html! {
            <button
                class={classes!("replay-entry", is_current.then_some("current"))}
                ref={if is_current { current_ref.clone() } else { NodeRef::default() }}
                onclick={props.on_show.reform(move |_| index)}
            >
                if entry.moves.is_empty() {
                    <span class="replay-move replay-start">{"start"}</span>
                } else {
                    { for moves }
                }
            </button>
        }
    };
    let rows = history.rows().iter().map(|row| {
        html! {
            <div class="replay-row">
                <span class="replay-row-time">{row.ends_after.as_secs_f64().to_string()}</span>
                <div class="replay-row-moves">
                    { for row.entries.clone().map(|index| entry(index, &history.entries()[index])) }
                </div>
            </div>
        }
    });

    html! {
        <div class="replay-moves" data-testid="replay-moves" ref={list_ref}>
            { for rows }
        </div>
    }
}

#[function_component(MoveNav)]
/// Renders a row of buttons stepping to the previous or next move or row of the history.
pub(super) fn move_nav(props: &MoveNavProps) -> Html {
    let buttons = NAV_BUTTONS.iter().map(|&(step, turn, title)| {
        html! {
            <button
                title={title}
                aria-label={title}
                class={classes!((props.pressed == Some(PlaybackButton::Step(step))).then_some("pressed"))}
                onclick={props.on_step.reform(move |_| step)}
            >
                <span
                    class="replay-icon replay-icon-arrow"
                    style={format!("transform: rotate({turn}deg);")}
                ></span>
            </button>
        }
    });

    html! {
        <div class="replay-nav" data-testid="replay-nav">
            { for buttons }
        </div>
    }
}

/// Scrolls the list whenever the shown entry changes, so it sits in the middle of the list.
#[hook]
fn use_keep_in_view(list_ref: &NodeRef, current_ref: &NodeRef, current: EntryIndex) {
    let list_ref = list_ref.clone();
    let current_ref = current_ref.clone();
    use_effect_with(current, move |_| {
        if let (Some(list), Some(entry)) = (
            list_ref.cast::<HtmlElement>(),
            current_ref.cast::<HtmlElement>(),
        ) {
            let middle = entry.offset_top() - (list.client_height() - entry.offset_height()) / 2;
            list.set_scroll_top(middle.max(0));
        }
    });
}
