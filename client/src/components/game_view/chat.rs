//! Room chat overlay for the game view.

use crate::reducer::{ClientPhase, GameAction, GameStateReducer, MsgSender};
use common::protocol::{ChatLine, ClientMessage};
use common::types::TimestampMs;
use gloo_events::EventListener;
use gloo_timers::callback::Timeout;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Fade duration applied after the full chat TTL has elapsed.
const CHAT_FADE_OUT_MS: i64 = 500;

/// Props for the room chat overlay.
#[derive(Properties, PartialEq)]
pub struct GameChatProps {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub tx: MsgSender,
    pub globals: crate::app::GlobalClientConfig,
    pub player_name: String,
}

/// Visual surface behind the chat UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChatSurface {
    LightBoard,
    DarkOverlay,
}

/// Chat color palette chosen for the current UI surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChatTheme {
    message_color: &'static str,
    message_shadow: &'static str,
    input_color: &'static str,
    input_shadow: &'static str,
    link_color: &'static str,
}

impl ChatTheme {
    /// Returns the chat palette for one surface type.
    fn for_surface(surface: ChatSurface) -> Self {
        match surface {
            ChatSurface::LightBoard => Self {
                message_color: "#6b7280",
                message_shadow: "#374151",
                input_color: "#6b7280",
                input_shadow: "#374151",
                link_color: "#1d4ed8",
            },
            ChatSurface::DarkOverlay => Self {
                message_color: "#d1d5db",
                message_shadow: "#374151",
                input_color: "#d1d5db",
                input_shadow: "#374151",
                link_color: "#93c5fd",
            },
        }
    }

    /// Returns the link drop-shadow color for this theme.
    fn link_shadow(self) -> String {
        shift_color_darker(self.link_color)
    }

    /// Returns the sender-name drop-shadow color for this theme.
    fn sender_shadow(self, sender_color: &str) -> String {
        shift_color_darker(sender_color)
    }
}

/// Stable identity for one rendered chat line.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ChatLineKey(String);

impl ChatLineKey {
    /// Builds a key from the immutable fields carried by the server.
    fn for_line(line: &ChatLine) -> Self {
        Self(format!(
            "{}:{}:{}:{}",
            line.sent_at.as_i64(),
            line.sender_name,
            line.sender_color.as_ref(),
            line.message
        ))
    }
}

