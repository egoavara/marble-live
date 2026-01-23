//! PlayerLegend component for displaying players during gameplay.

use crate::p2p::state::{P2PPhase, P2PStateContext};
use yew::prelude::*;

/// PlayerLegend component showing players and their elimination status during gameplay.
#[function_component(PlayerLegend)]
pub fn player_legend() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    // Only show during Countdown, Running, Finished phases
    let should_show = matches!(
        state.phase,
        P2PPhase::Countdown { .. } | P2PPhase::Running | P2PPhase::Finished
    );

    if !should_show {
        return html! {};
    }

    let game_state = &state.game_state;
    let eliminated_order = &game_state.eliminated_order;
    let total_players = game_state.players.len();

    // Build player data: (player_id, name, hash_code, color, is_self, is_eliminated, elimination_rank)
    let mut player_data: Vec<(u32, String, String, marble_core::Color, bool, bool, Option<usize>)> =
        Vec::new();

    // Get my player_id from peer_player_map
    let my_player_id = state
        .my_peer_id
        .and_then(|peer_id| state.peer_player_map.get(&peer_id).copied());

    for player in &game_state.players {
        let player_id = player.id;

        // Determine if this is self
        let is_self = my_player_id == Some(player_id);

        // Get hash_code: use my_hash_code if self, otherwise find from peers
        let hash_code = if is_self {
            state.my_hash_code.clone()
        } else {
            // Find peer with this player_id
            state
                .peer_player_map
                .iter()
                .find(|&(_, &pid)| pid == player_id)
                .and_then(|(peer_id, _)| state.peers.get(peer_id))
                .map(|peer| peer.hash_code.clone())
                .unwrap_or_default()
        };

        // Check if eliminated and get rank
        let elimination_position = eliminated_order.iter().position(|&id| id == player_id);
        let is_eliminated = elimination_position.is_some();
        // Elimination rank: first eliminated gets last place (total_players), last eliminated gets 2nd place
        let elimination_rank = elimination_position.map(|pos| total_players - pos);

        player_data.push((
            player_id,
            player.name.clone(),
            hash_code,
            player.color,
            is_self,
            is_eliminated,
            elimination_rank,
        ));
    }

    // Sort: non-eliminated first, then by player_id; eliminated sorted by elimination order (later = higher rank)
    player_data.sort_by(|a, b| {
        match (a.5, b.5) {
            // Both alive: sort by player_id
            (false, false) => a.0.cmp(&b.0),
            // Both eliminated: sort by rank (higher rank first, i.e., later elimination)
            (true, true) => b.6.cmp(&a.6),
            // Alive comes before eliminated
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
        }
    });

    html! {
        <div class="player-legend">
            <div class="player-legend-header">
                { "Players" }
            </div>
            <div class="player-legend-list">
                { for player_data.iter().map(|(_, name, hash_code, color, is_self, is_eliminated, rank)| {
                    let display_name = if hash_code.is_empty() {
                        name.clone()
                    } else {
                        format!("{}#{}", name, hash_code)
                    };

                    let rank_suffix: Option<String> = match rank {
                        Some(1) => Some("1st".to_string()),
                        Some(2) => Some("2nd".to_string()),
                        Some(3) => Some("3rd".to_string()),
                        Some(n) => Some(format!("{}th", n)),
                        None => None,
                    };

                    html! {
                        <div class={classes!(
                            "player-legend-item",
                            is_eliminated.then_some("eliminated")
                        )}>
                            <span
                                class="player-legend-color"
                                style={format!(
                                    "background: rgb({}, {}, {});",
                                    color.r, color.g, color.b
                                )}
                            />
                            <span class="player-legend-name">
                                { display_name }
                                { if *is_self {
                                    html! { <span class="tag you">{ "YOU" }</span> }
                                } else {
                                    html! {}
                                }}
                            </span>
                            { if let Some(rank_str) = rank_suffix {
                                html! {
                                    <span class="player-legend-rank">
                                        { format!("[{}]", rank_str) }
                                    </span>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
