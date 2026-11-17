//! Move history panel of the replay viewer, with buttons to step through it.

use super::history::{EntryIndex, HistoryEntry, HistoryStep, MoveHistory};
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
    /// Entry of the board shown now.
    pub(super) current: EntryIndex,
    /// Shows the board after a move picked from the list.
    pub(super) on_show: Callback<EntryIndex>,
    pub(super) on_step: Callback<HistoryStep>,
}

/// Buttons below the list, in the order shown, with their symbol and title.
const NAV_BUTTONS: [(HistoryStep, &str, &str); 4] = [
    (HistoryStep::PreviousRow, "▲", "Previous row (↑)"),
    (HistoryStep::PreviousMove, "◀", "Previous move (←)"),
    (HistoryStep::NextMove, "▶", "Next move (→)"),
    (HistoryStep::NextRow, "▼", "Next row (↓)"),
];

#[function_component(MoveList)]
/// Lists the moves of the replay one row per stretch of time, highlighting the one shown.
pub(super) fn move_list(props: &MoveListProps) -> Html {
    let list_ref = use_node_ref();
    let current_ref = use_node_ref();
    use_keep_in_view(&list_ref, &current_ref, props.current);

    let history = &props.history.0;
    // A tick is one box that wraps its moves inside it, so the tick shown stays a single
    // highlight however many lines it takes.
    let entry = |index: EntryIndex, entry: &HistoryEntry| {
        let is_current = index == props.current;
        let moves = entry.moves.iter().map(|piece_move| {
            html! {
                <span class="replay-move">
                    <span
                        class="replay-swatch"
                        style={format!(
                            "background: {};",
                            piece_move.color.as_ref().map_or("#94a3b8", |color| color.as_ref()),
                        )}
                    ></span>
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
    let buttons = NAV_BUTTONS.iter().map(|&(step, symbol, title)| {
        html! {
            <button title={title} aria-label={title} onclick={props.on_step.reform(move |_| step)}>
                {symbol}
            </button>
        }
    });

    html! {
        <>
            <div class="replay-moves" data-testid="replay-moves" ref={list_ref}>
                { for rows }
            </div>
            <div class="replay-nav" data-testid="replay-nav">
                { for buttons }
            </div>
        </>
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