#[function_component(GameChat)]
/// Renders the room chat overlay and manages chat-specific timers.
pub fn game_chat(props: &GameChatProps) -> Html {
    let chat_input_ref = use_node_ref();
    let chat_log_ref = use_node_ref();
    let chat_text = use_state(String::new);
    let expired_line_keys = use_state(HashSet::<ChatLineKey>::new);

    {
        let chat_input_ref = chat_input_ref.clone();
        use_effect_with((), move |_| {
            let listener =
                EventListener::new(&web_sys::window().unwrap(), "pointerdown", move |event| {
                    let Some(target) = event.target() else {
                        return;
                    };
                    let Ok(target_el) = target.dyn_into::<web_sys::Element>() else {
                        return;
                    };
                    if chat_input_ref.cast::<web_sys::Element>().is_none() {
                        return;
                    }
                    let target_is_input = target_el
                        .closest("[data-chat-input]")
                        .ok()
                        .flatten()
                        .is_some();
                    if !target_is_input
                        && let Some(input) = chat_input_ref.cast::<HtmlInputElement>()
                    {
                        let _ = input.blur();
                    }
                });
            move || drop(listener)
        });
    }

    {
        let chat_log_ref = chat_log_ref.clone();
        use_effect_with(props.reducer.chat_lines.len(), move |_| {
            if let Some(el) = chat_log_ref.cast::<web_sys::HtmlElement>() {
                el.set_scroll_top(el.scroll_height());
            }
            || ()
        });
    }

    {
        let chat_text = chat_text.clone();
        let expired_line_keys = expired_line_keys.clone();
        use_effect_with(props.reducer.chat_room_key.clone(), move |_| {
            chat_text.set(String::new());
            expired_line_keys.set(HashSet::new());
            || ()
        });
    }

    {
        let expired_line_keys = expired_line_keys.clone();
        use_effect_with(
            (
                props.reducer.chat_lines.clone(),
                props.globals.chat_message_ttl_ms,
                props.reducer.clock_offset_ms,
            ),
            move |(lines, ttl_ms, clock_offset_ms)| {
                let current_keys = current_chat_line_keys(lines);
                let mut retained = (*expired_line_keys).clone();
                retained.retain(|key| current_keys.contains(key));
                if retained != *expired_line_keys {
                    expired_line_keys.set(retained.clone());
                }

                let now = shifted_now_ms(*clock_offset_ms);
                let mut timeouts = Vec::new();
                for line in lines {
                    let key = ChatLineKey::for_line(line);
                    if retained.contains(&key) {
                        continue;
                    }
                    let age_ms = now.as_i64().saturating_sub(line.sent_at.as_i64());
                    let delay_ms = ((*ttl_ms).max(1) as i64).saturating_sub(age_ms).max(0);
                    let expired_line_keys = expired_line_keys.clone();
                    timeouts.push(Timeout::new(clamp_timeout_ms(delay_ms), move || {
                        let mut next = (*expired_line_keys).clone();
                        next.insert(key.clone());
                        expired_line_keys.set(next);
                    }));
                }
                move || drop(timeouts)
            },
        );
    }

    {
        let reducer = props.reducer.clone();
        use_effect_with(
            (
                props.reducer.chat_lines.clone(),
                props.globals.chat_message_ttl_ms,
                props.reducer.clock_offset_ms,
            ),
            move |(lines, ttl_ms, clock_offset_ms)| {
                let ttl_ms = *ttl_ms;
                let clock_offset_ms = *clock_offset_ms;
                let timeout = next_prune_delay_ms(lines, ttl_ms, clock_offset_ms).map(
                    |next_delay_ms| {
                        let reducer = reducer.clone();
                        Timeout::new(clamp_timeout_ms(next_delay_ms), move || {
                            reducer.dispatch(GameAction::PruneExpiredChat {
                                now: shifted_now_ms(clock_offset_ms),
                                ttl_ms,
                            });
                        })
                    },
                );
                move || drop(timeout)
            },
        );
    }

    let surface = chat_surface(&props.reducer);
    let theme = ChatTheme::for_surface(surface);
    let chat_char_count = (*chat_text).chars().count() as u32;
    let chat_max_chars = props.globals.chat_message_max_chars.max(1);
    let chat_warning_chars = props.globals.chat_warning_chars.min(chat_max_chars);
    let show_chat_counter = chat_char_count >= chat_warning_chars;

    let chat_on_input = {
        let chat_text = chat_text.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                chat_text.set(input.value());
            }
        })
    };

    let submit_chat_message = {
        let tx = props.tx.clone();
        let chat_text = chat_text.clone();
        let chat_input_ref = chat_input_ref.clone();
        let player_name = props.player_name.clone();
        Callback::from(move |_| {
            let message = (*chat_text).trim().to_string();
            if message.is_empty() {
                return;
            }
            let sent = try_send_client_message(
                &tx,
                ClientMessage::Chat {
                    name_hint: player_name.clone(),
                    message,
                },
                "failed to send Chat",
            );
            if sent {
                chat_text.set(String::new());
            }
            if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                let _ = input.focus();
            }
        })
    };

    let chat_on_keydown = {
        let submit_chat_message = submit_chat_message.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Enter" {
                event.prevent_default();
                submit_chat_message.emit(());
            }
            event.stop_propagation();
        })
    };

    let on_chat_submit = {
        let submit_chat_message = submit_chat_message.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            event.stop_propagation();
            submit_chat_message.emit(());
        })
    };

    html! {
        <div
            data-ui-exempt="true"
            data-chat-ui="true"
            style="position: absolute; left: 12px; bottom: 12px; width: min(460px, calc(100vw - 24px)); max-height: 38vh; display: flex; flex-direction: column; gap: 0; z-index: 160;"
        >
            <style>
                {format!(
                    ".ff-chat-input::placeholder {{ color: {}; text-shadow: 1px 1px 0 {}; opacity: 1; }}",
                    theme.input_color,
                    theme.input_shadow
                )}
            </style>
            <div
                data-chat-ui="true"
                ref={chat_log_ref}
                style="padding: 0; overflow-y: auto;"
            >
                {
                    html! {
                        for props.reducer.chat_lines.iter().map(|line| {
                            let key = ChatLineKey::for_line(line);
                            let is_expired = expired_line_keys.contains(&key);
                            let sender_color =
                                display_sender_color(line.sender_color.as_ref(), surface);
                            let sender_shadow = theme.sender_shadow(sender_color);
                            let link_shadow = theme.link_shadow();
                            html! {
                                <div
                                    key={key.0.clone()}
                                    style={format!(
                                        "font-size: 12px; line-height: 1.6; min-height: 1.6em; overflow-wrap: anywhere; opacity: {}; transition: opacity {}ms linear; will-change: opacity;",
                                        if is_expired { "0" } else { "1" },
                                        CHAT_FADE_OUT_MS
                                    )}
                                >
                                    <span style={format!("color: {}; font-weight: 700; text-shadow: 1px 1px 0 {};", sender_color, sender_shadow)}>
                                        {line.sender_name.clone()}
                                    </span>
                                    <span style={format!("color: {}; text-shadow: 1px 1px 0 {};", theme.message_color, theme.message_shadow)}>
                                        {" : "}{render_chat_message_with_links(&line.message, theme.link_color, &link_shadow)}
                                    </span>
                                </div>
                            }
                        })
                    }
                }
            </div>
            <form data-chat-ui="true" onsubmit={on_chat_submit}>
                <div data-chat-ui="true" style="position: relative; min-height: 1.6em;">
                    <input
                        ref={chat_input_ref}
                        class="ff-chat-input"
                        data-ui-exempt="true"
                        data-chat-ui="true"
                        data-chat-input="true"
                        type="text"
                        value={(*chat_text).clone()}
                        oninput={chat_on_input}
                        onkeydown={chat_on_keydown}
                        maxlength={chat_max_chars.to_string()}
                        placeholder="Write a message"
                        style={format!(
                            "width: 100%; border: 0; background: transparent; color: {}; text-shadow: 1px 1px 0 {}; caret-color: {}; font-size: 12px; line-height: 1.6; min-height: 1.6em; padding: 0; padding-right: 68px; outline: none;",
                            theme.input_color,
                            theme.input_shadow,
                            theme.input_color
                        )}
                        autocomplete="off"
                    />
                    {
                        if show_chat_counter {
                            html! {
                                <span
                                    data-chat-ui="true"
                                    style={format!(
                                        "position: absolute; right: 0; top: 0; font-size: 12px; line-height: 1.6; color: {}; text-shadow: 1px 1px 0 {}; pointer-events: none;",
                                        theme.input_color,
                                        theme.input_shadow
                                    )}
                                >
                                    {format!("{}/{}", chat_char_count, chat_max_chars)}
                                </span>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </form>
        </div>
    }
}

fn display_sender_color(sender_color: &str, surface: ChatSurface) -> &str {
    if surface == ChatSurface::DarkOverlay && sender_color.eq_ignore_ascii_case("#555555") {
        "#9ca3af"
    } else {
        sender_color
    }
}

fn current_chat_line_keys(lines: &[ChatLine]) -> HashSet<ChatLineKey> {
    lines.iter().map(ChatLineKey::for_line).collect()
}

fn chat_surface(reducer: &GameStateReducer) -> ChatSurface {
    if reducer.phase == ClientPhase::Alive && !reducer.is_dead && !reducer.is_victory {
        ChatSurface::LightBoard
    } else {
        ChatSurface::DarkOverlay
    }
}

fn next_prune_delay_ms(lines: &[ChatLine], ttl_ms: u32, clock_offset_ms: i64) -> Option<i64> {
    let now = shifted_now_ms(clock_offset_ms);
    lines.iter()
        .map(|line| {
            let age_ms = now.as_i64().saturating_sub(line.sent_at.as_i64());
            (ttl_ms.max(1) as i64 + CHAT_FADE_OUT_MS)
                .saturating_sub(age_ms)
                .max(0)
        })
        .min()
}

fn shifted_now_ms(clock_offset_ms: i64) -> TimestampMs {
    TimestampMs::from_millis(js_sys::Date::now() as i64 + clock_offset_ms)
}

fn clamp_timeout_ms(delay_ms: i64) -> u32 {
    delay_ms.clamp(0, u32::MAX as i64) as u32
}

fn shift_color_darker(hex: &str) -> String {
    let hex = hex.trim();
    if hex.len() == 7
        && hex.starts_with('#')
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[1..3], 16),
            u8::from_str_radix(&hex[3..5], 16),
            u8::from_str_radix(&hex[5..7], 16),
        )
    {
        let convert = |v: u8| ((v as f32 * 0.45).round() as u8).max(10);
        return format!("#{:02x}{:02x}{:02x}", convert(r), convert(g), convert(b));
    }
    "#111827".to_string()
}

