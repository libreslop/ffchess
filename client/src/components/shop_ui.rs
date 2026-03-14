use crate::reducer::MsgSender;
use common::models::{Piece, PieceConfig, ShopConfig};
use common::protocol::ClientMessage;
use glam::IVec2;
use std::collections::HashMap;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ShopUIProps {
    pub player_score: u64,
    pub player_pieces_count: usize,
    pub piece_on_shop: Option<Piece>,
    pub shop_config: ShopConfig,
    pub piece_configs: HashMap<String, PieceConfig>,
    pub tx: MsgSender,
    pub shop_pos: IVec2,
}

#[function_component(ShopUI)]
pub fn shop_ui(props: &ShopUIProps) -> Html {
    let group = if let Some(ref p) = props.piece_on_shop {
        props
            .shop_config
            .groups
            .iter()
            .find(|g| g.applies_to.contains(&p.piece_type))
            .unwrap_or(&props.shop_config.default_group)
    } else {
        &props.shop_config.default_group
    };

    let mut vars = HashMap::new();
    vars.insert(
        "player_piece_count".to_string(),
        props.player_pieces_count as f64,
    );
    for p_id in props.piece_configs.keys() {
        // We don't have individual counts here easily, but we can assume 0 or just not use them in simple expressions
        vars.insert(format!("{}_count", p_id), 0.0);
    }

    html! {
        <div style="position: absolute; bottom: 10px; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 8px; z-index: 50; width: 95%; max-width: 800px; pointer-events: none;">
            <span style="font-weight: 800; color: #1e293b; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.15em; pointer-events: auto; background: #ffffff; padding: 2px 8px; border: 2px solid #1e293b;">
                { &props.shop_config.display_name }
            </span>
            <div style="display: flex; gap: 4px; flex-wrap: wrap; justify-content: center; pointer-events: auto;">
                {
                    group.items.iter().enumerate().map(|(idx, item)| {
                        let price = common::logic::evaluate_expression(&item.price_expr, &vars) as u64;
                        let can_afford = props.player_score >= price;
                        let shop_pos = props.shop_pos;
                        let tx = props.tx.clone();
                        let on_buy = Callback::from(move |_| {
                            let _ = tx.0.send(ClientMessage::BuyPiece {
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
                                <span style="font-size: 0.65rem; font-family: monospace; font-weight: 600;">
                                    { price }
                                </span>
                            </button>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
