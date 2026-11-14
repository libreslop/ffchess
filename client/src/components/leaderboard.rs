//! Leaderboard overlay component, with the board's spectators listed at the middle right.

use common::models::Player;
use common::types::PlayerId;
use yew::prelude::*;

/// Most players and spectators listed by name.
const MAX_LISTED: usize = 10;

/// Properties for the leaderboard overlay.
#[derive(Properties, PartialEq)]
pub struct LeaderboardProps {
    pub players: Vec<Player>,
    pub self_id: PlayerId,
    /// Whether the mode shows player scores; the spectator list shows either way.
    pub show_scores: bool,
    pub spectators: Vec<String>,
}

#[function_component(Leaderboard)]
/// Renders the leaderboard with the top player scores at the top right, and the spectators
/// at the middle right.
///
/// `props` provides the player list and current player id. Returns rendered HTML.
pub fn leaderboard(props: &LeaderboardProps) -> Html {
    let mut players = props.players.clone();
    players.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.join_time.cmp(&b.join_time))
    });

    html! {
        <>
            if props.show_scores {
                <div style="position: absolute; top: 10px; right: 10px; background: transparent; padding: 5px; width: 200px; z-index: 60; pointer-events: none; display: flex; flex-direction: column; gap: 5px;">
                    {
                        players.into_iter().take(MAX_LISTED).map(|p| {
                            let is_self = props.self_id == p.id;
                            let display_name = if p.name.trim().is_empty() { "An Unnamed Player" } else { &p.name };
                            html! {
                                <div style={format!("display: flex; justify-content: space-between; gap: 16px; font-size: 0.9em; font-weight: bold; color: color-mix(in srgb, {} 85%, white);", p.color.as_ref())}>
                                    <span style={format!("overflow: hidden; text-overflow: ellipsis; white-space: nowrap; {}", if is_self { "text-decoration: underline;" } else { "" })}>{display_name}</span>
                                    <span>{p.score.to_string()}</span>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
            }
            <SpectatorList names={props.spectators.clone()} />
        </>
    }
}

/// Properties for the spectator list.
#[derive(Properties, PartialEq)]
struct SpectatorListProps {
    names: Vec<String>,
}

#[function_component(SpectatorList)]
/// Renders the names of the board's spectators, or nothing when nobody spectates.
fn spectator_list(props: &SpectatorListProps) -> Html {
    if props.names.is_empty() {
        return Html::default();
    }
    let hidden = props.names.len().saturating_sub(MAX_LISTED);
    html! {
        <div data-testid="spectator-list" style="position: absolute; top: 50%; right: 10px; transform: translateY(-50%); padding: 5px; width: 200px; z-index: 60; pointer-events: none; display: flex; flex-direction: column; gap: 3px; font-size: 0.8em; color: #818793; text-align: right;">
            <span style="font-weight: bold; text-transform: uppercase; letter-spacing: 1px; font-size: 0.85em;">
                {"Spectators"}
            </span>
            { for props.names.iter().take(MAX_LISTED).map(|name| html! {
                <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{name}</span>
            })}
            if hidden > 0 {
                <span style="font-style: italic;">{format!("+{hidden} more")}</span>
            }
        </div>
    }
}
