//! Root Yew component orchestrating UI state and network connections.

use crate::app::config::{load_global_config, order_modes};
use crate::app::ws::connect_ws;
use crate::components::{
    DisconnectedScreen, EndScreen, EndScreenKind, ErrorToast, FatalNotification, GameView,
    JoinScreen, Leaderboard,
};
use crate::reducer::{ClientPhase, GameAction, GameStateReducer, MsgSender};
use crate::ui_state::{CooldownSeconds, JoinStep, RejoinFlow};
use crate::utils::*;
use common::models::ModeSummary;
use common::protocol::ClientMessage;
use common::types::{KitId, ModeId, PlayerId};
use futures_util::future::{AbortHandle, Abortable};
use gloo_events::EventListener;
use gloo_net::websocket::Message;
use gloo_timers::callback::{Interval, Timeout};
use gloo_utils::document;
use std::rc::Rc;
use tokio::sync::mpsc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component(App)]
/// Renders the application shell and routes UI state to child components.
pub fn app() -> Html {
    let global_cfg = use_state(load_global_config);
    let reducer = use_reducer(GameStateReducer::default);
    let is_joining = use_state(|| false);
    let rejoin_flow = use_state(RejoinFlow::default);
    let tx = use_state(|| None::<MsgSender>);
    let player_name = use_state(get_stored_name);
    let join_step = use_state(JoinStep::default);
    let has_interacted = use_state(|| false);
    let show_disconnected = use_state(|| false);
    // Read initial mode list injected into index.html for immediate render
    let injected_modes: Vec<ModeSummary> = {
        let doc = document();
        if let Some(el) = doc.get_element_by_id("initial-modes") {
            if let Some(text) = el.text_content() {
                serde_json::from_str::<Vec<ModeSummary>>(&text).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };
    let injected_modes_copy = injected_modes.clone();

    let mode_options = {
        let order = global_cfg.game_order.clone();
        use_state(move || order_modes(injected_modes.clone(), &order))
    };

    // Read current mode info injected in index.html
    let injected_mode_info: Option<ModeSummary> = injected_modes_copy.first().cloned();

    // Determine current mode id from hash or global order/injected info
    let initial_mode_id = {
        let hash = web_sys::window()
            .unwrap()
            .location()
            .hash()
            .unwrap_or_default()
            .trim_start_matches('#')
            .to_string();
        if !hash.is_empty() {
            ModeId::from(hash)
        } else if let Some(first) = global_cfg.game_order.first() {
            first.clone()
        } else if let Some(m) = injected_mode_info.as_ref() {
            m.id.clone()
        } else {
            ModeId::from("ffa")
        }
    };
    let current_mode_id = use_state(|| initial_mode_id.clone());

    {
        let mode_options = mode_options.clone();
        let injected_mode_info = injected_mode_info.clone();
        let global_cfg = global_cfg.clone();
        use_effect_with((), move |_| {
            let modes_state = mode_options.clone();
            let injected = injected_mode_info.clone();
            let order = global_cfg.game_order.clone();
            let fetch_modes = Rc::new(move || {
                let modes_state = modes_state.clone();
                let injected = injected.clone();
                let order = order.clone();
                spawn_local(async move {
                    if let Ok(resp) = gloo_net::http::Request::get("/api/modes").send().await
                        && let Ok(list) = resp.json::<Vec<ModeSummary>>().await
                    {
                        modes_state.set(order_modes(list, &order));
                        return;
                    }
                    if let Some(info) = injected.clone() {
                        modes_state.set(order_modes(vec![info], &order));
                    }
                });
            });

            fetch_modes.clone()();
            let refresh_ms = global_cfg.modes_refresh_ms.max(500);
            let interval = Interval::new(refresh_ms, move || {
                fetch_modes.clone()();
            });
            || drop(interval)
        });
    }

    {
        let is_joining = is_joining.clone();
        use_effect_with(
            (
                reducer.player_id,
                reducer.error.clone(),
                reducer.queue_status.clone(),
                reducer.disconnected,
            ),
            move |_| {
                is_joining.set(false);
            },
        );
    }

    {
        let fatal_error = reducer.fatal_error;
        let reducer_handle = reducer.clone();
        use_effect_with(fatal_error, move |&fatal| {
            let mut timeout = None;
            if fatal {
                timeout = Some(Timeout::new(5000, move || {
                    reducer_handle.dispatch(GameAction::SetDisconnected {
                        disconnected: false,
                        is_fatal: false,
                        title: None,
                        msg: None,
                    });
                }));
            }
            move || {
                if let Some(t) = timeout {
                    drop(t);
                }
            }
        });
    }

    let landing_cooldown = {
        let initial_mode_id = initial_mode_id.clone();
        use_state(move || {
            let (ts, cd_ms) = get_death_info(&initial_mode_id);
            let now = common::types::TimestampMs::from_millis(js_sys::Date::now() as i64);
            let diff_ms = cd_ms - (now - ts);
            if diff_ms > common::types::DurationMs::zero() {
                CooldownSeconds::from_seconds((diff_ms.as_u64() / 1000) as u32)
            } else {
                CooldownSeconds::zero()
            }
        })
    };
    let lc_ref = use_mut_ref(|| *landing_cooldown);

    {
        let lc = landing_cooldown.clone();
        let lc_ref = lc_ref.clone();
        use_effect_with(*lc, move |&initial_lc| {
            let mut interval = None;
            if initial_lc.is_active() {
                *lc_ref.borrow_mut() = initial_lc;
                let lc_inner = lc.clone();
                let lr = lc_ref.clone();
                interval = Some(Interval::new(1000, move || {
                    let mut cur = *lr.borrow();
                    if cur.is_active() {
                        cur = cur.decrement();
                        *lr.borrow_mut() = cur;
                        lc_inner.set(cur);
                    }
                }));
            }
            || drop(interval)
        });
    }

    let reducer_ref = use_mut_ref(|| reducer.clone());
    *reducer_ref.borrow_mut() = reducer.clone();

    {
        let reducer_ref = reducer_ref.clone();
        let tx_handle = tx.clone();
        let global_cfg = global_cfg.clone();
        use_effect_with((*current_mode_id).clone(), move |mode_id| {
            reducer_ref.borrow().clone().dispatch(GameAction::Reset);
            let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ClientMessage>();
            let sender = MsgSender(client_tx);
            tx_handle.set(Some(sender.clone()));

            let tick_sender = sender.clone();
            let tick_reducer_ref = reducer_ref.clone();
            let tick_ms = global_cfg.tick_interval_ms.max(10);
            let interval = Interval::new(tick_ms, move || {
                let handle = tick_reducer_ref.borrow().clone();
                handle.dispatch(GameAction::Tick(tick_sender.clone()));
            });

            let ping_sender = sender.clone();
            let ping_interval_ms = global_cfg.ping_interval_ms.max(500);
            let ping_interval = Interval::new(ping_interval_ms, move || {
                let now = js_sys::Date::now() as u64;
                let _ = ping_sender.0.send(ClientMessage::Ping(now));
            });

            let listener_reducer_ref = reducer_ref.clone();
            let current_ws_tx = Rc::new(std::cell::RefCell::new(
                None::<mpsc::UnboundedSender<Message>>,
            ));

            let sender_ws_tx = current_ws_tx.clone();
            let sender_reducer_ref = reducer_ref.clone();
            spawn_local(async move {
                while let Some(msg) = client_rx.recv().await {
                    let maybe_tx = sender_ws_tx.borrow().clone();
                    let current_reducer = sender_reducer_ref.borrow().clone();
                    if let Some(tx) = maybe_tx {
                        if tx
                            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                            .is_err()
                            && !current_reducer.disconnected
                            && !current_reducer.fatal_error
                        {
                            sender_reducer_ref.borrow().clone().dispatch(
                                GameAction::SetDisconnected {
                                    disconnected: true,
                                    is_fatal: false,
                                    title: None,
                                    msg: None,
                                },
                            );
                        }
                    } else if !matches!(msg, ClientMessage::Ping(_))
                        && !current_reducer.disconnected
                        && !current_reducer.fatal_error
                    {
                        sender_reducer_ref
                            .borrow()
                            .clone()
                            .dispatch(GameAction::SetDisconnected {
                                disconnected: true,
                                is_fatal: false,
                                title: None,
                                msg: None,
                            });
                    }
                }
            });

            let (abort_handle, abort_reg) = AbortHandle::new_pair();
            {
                let listener_reducer_ref = listener_reducer_ref.clone();
                let current_ws_tx = current_ws_tx.clone();
                let mode_id = mode_id.clone();
                spawn_local(async move {
                    let window = web_sys::window().unwrap();
                    let host = window.location().host().unwrap();
                    let protocol = if window.location().protocol().unwrap() == "https:" {
                        "wss:"
                    } else {
                        "ws:"
                    };

                    let ws_url = format!("{}//{}/api/ws/{}", protocol, host, mode_id.as_ref());
                    let fut = Abortable::new(
                        connect_ws(
                            ws_url,
                            mode_id.clone(),
                            listener_reducer_ref.clone(),
                            current_ws_tx.clone(),
                        ),
                        abort_reg,
                    );
                    let _ = fut.await;
                });
            }
            move || {
                drop(interval);
                drop(ping_interval);
                abort_handle.abort();
            }
        });
    }

    let on_join = {
        let tx = tx.clone();
        let player_name = player_name.clone();
        let reducer_ref = reducer_ref.clone();
        let is_joining = is_joining.clone();
        let has_interacted = has_interacted.clone();
        let current_mode_id = current_mode_id.clone();
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
                is_joining.set(true);
                let stored_id = get_stored_id(&mode_id);
                let stored_secret = get_stored_secret(&mode_id);
                let _ = sender.0.send(ClientMessage::Join {
                    name: (*player_name).clone(),
                    kit_name,
                    player_id: stored_id,
                    session_secret: stored_secret,
                });
            }
        })
    };

    let on_name_input = {
        let player_name = player_name.clone();
        Callback::from(move |e: InputEvent| {
            player_name.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_name_submit = {
        let join_step = join_step.clone();
        let player_name = player_name.clone();
        let landing_cooldown = landing_cooldown.clone();
        let reducer = reducer.clone();
        let has_interacted = has_interacted.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            has_interacted.set(true);
            if reducer.disconnected || landing_cooldown.is_active() {
                return;
            }
            let name = (*player_name).trim().to_string();
            set_stored_name(&name);
            join_step.set(JoinStep::SelectKit);
        })
    };

    let player_id = reducer.player_id.unwrap_or_else(PlayerId::nil);
    let is_dead = reducer.is_dead;
    let is_victory = reducer.is_victory;
    let has_match_result = is_dead || is_victory;
    let is_joined = reducer.phase == ClientPhase::Alive || has_match_result;
    let force_join_overlay = rejoin_flow.forces_join_overlay(has_match_result);

    {
        let player_name = player_name.clone();
        use_effect_with(
            (
                reducer.player_id,
                reducer.state.players.clone(),
                (*player_name).clone(),
            ),
            move |(player_id, players, current_name)| {
                if current_name.trim().is_empty()
                    && let Some(pid) = *player_id
                    && pid != PlayerId::nil()
                    && let Some(player) = players.get(&pid)
                {
                    let server_name = player.name.trim();
                    if !server_name.is_empty() {
                        player_name.set(server_name.to_string());
                        set_stored_name(server_name);
                    }
                }
            },
        );
    }

    {
        let show_disconnected = show_disconnected.clone();
        let should_show =
            reducer.disconnected && !reducer.fatal_error && is_joined && !has_match_result;
        use_effect_with(should_show, move |&should| {
            if should {
                show_disconnected.set(true);
                Box::new(|| ()) as Box<dyn FnOnce()>
            } else {
                let sd = show_disconnected.clone();
                let handle = Timeout::new(300, move || {
                    sd.set(false);
                });
                Box::new(move || drop(handle)) as Box<dyn FnOnce()>
            }
        });
    }

    {
        let rejoin_flow = rejoin_flow.clone();
        use_effect_with(
            (
                reducer.phase,
                reducer.queue_status.clone(),
                has_match_result,
                *rejoin_flow,
            ),
            move |(phase, queue_status, has_match_result, flow)| {
                if flow.is_active()
                    && !*has_match_result
                    && *phase == ClientPhase::Alive
                    && queue_status.is_none()
                {
                    rejoin_flow.set(RejoinFlow::Inactive);
                }
            },
        );
    }

    let rejoin_cooldown = use_state(CooldownSeconds::zero);
    let rc_ref = use_mut_ref(CooldownSeconds::zero);
    {
        let rejoin_cooldown = rejoin_cooldown.clone();
        let rc_ref = rc_ref.clone();
        let current_mode_id = current_mode_id.clone();
        let reducer = reducer.clone();
        use_effect_with(has_match_result, move |has_match_result| {
            let mut interval = None;
            if *has_match_result {
                let cd_ms = reducer
                    .mode
                    .as_ref()
                    .map(|m| m.respawn_cooldown_ms)
                    .unwrap_or_else(|| common::types::DurationMs::from_millis(5000));
                set_death_timestamp(
                    &current_mode_id,
                    common::types::TimestampMs::from_millis(js_sys::Date::now() as i64),
                    cd_ms,
                );
                let cooldown_sec =
                    CooldownSeconds::from_seconds((cd_ms.as_u64() / 1000).max(1) as u32);
                rejoin_cooldown.set(cooldown_sec);
                *rc_ref.borrow_mut() = cooldown_sec;
                let rc = rejoin_cooldown.clone();
                let rr = rc_ref.clone();
                interval = Some(Interval::new(1000, move || {
                    let mut val = *rr.borrow();
                    if val.is_active() {
                        val = val.decrement();
                        *rr.borrow_mut() = val;
                        rc.set(val);
                    }
                }));
            }
            || drop(interval)
        });
    }

    let on_rejoin = {
        let rc_ref = rc_ref.clone();
        let reducer = reducer.clone();
        let join_step = join_step.clone();
        let has_interacted = has_interacted.clone();
        let rejoin_flow = rejoin_flow.clone();
        Callback::from(move |_| {
            if rc_ref.borrow().is_zero() {
                has_interacted.set(true);
                if reducer.disconnected {
                    return;
                }
                rejoin_flow.set(RejoinFlow::Active);
                reducer.dispatch(GameAction::Reset);
                join_step.set(JoinStep::SelectKit);
            }
        })
    };

    {
        let join_step = join_step.clone();
        let player_name = player_name.clone();
        let landing_cooldown = landing_cooldown.clone();
        let reducer = reducer.clone();
        let has_interacted = has_interacted.clone();
        let on_join = on_join.clone();
        let on_rejoin = on_rejoin.clone();
        let rc_ref = rc_ref.clone();
        let disconnected = reducer.disconnected;
        let queueing = reducer.queue_status.is_some();

        let kits = reducer
            .mode
            .as_ref()
            .map(|m| m.kits.clone())
            .unwrap_or_default();

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
            ),
            move |&(joined, dead, victory, step, lc, disc, queueing, ref kits)| {
                let on_join = on_join.clone();
                let on_rejoin = on_rejoin.clone();
                let rc_ref = rc_ref.clone();
                let kits = kits.clone();

                let listener =
                    EventListener::new(&web_sys::window().unwrap(), "keydown", move |e| {
                        let e = e.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                        let key = e.key();
                        if key == "Enter" {
                            if !joined && !dead && !victory {
                                if step.is_enter_name() && lc.is_zero() && !disc {
                                    let name = (*player_name).trim().to_string();
                                    set_stored_name(&name);
                                    join_step.set(JoinStep::SelectKit);
                                    has_interacted.set(true);
                                }
                            } else if (dead || victory) && rc_ref.borrow().is_zero() && !disc {
                                on_rejoin.emit(MouseEvent::new("click").unwrap());
                                has_interacted.set(true);
                            }
                        } else if !joined
                            && !dead
                            && !victory
                            && step.is_select_kit()
                            && !queueing
                            && !disc
                            && let Ok(num) = key.parse::<usize>()
                            && num > 0
                            && num <= kits.len()
                            && let Some(kit) = kits.get(num - 1)
                        {
                            on_join.emit(kit.name.clone());
                            has_interacted.set(true);
                        }
                    });
                move || drop(listener)
            },
        );
    }

    html! {
        <div style="margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; position: relative; background: #f0f2f5;">
            <style>{"
                @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
                @keyframes simpleFadeIn { from { opacity: 0; } to { opacity: 1; } }
                @keyframes fadeInOut { 0% { opacity: 0; transform: translate(-50%, 20px); } 15% { opacity: 1; transform: translate(-50%, 0); } 85% { opacity: 1; transform: translate(-50%, 0); } 100% { opacity: 0; transform: translate(-50%, -20px); } }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
            "}</style>

            if let Some(sender) = (*tx).clone() {
                <GameView
                    key="stable-game-view"
                    reducer={reducer.clone()}
                    tx={sender}
                    render_interval_ms={global_cfg.render_interval_ms}
                    globals={(*global_cfg).clone()}
                />
            } else if !*show_disconnected || !*has_interacted {
                <div style="position: absolute; inset: 0; background: #f0f2f5; display: flex; align-items: center; justify-content: center; z-index: 200;">
                    <div style="text-align: center;">
                        <h2 style="color: #64748b;">{"Connecting to server..."}</h2>
                        <div style="width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top: 4px solid #2563eb; border-radius: 50%; margin: 20px auto; animation: spin 1s linear infinite;"></div>
                    </div>
                </div>
            }

            if *show_disconnected {
                <DisconnectedScreen
                    show={true}
                    disconnected={reducer.disconnected && !reducer.fatal_error && is_joined && !has_match_result}
                    title={reducer.disconnected_title.clone()}
                    msg={reducer.disconnected_msg.clone()}
                />
            }

            <FatalNotification
                show={reducer.fatal_error}
                title={reducer.disconnected_title.clone()}
                msg={reducer.disconnected_msg.clone()}
            />

            if has_match_result && !force_join_overlay {
                <EndScreen
                    kind={if is_victory { EndScreenKind::Victory } else { EndScreenKind::Defeat }}
                    title={reducer.victory_title.clone()}
                    message={reducer.victory_msg.clone()}
                    score={reducer.last_score}
                    kills={reducer.last_kills}
                    captured={reducer.last_captured}
                    survival_secs={reducer.last_survival_secs}
                    on_rejoin={on_rejoin.clone()}
                    rejoin_cooldown={*rejoin_cooldown}
                />
            } else if is_joined && !force_join_overlay {
                    <div data-testid="in-game-hud">
                        <Leaderboard players={reducer.state.players.values().cloned().collect::<Vec<_>>()} self_id={player_id} />
                        <div
                            data-testid="stats-overlay"
                            class="pointer-events-none"
                            style="
                                position: fixed;
                                right: 4px;
                                bottom: 4px;
                                padding: 0;
                                background: transparent;
                                color: #000;
                                font-family: monospace;
                                font-size: 11px;
                                line-height: 1.2;
                                text-align: right;
                                z-index: 50;
                            "
                        >
                            <div>{format!("FPS: {}", reducer.fps)}</div>
                            <div>{format!("Ping: {}ms", reducer.ping_ms)}</div>
                            <div>{format!("Board: {}x{}", reducer.state.board_size, reducer.state.board_size)}</div>
                        </div>
                    </div>
            } else if (tx.is_some() && !has_match_result) || force_join_overlay {
                <JoinScreen
                    player_name={(*player_name).clone()}
                    on_name_input={on_name_input}
                    on_name_submit={on_name_submit}
                    landing_cooldown={*landing_cooldown}
                    join_step={*join_step}
                    on_join={on_join}
                    error={reducer.error.clone()}
                    queue_status={reducer.queue_status.clone()}
                    is_loading={*is_joining}
                    mode={reducer.mode.clone()}
                    mode_options={(*mode_options).clone()}
                    selected_mode_id={(*current_mode_id).clone()}
                    on_select_mode={Callback::from(move |id: ModeId| {
                        let window = web_sys::window().unwrap();
                        let _ = window.location().set_hash(&format!("#{}", id.as_ref()));
                        current_mode_id.set(id);
                    })}
                />
            }

            if let Some(error) = &reducer.error {
                if is_joined && !has_match_result {
                    <ErrorToast error={error.clone()} />
                }
            }
        </div>
    }
}
