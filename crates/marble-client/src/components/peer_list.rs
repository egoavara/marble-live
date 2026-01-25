//! PeerList component for displaying connected peers.

use yew::prelude::*;

use crate::services::p2p::{P2pConnectionState, P2pPeerInfo};

/// Arrival information for a player.
#[derive(Clone, PartialEq, Default)]
pub struct ArrivalInfo {
    /// P2P player_id (name)
    pub player_id: String,
    /// Gamerule-applied rank (1-based, None = not arrived)
    pub rank: Option<u32>,
    /// Actual arrival order (1-based)
    pub arrival_order: Option<u32>,
    /// Live rank based on y-position (1-based, for non-arrived players)
    pub live_rank: Option<u32>,
}

/// Props for the PeerList component.
#[derive(Properties, PartialEq)]
pub struct PeerListProps {
    /// List of connected peers
    pub peers: Vec<P2pPeerInfo>,
    /// Current player's ID
    pub my_player_id: String,
    /// Current connection state
    pub connection_state: P2pConnectionState,
    /// Arrival info for all players
    #[prop_or_default]
    pub arrival_info: Vec<ArrivalInfo>,
    /// Selected gamerule ("top_n" or "last_n")
    #[prop_or_default]
    pub gamerule: String,
}

/// Player info for rendering (combines peer info with arrival info).
#[derive(Clone)]
struct PlayerRenderInfo {
    player_id: String,
    is_self: bool,
    connected: bool,
    rtt_ms: Option<u32>,
    rank: Option<u32>,
    arrival_order: Option<u32>,
    live_rank: Option<u32>,
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

    // Build player list with arrival info
    let mut players: Vec<PlayerRenderInfo> = Vec::new();

    // Add self
    let self_arrival = props.arrival_info.iter().find(|a| a.player_id == props.my_player_id);
    players.push(PlayerRenderInfo {
        player_id: props.my_player_id.clone(),
        is_self: true,
        connected: true,
        rtt_ms: None,
        rank: self_arrival.and_then(|a| a.rank),
        arrival_order: self_arrival.and_then(|a| a.arrival_order),
        live_rank: self_arrival.and_then(|a| a.live_rank),
    });

    // Add peers
    for peer in &props.peers {
        let display_name = peer.player_id.clone()
            .unwrap_or_else(|| format!("Peer-{}", &peer.peer_id.to_string()[..8]));
        let peer_arrival = props.arrival_info.iter().find(|a| a.player_id == display_name);
        players.push(PlayerRenderInfo {
            player_id: display_name,
            is_self: false,
            connected: peer.connected,
            rtt_ms: peer.rtt_ms,
            rank: peer_arrival.and_then(|a| a.rank),
            arrival_order: peer_arrival.and_then(|a| a.arrival_order),
            live_rank: peer_arrival.and_then(|a| a.live_rank),
        });
    }

    // Sort: arrived players (by rank) first, then non-arrived players (by live_rank)
    players.sort_by(|a, b| {
        match (a.rank, b.rank) {
            (Some(rank_a), Some(rank_b)) => rank_a.cmp(&rank_b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                // Both not arrived: sort by live_rank
                match (a.live_rank, b.live_rank) {
                    (Some(lr_a), Some(lr_b)) => lr_a.cmp(&lr_b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        }
    });

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
                { for players.iter().map(|player| {
                    let status_class = if player.connected { "connected" } else { "disconnected" };
                    let status_icon = if player.connected { "●" } else { "○" };
                    let arrived = player.rank.is_some();
                    let item_class = if player.is_self {
                        if arrived { "peer-list-item self arrived" } else { "peer-list-item self" }
                    } else if arrived {
                        "peer-list-item arrived"
                    } else {
                        "peer-list-item"
                    };

                    let rank_display = match (player.rank, player.live_rank) {
                        (Some(r), _) => html! { <span class="peer-rank arrived">{r}</span> },
                        (None, Some(lr)) => html! { <span class="peer-rank live">{lr}</span> },
                        (None, None) => html! { <span class="peer-rank pending">{"-"}</span> },
                    };

                    let rtt_display = player.rtt_ms.map(|rtt| {
                        html! { <span class="peer-rtt">{format!("{}ms", rtt)}</span> }
                    });

                    let you_tag = if player.is_self {
                        html! { <span class="peer-tag you">{"YOU"}</span> }
                    } else {
                        html! {}
                    };

                    let arrived_indicator = if arrived {
                        html! { <span class="peer-arrived-check">{"✓"}</span> }
                    } else {
                        html! {}
                    };

                    html! {
                        <div class={item_class} key={player.player_id.clone()}>
                            {rank_display}
                            <span class={classes!("peer-status", status_class)}>{status_icon}</span>
                            <span class="peer-name">{&player.player_id}</span>
                            {arrived_indicator}
                            {you_tag}
                            {rtt_display}
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
