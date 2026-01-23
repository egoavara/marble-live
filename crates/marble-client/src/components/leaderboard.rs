//! Leaderboard overlay component for game.

use crate::p2p::state::P2PStateContext;
use yew::prelude::*;

/// Leaderboard overlay showing players in a game-style format.
#[function_component(Leaderboard)]
pub fn leaderboard() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let my_peer_short = state
        .my_peer_id
        .map(|id| id.0.to_string()[..8].to_string())
        .unwrap_or_else(|| "N/A".to_string());

    // Build player list: self first, then peers
    // (display_name, color, is_self, is_host, ready, rtt)
    let mut players: Vec<(String, marble_core::Color, bool, bool, bool, Option<u32>)> = Vec::new();

    // Add self with hash code
    let my_name = if state.my_name.is_empty() {
        format!("You ({})", my_peer_short)
    } else if state.my_hash_code.is_empty() {
        state.my_name.clone()
    } else {
        format!("{}#{}", state.my_name, state.my_hash_code)
    };
    players.push((my_name, state.my_color, true, state.is_host, state.my_ready, None));

    // Add peers with hash codes
    for (peer_id, info) in state.peers.iter() {
        if !info.connected {
            continue;
        }
        let peer_short = peer_id.0.to_string()[..8].to_string();
        let name = if info.name.is_empty() {
            format!("Peer-{}", peer_short)
        } else if info.hash_code.is_empty() {
            info.name.clone()
        } else {
            format!("{}#{}", info.name, info.hash_code)
        };
        let is_peer_host = state.all_peer_ids().first() == Some(peer_id);
        players.push((name, info.color, false, is_peer_host, info.ready, info.rtt_ms));
    }

    html! {
        <div class="leaderboard">
            <div class="leaderboard-header">
                { "Players" }
            </div>
            <div class="leaderboard-list">
                { for players.iter().enumerate().map(|(idx, (name, color, is_self, is_host, ready, rtt))| {
                    html! {
                        <div class={classes!("leaderboard-item", is_self.then_some("self"))}>
                            <span class="leaderboard-rank">{ idx + 1 }</span>
                            <span
                                class="leaderboard-color"
                                style={format!(
                                    "background: rgb({}, {}, {});",
                                    color.r, color.g, color.b
                                )}
                            />
                            <span class="leaderboard-name">
                                { name }
                                { if *is_self {
                                    html! { <span class="tag you">{ "YOU" }</span> }
                                } else {
                                    html! {}
                                }}
                                { if *is_host {
                                    html! { <span class="tag host">{ "HOST" }</span> }
                                } else {
                                    html! {}
                                }}
                            </span>
                            <span class="leaderboard-status">
                                { if *ready {
                                    html! { <span class="ready-indicator ready" /> }
                                } else {
                                    html! { <span class="ready-indicator not-ready" /> }
                                }}
                                { if let Some(rtt_val) = rtt {
                                    let rtt_class = if *rtt_val < 50 {
                                        "good"
                                    } else if *rtt_val < 100 {
                                        "medium"
                                    } else {
                                        "bad"
                                    };
                                    html! {
                                        <span class={classes!("rtt", rtt_class)}>
                                            { format!("{}ms", rtt_val) }
                                        </span>
                                    }
                                } else {
                                    html! {}
                                }}
                            </span>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
