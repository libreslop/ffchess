//! Shop overlay component for purchases.

use common::models::{Piece, ShopConfig};
use common::types::{ExprVars, Score};
use gloo_events::EventListener;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlTextAreaElement, KeyboardEvent};
use yew::prelude::*;

/// Properties for the shop overlay UI.
#[derive(Properties, PartialEq)]
pub struct ShopUIProps {
    pub player_score: Score,
    /// Pricing variables of the buying player.
    pub price_vars: ExprVars,
    pub piece_on_shop: Piece,
    pub shop_config: ShopConfig,
    pub on_buy: Callback<usize>,
}

/// One purchasable offer as displayed.
#[derive(Clone, PartialEq)]
struct OfferView {
    name: String,
    price: Score,
    shows_price: bool,
    affordable: bool,
}

/// Renders shop items and buy actions for the player.
#[function_component(ShopUI)]
pub fn shop_ui(props: &ShopUIProps) -> Html {
    let offers: Vec<OfferView> = props
        .shop_config
        .manual_items_for(&props.piece_on_shop)
        .unwrap_or_default()
        .iter()
        .map(|item| {
            let price = item.price(&props.price_vars);
            OfferView {
                name: item.display_name.clone(),
                price,
                shows_price: item.price_expr.is_some(),
                affordable: props.player_score >= price,
            }
        })
        .collect();
    let hotkey_can_buy: Vec<bool> = offers.iter().map(|offer| offer.affordable).collect();

    {
        let on_buy = props.on_buy.clone();
        use_effect_with((hotkey_can_buy, on_buy), move |(hotkey_can_buy, on_buy)| {
            let hotkey_can_buy = hotkey_can_buy.clone();
            let on_buy = on_buy.clone();
            let window = web_sys::window().expect("window available");
            let listener = EventListener::new(&window, "keydown", move |event| {
                let Some(key_event) = event.dyn_ref::<KeyboardEvent>() else {
                    return;
                };
                if key_event.repeat() {
                    return;
                }
                if let Some(target) = key_event.target()
                    && let Some(input) = target.dyn_ref::<HtmlInputElement>()
                    && !input.read_only()
                {
                    return;
                }
                if let Some(target) = key_event.target()
                    && target.dyn_ref::<HtmlTextAreaElement>().is_some()
                {
                    return;
                }
                let key = key_event.key();
                let Some(digit) = key
                    .chars()
                    .next()
                    .and_then(|ch| ch.to_digit(10))
                    .and_then(|d| usize::try_from(d).ok())
                else {
                    return;
                };
                if digit == 0 {
                    return;
                }
                let index = digit - 1;
                if hotkey_can_buy.get(index).copied().unwrap_or(false) {
                    key_event.prevent_default();
                    on_buy.emit(index);
                }
            });
            move || drop(listener)
        });
    }

    if offers.is_empty() {
        return Html::default();
    }

    html! {
        <div data-shop-ui="true" data-ui-exempt="true" style="position: absolute; bottom: 10px; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 8px; z-index: 50; width: 95%; max-width: 800px; pointer-events: none;">
            <span style="font-weight: 800; color: #1e293b; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.15em; pointer-events: auto; background: #ffffff; padding: 2px 8px; border: 2px solid #1e293b;">
                { &props.shop_config.display_name }
            </span>
            <div style="display: flex; gap: 4px; flex-wrap: wrap; justify-content: center; pointer-events: auto;">
                {
                    offers.into_iter().enumerate().map(|(idx, offer)| {
                        let can_afford = offer.affordable;
                        let on_buy_cb = props.on_buy.clone();
                        let on_buy = Callback::from(move |_| {
                            on_buy_cb.emit(idx);
                        });

                        html! {
                            <button
                                onclick={on_buy}
                                disabled={!can_afford}
                                style={format!(
                                    "position: relative; display: flex; flex-direction: column; align-items: center; justify-content: center; width: 70px; height: 70px; padding: 2px; cursor: {}; border-radius: 0; border: 2px solid {}; background: {}; color: {}; transition: all 0.1s; aspect-ratio: 1/1;",
                                    if can_afford { "pointer" } else { "not-allowed" },
                                    if can_afford { "#1e293b" } else { "#94a3b8" },
                                    if can_afford { "#ffffff" } else { "#f1f5f9" },
                                    if can_afford { "#0f172a" } else { "#94a3b8" }
                                )}
                            >
                                <span style="position: absolute; top: 2px; left: 4px; font-size: 0.65rem; font-family: monospace; font-weight: 800; color: #475569;">
                                    { (idx + 1).to_string() }
                                </span>
                                <span style="font-size: 0.7rem; font-weight: 900; line-height: 1.1; text-align: center; margin-bottom: 2px; text-transform: uppercase;">
                                    { offer.name }
                                </span>
                                if offer.shows_price {
                                    <span style="font-size: 0.65rem; font-family: monospace; font-weight: 600;">
                                        { offer.price.to_string() }
                                    </span>
                                }
                            </button>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
