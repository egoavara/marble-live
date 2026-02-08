use std::collections::HashMap;

use marble_proto::room::{PeerConnection, PeerTopology};
use matchbox_socket::PeerId;

/// Maps player_id to peer_id for WebRTC connections
#[derive(Debug, Clone)]
pub struct PeerMapping {
    pub player_id: String,
    pub peer_id: PeerId,
}

/// Handles topology management and peer connection decisions
pub struct TopologyHandler {
    /// My player ID
    my_player_id: String,
    /// My mesh group
    mesh_group: u32,
    /// Whether I'm a bridge node
    is_bridge: bool,
    /// Peers I should connect to (player_id -> peer_id from proto)
    expected_peers: HashMap<String, String>,
    /// Bridge peers I should connect to (if I'm a bridge)
    expected_bridges: HashMap<String, String>,
    /// Currently connected peers (player_id -> PeerId)
    connected_peers: HashMap<String, PeerId>,
}

impl TopologyHandler {
    pub fn new(my_player_id: String) -> Self {
        Self {
            my_player_id,
            mesh_group: 0,
            is_bridge: false,
            expected_peers: HashMap::new(),
            expected_bridges: HashMap::new(),
            connected_peers: HashMap::new(),
        }
    }

    /// Apply new topology from server
    pub fn apply_topology(&mut self, topology: &PeerTopology) {
        self.mesh_group = topology.mesh_group;
        self.is_bridge = topology.is_bridge;

        // Update expected peers
        self.expected_peers.clear();
        for peer in &topology.connect_to {
            self.expected_peers
                .insert(peer.player_id.clone(), peer.peer_id.clone());
        }

        // Update expected bridges
        self.expected_bridges.clear();
        for peer in &topology.bridge_peers {
            self.expected_bridges
                .insert(peer.player_id.clone(), peer.peer_id.clone());
        }
    }

    /// Check if we should connect to a peer
    /// Called when a new peer is discovered via signaling
    pub fn should_connect(&self, peer_player_id: &str) -> bool {
        self.expected_peers.contains_key(peer_player_id)
            || (self.is_bridge && self.expected_bridges.contains_key(peer_player_id))
    }

    /// Register a connected peer
    pub fn on_peer_connected(&mut self, player_id: String, peer_id: PeerId) {
        self.connected_peers.insert(player_id, peer_id);
    }

    /// Handle peer disconnection
    pub fn on_peer_disconnected(&mut self, peer_id: &PeerId) -> Option<String> {
        let mut disconnected_player = None;
        self.connected_peers.retain(|player_id, pid| {
            if pid == peer_id {
                disconnected_player = Some(player_id.clone());
                false
            } else {
                true
            }
        });
        disconnected_player
    }

    /// Get all connected peer IDs
    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.connected_peers.values().copied().collect()
    }

    /// Get connected peers in the same group
    pub fn get_group_peers(&self) -> Vec<PeerId> {
        // All expected_peers are in the same group
        self.expected_peers
            .keys()
            .filter_map(|pid| self.connected_peers.get(pid).copied())
            .collect()
    }

    /// Get connected bridge peers (only if this node is a bridge)
    pub fn get_bridge_peers(&self) -> Vec<PeerId> {
        if !self.is_bridge {
            return vec![];
        }
        self.expected_bridges
            .keys()
            .filter_map(|pid| self.connected_peers.get(pid).copied())
            .collect()
    }

    /// Get mesh group ID
    pub fn mesh_group(&self) -> u32 {
        self.mesh_group
    }

    /// Check if this node is a bridge
    pub fn is_bridge(&self) -> bool {
        self.is_bridge
    }

    /// Get my player ID
    pub fn my_player_id(&self) -> &str {
        &self.my_player_id
    }

    /// Get connection status for all expected peers
    pub fn get_connection_status(&self) -> Vec<(String, bool)> {
        let mut status = Vec::new();

        for (player_id, _) in &self.expected_peers {
            let connected = self.connected_peers.contains_key(player_id);
            status.push((player_id.clone(), connected));
        }

        if self.is_bridge {
            for (player_id, _) in &self.expected_bridges {
                let connected = self.connected_peers.contains_key(player_id);
                status.push((player_id.clone(), connected));
            }
        }

        status
    }

    /// Find player_id by peer_id
    pub fn find_player_by_peer(&self, peer_id: &PeerId) -> Option<String> {
        for (player_id, pid) in &self.connected_peers {
            if pid == peer_id {
                return Some(player_id.clone());
            }
        }
        None
    }

    /// Get expected peer count
    pub fn expected_peer_count(&self) -> usize {
        self.expected_peers.len()
            + if self.is_bridge {
                self.expected_bridges.len()
            } else {
                0
            }
    }

    /// Get connected peer count
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn create_test_topology() -> PeerTopology {
        PeerTopology {
            mesh_group: 1,
            is_bridge: true,
            connect_to: vec![
                PeerConnection {
                    player_id: "p1".to_string(),
                    peer_id: "peer1".to_string(),
                },
                PeerConnection {
                    player_id: "p2".to_string(),
                    peer_id: "peer2".to_string(),
                },
            ],
            bridge_peers: vec![PeerConnection {
                player_id: "p3".to_string(),
                peer_id: "peer3".to_string(),
            }],
        }
    }

    #[wasm_bindgen_test]
    fn test_apply_topology() {
        let mut handler = TopologyHandler::new("me".to_string());
        let topology = create_test_topology();

        handler.apply_topology(&topology);

        assert_eq!(handler.mesh_group(), 1);
        assert!(handler.is_bridge());
        assert_eq!(handler.expected_peer_count(), 3);
    }

    #[wasm_bindgen_test]
    fn test_should_connect() {
        let mut handler = TopologyHandler::new("me".to_string());
        handler.apply_topology(&create_test_topology());

        assert!(handler.should_connect("p1"));
        assert!(handler.should_connect("p2"));
        assert!(handler.should_connect("p3")); // bridge peer
        assert!(!handler.should_connect("unknown"));
    }
}
