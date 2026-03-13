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
        props.shop_config.groups.iter().find(|g| g.applies_to.contains(&p.piece_type)).unwrap_or(&props.shop_config.default_group)
    } else {
        &props.shop_config.default_group
    };

    let mut vars = HashMap::new();
    vars.insert("player_piece_count".to_string(), props.player_pieces_count as f64);
    for p_id in props.piece_configs.keys() {
        // We don't have individual counts here easily, but we can assume 0 or just not use them in simple expressions
        vars.insert(format!("{}_count", p_id), 0.0); 
    }

    html! {
        <div style="position: absolute; bottom: 40px; left: 50%; transform: translateX(-50%); background: rgba(255, 255, 255, 0.9); padding: 15px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.2); display: flex; flex-direction: column; align-items: center; gap: 10px; z-index: 50; width: 90%; max-width: 600px;">
            <span style="font-weight: bold; color: #1e3a8a; font-size: 0.9em; text-align: center;">{ &props.shop_config.display_name }</span>
            <div style="display: flex; gap: 8px; flex-wrap: wrap; justify-content: center;">
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
                                    "padding: 6px 12px; cursor: {}; border-radius: 6px; border: 1px solid #ddd; background: {}; color: {}; font-size: 0.85em; white-space: nowrap;",
                                    if can_afford { "pointer" } else { "not-allowed" },
                                    if can_afford { "white" } else { "#f1f5f9" },
                                    if can_afford { "black" } else { "#94a3b8" }
                                )}
                            >
                                {format!("{} ({})", item.display_name, price)}
                            </button>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
