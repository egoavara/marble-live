//! Session management for P2P game.
//!
//! Provides utilities for peer management.
//! Note: Host status is now determined by the server (room creator),
//! not by P2P peer election.

use marble_core::Color;
use matchbox_socket::PeerId;

use crate::p2p::state::{P2PGameState, PeerInfo};

/// Get player info for game start message.
/// Returns a sorted list of (peer_id, name, color) tuples.
pub fn get_player_list(state: &P2PGameState) -> Vec<(PeerId, String, Color)> {
    let mut players = Vec::new();

    // Add self
    if let Some(my_id) = state.my_peer_id {
        players.push((
            my_id,
            if state.my_name.is_empty() {
                format!("Player-{}", &my_id.0.to_string()[..8])
            } else {
                state.my_name.clone()
            },
            state.my_color,
        ));
    }

    // Add all peers
    for (peer_id, info) in &state.peers {
        players.push((*peer_id, info.name.clone(), info.color));
    }

    // Sort by peer ID for deterministic order
    players.sort_by_key(|(id, _, _)| *id);
    players
}

/// Assign colors to players based on their index.
pub fn assign_player_color(index: usize) -> Color {
    match index % 8 {
        0 => Color::RED,
        1 => Color::BLUE,
        2 => Color::GREEN,
        3 => Color::ORANGE,
        4 => Color::PURPLE,
        5 => Color::YELLOW,
        6 => Color::CYAN,
        _ => Color::PINK,
    }
}

/// Find a peer by their ID.
pub fn find_peer<'a>(peers: &'a std::collections::HashMap<PeerId, PeerInfo>, peer_id: PeerId) -> Option<&'a PeerInfo> {
    peers.get(&peer_id)
}

/// Get the average RTT across all peers.
pub fn average_rtt(state: &P2PGameState) -> Option<u32> {
    let rtts: Vec<u32> = state
        .peers
        .values()
        .filter_map(|p| p.rtt_ms)
        .collect();

    if rtts.is_empty() {
        None
    } else {
        Some(rtts.iter().sum::<u32>() / rtts.len() as u32)
    }
}

/// Get the maximum RTT among all peers.
pub fn max_rtt(state: &P2PGameState) -> Option<u32> {
    state.peers.values().filter_map(|p| p.rtt_ms).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assign_player_color() {
        let c0 = assign_player_color(0);
        let c1 = assign_player_color(1);
        let c8 = assign_player_color(8);

        assert_eq!(c0, Color::RED);
        assert_eq!(c1, Color::BLUE);
        assert_eq!(c8, Color::RED); // Wraps around
    }
}
