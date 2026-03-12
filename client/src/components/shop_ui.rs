use crate::reducer::MsgSender;
use common::*;
use glam::IVec2;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ShopUIProps {
    pub player_score: u64,
    pub player_pieces_count: usize,
    pub piece_on_shop_type: Option<PieceType>,
    pub shop_pos: IVec2,
    pub tx: MsgSender,
}

#[function_component(ShopUI)]
pub fn shop_ui(props: &ShopUIProps) -> Html {
    let on_buy = {
        let tx = props.tx.clone();
        let shop_pos = props.shop_pos;
        Callback::from(move |pt: PieceType| {
            let _ = tx.0.send(ClientMessage::BuyPiece {
                shop_pos,
                piece_type: pt,
            });
        })
    };

    let current_piece_type = props.piece_on_shop_type.unwrap_or(PieceType::Pawn);
    let current_value = get_piece_value(current_piece_type);
    let is_king_on_shop = current_piece_type == PieceType::King;

    html! {
        <div style="position: absolute; bottom: 40px; left: 50%; transform: translateX(-50%); background: rgba(255, 255, 255, 0.9); padding: 15px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.2); display: flex; flex-direction: column; align-items: center; gap: 10px; z-index: 50;">
            <span style="font-weight: bold; color: #1e3a8a;">{"RECRUITMENT & UPGRADES"}</span>
            <div style="display: flex; gap: 10px;">
                {
                    [PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen].into_iter().map(|pt| {
                        let cost = get_upgrade_cost(pt, props.player_pieces_count);
                        let can_afford = props.player_score >= cost;

                        let should_show = if pt == PieceType::Pawn {
                            true
                        } else if is_king_on_shop {
                            false
                        } else {
                            get_piece_value(pt) > current_value
                        };

                        if should_show {
                            let label = match pt {
                                PieceType::Pawn => "Recruit Pawn",
                                PieceType::Knight => "Knight",
                                PieceType::Bishop => "Bishop",
                                PieceType::Rook => "Rook",
                                PieceType::Queen => "Queen",
                                _ => "Unknown",
                            };
                            html! {
                                <button
                                    onclick={on_buy.reform(move |_| pt)}
                                    disabled={!can_afford}
                                    style={format!(
                                        "padding: 8px 15px; cursor: {}; border-radius: 6px; border: 1px solid #ddd; background: {}; color: {};",
                                        if can_afford { "pointer" } else { "not-allowed" },
                                        if can_afford { "white" } else { "#f1f5f9" },
                                        if can_afford { "black" } else { "#94a3b8" }
                                    )}
                                >
                                    {format!("{} ({})", label, cost)}
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