fn render_chat_message_with_links(message: &str, link_color: &str, link_shadow: &str) -> Html {
    let mut nodes: Vec<Html> = Vec::new();
    for (i, token) in message.split(' ').enumerate() {
        if i > 0 {
            nodes.push(html! { " " });
        }
        let (core, trailing) = split_url_trailing_punctuation(token);
        if let Some(href) = url_href(core) {
            nodes.push(html! {
                <a
                    href={href}
                    target="_blank"
                    rel="noopener noreferrer"
                    data-chat-ui="true"
                    style={format!(
                        "color: {}; text-shadow: 1px 1px 0 {}; text-decoration: underline;",
                        link_color,
                        link_shadow
                    )}
                >
                    {core}
                </a>
            });
            if !trailing.is_empty() {
                nodes.push(html! { trailing });
            }
        } else {
            nodes.push(html! { token });
        }
    }
    html! { for nodes }
}

fn split_url_trailing_punctuation(token: &str) -> (&str, &str) {
    let punct = [',', '.', '!', '?', ';', ':', ')', ']', '}'];
    let mut end = token.len();
    while end > 0 {
        let ch = token[..end].chars().next_back().unwrap_or_default();
        if punct.contains(&ch) {
            end -= ch.len_utf8();
        } else {
            break;
        }
    }
    (&token[..end], &token[end..])
}

fn url_href(token: &str) -> Option<String> {
    if token.starts_with("http://") || token.starts_with("https://") {
        return Some(token.to_string());
    }
    if token.starts_with("www.") {
        return Some(format!("https://{token}"));
    }
    None
}

fn try_send_client_message(tx: &MsgSender, message: ClientMessage, context: &str) -> bool {
    if tx.0.try_send(message).is_err() {
        web_sys::console::error_1(&context.into());
        return false;
    }
    true
}
