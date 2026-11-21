//! Title bar of the replay panel, with the buttons that flip the board and share the replay.

use super::share_menu::{ShareMenu, ShareNotice, ShareTarget};
use gloo_timers::callback::Timeout;
use yew::prelude::*;

/// Title of the replay panel.
const TITLE: &str = "Analysis board";
/// How long a passing notice stays in place of the title, in milliseconds.
const NOTICE_MS: u32 = 1500;

/// Properties for the replay title bar.
#[derive(Properties, PartialEq)]
pub(super) struct ReplayTitleBarProps {
    pub(super) share: ShareTarget,
    pub(super) on_flip: Callback<()>,
}

#[function_component(ReplayTitleBar)]
/// Renders the panel title, with the flip and share buttons on its right.
pub(super) fn replay_title_bar(props: &ReplayTitleBarProps) -> Html {
    let notice = use_state(|| None::<ShareNotice>);
    // Kept so a new notice restarts the wait instead of an earlier one cutting it short.
    let fade = use_mut_ref(|| None::<Timeout>);
    let on_notice = {
        let notice = notice.clone();
        Callback::from(move |next: Option<ShareNotice>| {
            notice.set(next);
            let notice = notice.clone();
            let timeout = next
                .filter(|next| !next.is_lasting())
                .map(|_| Timeout::new(NOTICE_MS, move || notice.set(None)));
            fade.replace(timeout);
        })
    };
    let title = notice.map_or_else(|| TITLE.to_string(), ShareNotice::text);

    html! {
        <div class="replay-title">
            <span class="replay-title-text">{title}</span>
            <button
                data-testid="replay-flip"
                title="Flip board"
                aria-label="Flip board"
                onclick={props.on_flip.reform(|_| ())}
                class="replay-button replay-icon-button"
            >
                <span class="replay-icon replay-icon-flip"></span>
            </button>
            <ShareMenu target={props.share.clone()} on_notice={on_notice} />
        </div>
    }
}
