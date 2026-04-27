//! Shop overlay component for purchases.

use crate::reducer::MsgSender;
use common::models::{Piece, PieceConfig, ShopConfig};
use common::protocol::ClientMessage;
use common::types::{BoardCoord, PieceTypeId, Score};
use std::collections::HashMap;
use yew::prelude::*;

/// Properties for the shop overlay UI.
#[derive(Properties, PartialEq)]
pub struct ShopUIProps {
    pub player_score: Score,
    pub player_pieces_count: usize,
    pub piece_on_shop: Option<Piece>,
    pub shop_config: ShopConfig,
    pub piece_configs: HashMap<PieceTypeId, PieceConfig>,
    pub tx: MsgSender,
    pub shop_pos: BoardCoord,
}

#[function_component(ShopUI)]
/// Renders shop items and buy actions for the player.
///
/// `props` provides shop context, piece configs, and send channel. Returns rendered HTML.
pub fn shop_ui(props: &ShopUIProps) -> Html {
    let Some(group) =
        common::logic::select_shop_group(&props.shop_config, props.piece_on_shop.as_ref())
    else {
        return Html::default();
    };
    if group.items.is_empty() {
        return Html::default();
    }

    let vars = common::logic::build_price_vars(
        props.player_pieces_count,
        props.piece_configs.keys().map(|p_id| (p_id, 0)),
    );

    html! {
        <div data-shop-ui="true" style="position: absolute; bottom: 10px; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 8px; z-index: 50; width: 95%; max-width: 800px; pointer-events: none;">
            <span style="font-weight: 800; color: #1e293b; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.15em; pointer-events: auto; background: #ffffff; padding: 2px 8px; border: 2px solid #1e293b;">
                { &props.shop_config.display_name }
            </span>
            <div style="display: flex; gap: 4px; flex-wrap: wrap; justify-content: center; pointer-events: auto;">
                {
                    group.items.iter().enumerate().map(|(idx, item)| {
                        let price = item
                            .price_expr
                            .as_ref()
                            .map(|expr| Score::from(common::logic::evaluate_expression(expr, &vars) as u64))
                            .unwrap_or_else(Score::zero);
                        let can_afford = props.player_score >= price;
                        let shop_pos = props.shop_pos;
                        let tx = props.tx.clone();
                        let on_buy = Callback::from(move |_| {
                            let _ = tx.0.try_send(ClientMessage::BuyPiece {
                                shop_pos,
                                item_index: idx,
                            });
                        });

                        html! {
                            <button
                                onclick={on_buy}
                                disabled={!can_afford}
                                style={format!(
                                    "display: flex; flex-direction: column; align-items: center; justify-content: center; width: 70px; height: 70px; padding: 2px; cursor: {}; border-radius: 0; border: 2px solid {}; background: {}; color: {}; transition: all 0.1s; aspect-ratio: 1/1;",
                                    if can_afford { "pointer" } else { "not-allowed" },
                                    if can_afford { "#1e293b" } else { "#94a3b8" },
                                    if can_afford { "#ffffff" } else { "#f1f5f9" },
                                    if can_afford { "#0f172a" } else { "#94a3b8" }
                                )}
                            >
                                <span style="font-size: 0.7rem; font-weight: 900; line-height: 1.1; text-align: center; margin-bottom: 2px; text-transform: uppercase;">
                                    { &item.display_name }
                                </span>
                                if item.price_expr.is_some() {
                                    <span style="font-size: 0.65rem; font-family: monospace; font-weight: 600;">
                                        { price.to_string() }
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
