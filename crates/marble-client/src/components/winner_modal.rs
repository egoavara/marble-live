//! WinnerModal component for displaying game results.

use crate::p2p::state::{P2PPhase, P2PStateContext};
use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// WinnerModal component showing game results when the game finishes.
/// The room is not reusable - players must leave and create a new room for another game.
#[function_component(WinnerModal)]
pub fn winner_modal() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");
    let navigator = use_navigator().expect("Navigator not found");

    // Only show when game is finished
    if !matches!(state.phase, P2PPhase::Finished) {
        return html! {};
    }

    let game_state = &state.game_state;
    let rankings = game_state.get_rankings();
    let winner_id = game_state.winner();

    // Get winner info
    let winner_info = winner_id.and_then(|id| {
        game_state.players.iter().find(|p| p.id == id).map(|p| {
            // Find hash_code for winner
            let is_self = state
                .my_peer_id
                .and_then(|peer_id| state.peer_player_map.get(&peer_id).copied())
                == Some(id);

            let hash_code = if is_self {
                state.my_hash_code.clone()
            } else {
                state
                    .peer_player_map
                    .iter()
                    .find(|&(_, &pid)| pid == id)
                    .and_then(|(peer_id, _)| state.peers.get(peer_id))
                    .map(|peer| peer.hash_code.clone())
                    .unwrap_or_default()
            };

            (p.name.clone(), hash_code, p.color, is_self)
        })
    });

    // Build rankings list with player info
    let rankings_list: Vec<_> = rankings
        .iter()
        .enumerate()
        .filter_map(|(rank, &player_id)| {
            game_state.players.iter().find(|p| p.id == player_id).map(|p| {
                let is_self = state
                    .my_peer_id
                    .and_then(|peer_id| state.peer_player_map.get(&peer_id).copied())
                    == Some(player_id);

                let hash_code = if is_self {
                    state.my_hash_code.clone()
                } else {
                    state
                        .peer_player_map
                        .iter()
                        .find(|&(_, &pid)| pid == player_id)
                        .and_then(|(peer_id, _)| state.peers.get(peer_id))
                        .map(|peer| peer.hash_code.clone())
                        .unwrap_or_default()
                };

                (rank + 1, p.name.clone(), hash_code, p.color, is_self, player_id == winner_id.unwrap_or(u32::MAX))
            })
        })
        .collect();

    // Leave Game handler - go back to home to create a new room
    let on_leave = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Home);
        })
    };

    html! {
        <div class="winner-modal-overlay">
            <div class="winner-modal">
                <div class="winner-modal-header">
                    { "GAME OVER" }
                </div>

                <div class="winner-modal-content">
                    // Winner announcement
                    { if let Some((name, hash_code, color, is_winner_self)) = winner_info {
                        let display_name = if hash_code.is_empty() {
                            name
                        } else {
                            format!("{}#{}", name, hash_code)
                        };
                        html! {
                            <div class="winner-announcement">
                                <span
                                    class="winner-color"
                                    style={format!(
                                        "background: rgb({}, {}, {});",
                                        color.r, color.g, color.b
                                    )}
                                />
                                <span class="winner-name">
                                    { display_name }
                                    { if is_winner_self {
                                        html! { <span class="tag you">{ "YOU" }</span> }
                                    } else {
                                        html! {}
                                    }}
                                </span>
                                <span class="winner-text">{ " Wins!" }</span>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="winner-announcement">
                                { "No Winner" }
                            </div>
                        }
                    }}

                    // Rankings
                    <div class="rankings-section">
                        <div class="rankings-header">
                            { "Final Rankings" }
                        </div>
                        <div class="rankings-list">
                            { for rankings_list.iter().map(|(rank, name, hash_code, color, is_self, is_winner)| {
                                let display_name = if hash_code.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{}#{}", name, hash_code)
                                };

                                let rank_suffix = match rank {
                                    1 => "1st",
                                    2 => "2nd",
                                    3 => "3rd",
                                    _ => "th",
                                };
                                let rank_str = if *rank > 3 {
                                    format!("{}{}", rank, rank_suffix)
                                } else {
                                    rank_suffix.to_string()
                                };

                                html! {
                                    <div class={classes!(
                                        "rankings-item",
                                        is_winner.then_some("winner")
                                    )}>
                                        <span class="rankings-rank">{ rank_str }</span>
                                        <span
                                            class="rankings-color"
                                            style={format!(
                                                "background: rgb({}, {}, {});",
                                                color.r, color.g, color.b
                                            )}
                                        />
                                        <span class="rankings-name">
                                            { display_name }
                                            { if *is_self {
                                                html! { <span class="tag you">{ "YOU" }</span> }
                                            } else {
                                                html! {}
                                            }}
                                        </span>
                                    </div>
                                }
                            })}
                        </div>
                    </div>
                </div>

                <div class="winner-modal-actions">
                    <button
                        class="btn leave-btn"
                        onclick={on_leave}
                    >
                        { "Leave Game" }
                    </button>
                </div>
            </div>
        </div>
    }
}
