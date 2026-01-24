//! PeerList component for displaying connected peers.

use yew::prelude::*;

use crate::services::p2p::{P2pConnectionState, P2pPeerInfo};

/// Props for the PeerList component.
#[derive(Properties, PartialEq)]
pub struct PeerListProps {
    /// List of connected peers
    pub peers: Vec<P2pPeerInfo>,
    /// Current player's ID
    pub my_player_id: String,
    /// Current connection state
    pub connection_state: P2pConnectionState,
}

/// PeerList component - displays connected peers with connection status.
///
/// Positioned as an overlay on the left side, vertically centered.
#[function_component(PeerList)]
pub fn peer_list(props: &PeerListProps) -> Html {
    let connection_indicator = match &props.connection_state {
        P2pConnectionState::Connected => html! {
            <span class="connection-indicator connected" title="Connected">{"●"}</span>
        },
        P2pConnectionState::Connecting => html! {
            <span class="connection-indicator connecting" title="Connecting...">{"○"}</span>
        },
        P2pConnectionState::Disconnected => html! {
            <span class="connection-indicator disconnected" title="Disconnected">{"○"}</span>
        },
        P2pConnectionState::Error(msg) => html! {
            <span class="connection-indicator error" title={msg.clone()}>{"✕"}</span>
        },
    };

    let peer_count = props.peers.iter().filter(|p| p.connected).count();

    html! {
        <div class="peer-list">
            <div class="peer-list-header">
                {connection_indicator}
                <span class="peer-list-title">{"Players"}</span>
                <span class="peer-list-count">{format!("({})", peer_count + 1)}</span>
            </div>
            <div class="peer-list-debug">
                {format!("{:?}", props.connection_state)}
            </div>
            <div class="peer-list-items">
                // Self entry (always first)
                <div class="peer-list-item self">
                    <span class="peer-rank">{"1"}</span>
                    <span class="peer-status connected">{"●"}</span>
                    <span class="peer-name">{&props.my_player_id}</span>
                    <span class="peer-tag you">{"YOU"}</span>
                </div>
                // Other peers
                { for props.peers.iter().enumerate().map(|(idx, peer)| {
                    let status_class = if peer.connected { "connected" } else { "disconnected" };
                    let status_icon = if peer.connected { "●" } else { "○" };
                    let display_name = peer.player_id.clone()
                        .unwrap_or_else(|| format!("Peer-{}", &peer.peer_id.to_string()[..8]));

                    let rtt_display = peer.rtt_ms.map(|rtt| {
                        html! { <span class="peer-rtt">{format!("{}ms", rtt)}</span> }
                    });

                    html! {
                        <div class="peer-list-item" key={peer.peer_id.to_string()}>
                            <span class="peer-rank">{idx + 2}</span>
                            <span class={classes!("peer-status", status_class)}>{status_icon}</span>
                            <span class="peer-name">{display_name}</span>
                            { rtt_display }
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
