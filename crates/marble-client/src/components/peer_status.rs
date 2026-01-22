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
        <div class="peer-status-panel" style="background: #f5f5f5; padding: 15px; border-radius: 8px; border: 1px solid #ddd;">
            <h3 style="margin: 0 0 10px 0; color: #333;">{"Players"}</h3>

            // Self
            <div style="display: flex; align-items: center; gap: 8px; padding: 8px; background: #e3f2fd; border-radius: 4px; margin-bottom: 8px;">
                <div style={format!(
                    "width: 12px; height: 12px; border-radius: 50%; background: rgb({}, {}, {});",
                    state.my_color.r, state.my_color.g, state.my_color.b
                )}></div>
                <span style="flex: 1; color: #333; font-weight: bold;">
                    {if state.my_name.is_empty() {
                        format!("You ({})", my_peer_short)
                    } else {
                        format!("{} (You)", state.my_name)
                    }}
                </span>
                {if state.is_host {
                    html! { <span style="background: #ff9800; color: white; padding: 2px 6px; border-radius: 4px; font-size: 10px;">{"HOST"}</span> }
                } else {
                    html! {}
                }}
                {if state.my_ready {
                    html! { <span style="color: green;">{"Ready"}</span> }
                } else {
                    html! { <span style="color: orange;">{"Not Ready"}</span> }
                }}
            </div>

            // Peers
            {for state.peers.iter().map(|(peer_id, info)| {
                let peer_short = peer_id.0.to_string()[..8].to_string();
                let is_peer_host = state.all_peer_ids().first() == Some(peer_id);

                // Style based on connection status
                let container_style = if info.connected {
                    "display: flex; align-items: center; gap: 8px; padding: 8px; background: white; border-radius: 4px; margin-bottom: 4px; border: 1px solid #eee;"
                } else {
                    "display: flex; align-items: center; gap: 8px; padding: 8px; background: #f0f0f0; border-radius: 4px; margin-bottom: 4px; border: 1px solid #ddd; opacity: 0.7;"
                };

                let name_style = if info.connected {
                    "flex: 1; color: #333;"
                } else {
                    "flex: 1; color: #888;"
                };

                html! {
                    <div style={container_style}>
                        <div style={format!(
                            "width: 12px; height: 12px; border-radius: 50%; background: rgb({}, {}, {});{}",
                            info.color.r, info.color.g, info.color.b,
                            if info.connected { "" } else { " opacity: 0.5;" }
                        )}></div>
                        <span style={name_style}>
                            {if info.name.is_empty() {
                                format!("Peer-{}", peer_short)
                            } else {
                                info.name.clone()
                            }}
                        </span>
                        {if !info.connected {
                            html! { <span style="background: #9e9e9e; color: white; padding: 2px 6px; border-radius: 4px; font-size: 10px;">{"DISCONNECTED"}</span> }
                        } else if is_peer_host {
                            html! { <span style="background: #ff9800; color: white; padding: 2px 6px; border-radius: 4px; font-size: 10px;">{"HOST"}</span> }
                        } else {
                            html! {}
                        }}
                        {if info.connected {
                            if info.ready {
                                html! { <span style="color: green;">{"Ready"}</span> }
                            } else {
                                html! { <span style="color: orange;">{"Not Ready"}</span> }
                            }
                        } else {
                            html! {}
                        }}
                        {if info.connected {
                            if let Some(rtt) = info.rtt_ms {
                                let color = if rtt < 50 {
                                    "#4caf50"
                                } else if rtt < 100 {
                                    "#ff9800"
                                } else {
                                    "#f44336"
                                };
                                html! {
                                    <span style={format!("color: {}; font-size: 12px;", color)}>
                                        {format!("{}ms", rtt)}
                                    </span>
                                }
                            } else {
                                html! { <span style="color: #999; font-size: 12px;">{"--ms"}</span> }
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            })}

            {if connected_peer_count == 0 {
                html! {
                    <div style="color: #666; font-style: italic; padding: 8px;">
                        {"Waiting for other players to join..."}
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
