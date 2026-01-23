//! Peer status panel component.

use crate::p2p::state::P2PStateContext;
use yew::prelude::*;

/// Peer status panel showing connected peers and their RTT.
#[function_component(PeerStatusPanel)]
pub fn peer_status_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let my_peer_short = state
        .my_peer_id
        .map(|id| id.0.to_string()[..8].to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let connected_peer_count = state.peers.values().filter(|p| p.connected).count();

    html! {
        <div class="peer-status-panel">
            <h3>{ "Players" }</h3>

            // Self
            <div class="player-item self">
                <div
                    class="player-color"
                    style={format!(
                        "background: rgb({}, {}, {});",
                        state.my_color.r, state.my_color.g, state.my_color.b
                    )}
                />
                <span class="player-name">
                    { if state.my_name.is_empty() {
                        format!("You ({})", my_peer_short)
                    } else {
                        format!("{} (You)", state.my_name)
                    }}
                </span>
                { if state.is_host {
                    html! { <span class="badge host">{ "HOST" }</span> }
                } else {
                    html! {}
                }}
                { if state.my_ready {
                    html! { <span class="ready-status ready">{ "Ready" }</span> }
                } else {
                    html! { <span class="ready-status not-ready">{ "Not Ready" }</span> }
                }}
            </div>

            // Peers
            { for state.peers.iter().map(|(peer_id, info)| {
                let peer_short = peer_id.0.to_string()[..8].to_string();
                let is_peer_host = state.all_peer_ids().first() == Some(peer_id);

                html! {
                    <div class={classes!("player-item", (!info.connected).then_some("disconnected"))}>
                        <div
                            class="player-color"
                            style={format!(
                                "background: rgb({}, {}, {});{}",
                                info.color.r, info.color.g, info.color.b,
                                if info.connected { "" } else { " opacity: 0.5;" }
                            )}
                        />
                        <span class="player-name">
                            { if info.name.is_empty() {
                                format!("Peer-{}", peer_short)
                            } else {
                                info.name.clone()
                            }}
                        </span>
                        { if !info.connected {
                            html! { <span class="badge disconnected">{ "OFFLINE" }</span> }
                        } else if is_peer_host {
                            html! { <span class="badge host">{ "HOST" }</span> }
                        } else {
                            html! {}
                        }}
                        { if info.connected {
                            if info.ready {
                                html! { <span class="ready-status ready">{ "Ready" }</span> }
                            } else {
                                html! { <span class="ready-status not-ready">{ "Not Ready" }</span> }
                            }
                        } else {
                            html! {}
                        }}
                        { if info.connected {
                            if let Some(rtt) = info.rtt_ms {
                                let rtt_class = if rtt < 50 {
                                    "good"
                                } else if rtt < 100 {
                                    "medium"
                                } else {
                                    "bad"
                                };
                                html! {
                                    <span class={classes!("rtt", rtt_class)}>
                                        { format!("{}ms", rtt) }
                                    </span>
                                }
                            } else {
                                html! { <span class="rtt">{ "--ms" }</span> }
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            })}

            { if connected_peer_count == 0 {
                html! {
                    <div class="waiting-message">
                        { "Waiting for other players to join..." }
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
