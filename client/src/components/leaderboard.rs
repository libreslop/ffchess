use common::models::Player;
use common::types::PlayerId;
use yew::prelude::*;

/// Properties for the leaderboard overlay.
#[derive(Properties, PartialEq)]
pub struct LeaderboardProps {
    pub players: Vec<Player>,
    pub self_id: PlayerId,
}

#[function_component(Leaderboard)]
pub fn leaderboard(props: &LeaderboardProps) -> Html {
    let mut players = props.players.clone();
    players.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.join_time.cmp(&b.join_time))
    });

    html! {
        <div style="position: absolute; top: 10px; right: 10px; background: transparent; padding: 5px; width: 200px; z-index: 60; pointer-events: none;">
            <div style="display: flex; flex-direction: column; gap: 5px;">
                {
                    players.into_iter().take(10).map(|p| {
                        let is_self = props.self_id == p.id;
                        let display_name = if p.name.trim().is_empty() { "An Unnamed Player" } else { &p.name };
                        html! {
                            <div style={format!("display: flex; justify-content: space-between; gap: 16px; font-size: 0.9em; font-weight: bold; color: {};", p.color.as_ref())}>
                                <span style={format!("overflow: hidden; text-overflow: ellipsis; white-space: nowrap; {}", if is_self { "text-decoration: underline;" } else { "" })}>{display_name}</span>
                                <span>{p.score.to_string()}</span>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        </div>
    }
}
