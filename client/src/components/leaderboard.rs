use yew::prelude::*;
use common::models::Player;
use uuid::Uuid;

#[derive(Properties, PartialEq)]
pub struct LeaderboardProps {
    pub players: Vec<Player>,
    pub self_id: Uuid,
}

#[function_component(Leaderboard)]
pub fn leaderboard(props: &LeaderboardProps) -> Html {
    let mut players = props.players.clone();
    players.sort_by(|a, b| b.score.cmp(&a.score));
    
    html! {
        <div style="position: absolute; top: 20px; right: 20px; background: transparent; padding: 15px; width: 200px; z-index: 60; pointer-events: none;">
            <div style="display: flex; flex-direction: column; gap: 5px;">
                {
                    players.into_iter().take(10).map(|p| {
                        let is_self = props.self_id == p.id;
                        let display_name = if p.name.trim().is_empty() { "An Unnamed Player" } else { &p.name };
                        html! {
                            <div style={format!("display: flex; justify-content: space-between; font-size: 0.9em; {}", if is_self { "font-weight: bold; color: #2563eb;" } else { "" })}>
                                <span style="max-width: 130px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{display_name}</span>
                                <span>{p.score}</span>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
