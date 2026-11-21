//! Share button of the replay panel and its menu: copy a link to the replay, download it, or
//! render it to a video.

use super::loaded::SharedReplay;
use super::video::{VideoRender, VideoSettings};
use crate::app::ReplayLink;
use crate::utils::download;
use gloo_events::EventListener;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, BlobPropertyBag, Node};
use yew::prelude::*;

/// What the panel title says in its place while a share action runs, or once it is done.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ShareNotice {
    Copied,
    /// A video is rendering, this many percent done.
    Rendering(u32),
    RenderFailed,
}

impl ShareNotice {
    /// Returns the text shown in place of the title.
    pub(super) fn text(self) -> String {
        match self {
            Self::Copied => "Link copied".to_string(),
            Self::Rendering(percent) => format!("Rendering {percent}%"),
            Self::RenderFailed => "Render failed".to_string(),
        }
    }

    /// Returns true when the notice stays until the next one replaces it, rather than fading.
    pub(super) fn is_lasting(self) -> bool {
        matches!(self, Self::Rendering(_))
    }
}

/// The replay the share actions work on, and how a video of it is drawn.
#[derive(Clone, PartialEq)]
pub(super) struct ShareTarget {
    pub(super) replay: SharedReplay,
    /// Whether the board is shown upside down, as the video shows it too.
    pub(super) rotated_180: bool,
    pub(super) video: VideoSettings,
}

/// Properties for the share button.
#[derive(Properties, PartialEq)]
pub(super) struct ShareMenuProps {
    pub(super) target: ShareTarget,
    /// Shows a notice in place of the title; `None` brings the title back.
    pub(super) on_notice: Callback<Option<ShareNotice>>,
}

/// One entry of the share menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShareAction {
    CopyLink,
    Download,
    RenderVideo,
}

impl ShareAction {
    const ALL: [Self; 3] = [Self::CopyLink, Self::Download, Self::RenderVideo];

    /// Returns the label of the entry.
    fn label(self) -> &'static str {
        match self {
            Self::CopyLink => "Copy link",
            Self::Download => "Download replay",
            Self::RenderVideo => "Render video",
        }
    }

    /// Returns the class of the entry's icon.
    fn icon_class(self) -> &'static str {
        match self {
            Self::CopyLink => "replay-icon-link",
            Self::Download => "replay-icon-download",
            Self::RenderVideo => "replay-icon-video",
        }
    }
}

#[function_component(ShareMenu)]
/// Renders the share button, which opens the menu of share actions below it.
pub(super) fn share_menu(props: &ShareMenuProps) -> Html {
    let open = use_state(|| false);
    let rendering = use_state(|| false);
    let menu_ref = use_node_ref();
    use_close_on_outside_press(&menu_ref, open.clone());

    let on_toggle = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(!*open))
    };
    let on_action = |action: ShareAction| {
        let open = open.clone();
        let rendering = rendering.clone();
        let target = props.target.clone();
        let on_notice = props.on_notice.clone();
        Callback::from(move |_: MouseEvent| {
            open.set(false);
            match action {
                ShareAction::CopyLink => target.copy_link(on_notice.clone()),
                ShareAction::Download => target.download(),
                ShareAction::RenderVideo if !*rendering => {
                    target.render_video(on_notice.clone(), rendering.clone());
                }
                ShareAction::RenderVideo => {}
            }
        })
    };

    html! {
        <div class="replay-share" ref={menu_ref}>
            <button
                data-testid="replay-share"
                title="Share replay"
                aria-label="Share replay"
                aria-expanded={(*open).to_string()}
                onclick={on_toggle}
                class={classes!("replay-button", "replay-icon-button", (*open).then_some("pressed"))}
            >
                <span class="replay-icon replay-icon-share"></span>
            </button>
            if *open {
                <div class="replay-menu" role="menu">
                    { for ShareAction::ALL.into_iter().map(|action| html! {
                        <button
                            role="menuitem"
                            class="replay-menu-item"
                            disabled={action == ShareAction::RenderVideo && *rendering}
                            onclick={on_action(action)}
                        >
                            <span class={classes!("replay-icon", action.icon_class())}></span>
                            <span>{action.label()}</span>
                        </button>
                    }) }
                </div>
            }
        </div>
    }
}

/// Closes the menu while `open` when a press lands outside `menu_ref`.
#[hook]
fn use_close_on_outside_press(menu_ref: &NodeRef, open: UseStateHandle<bool>) {
    use_effect_with((menu_ref.clone(), *open), move |(menu_ref, is_open)| {
        let menu_ref = menu_ref.clone();
        let listener = is_open.then(|| {
            EventListener::new(&gloo_utils::document(), "pointerdown", move |event| {
                let target = event
                    .target()
                    .and_then(|target| target.dyn_into::<Node>().ok());
                let inside = menu_ref
                    .get()
                    .is_some_and(|menu| menu.contains(target.as_ref()));
                if !inside {
                    open.set(false);
                }
            })
        });
        move || drop(listener)
    });
}

impl ShareTarget {
    /// Copies the link that opens the replay, telling `on_notice` once it is copied.
    fn copy_link(&self, on_notice: Callback<Option<ShareNotice>>) {
        let replay = &self.replay.0;
        let Some(link) = ReplayLink::share_url(replay.id, &replay.header.mode.id) else {
            return;
        };
        spawn_local(async move {
            if copy_to_clipboard(&link).await {
                on_notice.emit(Some(ShareNotice::Copied));
            }
        });
    }

    /// Downloads the replay as the server sent it.
    fn download(&self) {
        let replay = &self.replay.0;
        let options = BlobPropertyBag::new();
        options.set_type("application/json");
        let parts = js_sys::Array::of1(&JsValue::from_str(&replay.source));
        if let Ok(blob) = Blob::new_with_str_sequence_and_options(&parts, &options) {
            download(&blob, &format!("{}.ffr.json", replay.file_stem()));
        }
    }

    /// Renders the replay to a video and downloads it, showing its progress through
    /// `on_notice`, with `rendering` set meanwhile.
    fn render_video(
        &self,
        on_notice: Callback<Option<ShareNotice>>,
        rendering: UseStateHandle<bool>,
    ) {
        let replay = self.replay.0.clone();
        let render = VideoRender::new(replay.clone(), self.rotated_180, self.video);
        rendering.set(true);
        spawn_local(async move {
            on_notice.emit(Some(ShareNotice::Rendering(0)));
            let video = render
                .run(|percent| on_notice.emit(Some(ShareNotice::Rendering(percent))))
                .await;
            match video {
                Ok(video) => {
                    download(&video, &format!("{}.webm", replay.file_stem()));
                    on_notice.emit(None);
                }
                Err(error) => {
                    web_sys::console::error_1(&error);
                    on_notice.emit(Some(ShareNotice::RenderFailed));
                }
            }
            rendering.set(false);
        });
    }
}

/// Copies `text` to the clipboard, returning true once it is there.
async fn copy_to_clipboard(text: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let promise = window.navigator().clipboard().write_text(text);
    JsFuture::from(promise).await.is_ok()
}
