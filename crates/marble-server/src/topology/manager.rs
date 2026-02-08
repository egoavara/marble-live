use std::collections::HashMap;

use marble_proto::room::{PeerConnection, PeerConnectionStatus, PeerTopology};

use super::{BridgeSelector, MeshGroup};

/// Configuration for topology manager
#[derive(Debug, Clone)]
pub struct TopologyManagerConfig {
    /// Maximum players in a mesh group
    pub mesh_group_size: u32,
    /// Number of connections per peer
    pub peer_connections: u32,
    /// Number of bridges per group
    pub bridges_per_group: usize,
    /// Gossip TTL (hop limit)
    pub gossip_ttl: u32,
    /// Lockstep delay frames
    pub lockstep_delay_frames: u32,
}

impl Default for TopologyManagerConfig {
    fn default() -> Self {
        Self {
            mesh_group_size: 35,
            peer_connections: 5,
            bridges_per_group: 2,
            gossip_ttl: 10,
            lockstep_delay_frames: 6,
        }
    }
}

/// Topology manager for a room
#[derive(Debug, Clone)]
pub struct TopologyManager {
    /// Configuration
    pub config: TopologyManagerConfig,
    /// Mesh groups
    groups: Vec<MeshGroup>,
    /// Player to group mapping (`player_id` -> `group_id`)
    player_groups: HashMap<String, u32>,
    /// Player to `peer_id` mapping
    player_peers: HashMap<String, String>,
    /// Bridge selector
    bridge_selector: BridgeSelector,
    /// Flag indicating if topology needs recalculation
    topology_dirty: bool,
}

impl TopologyManager {
    pub fn new(config: TopologyManagerConfig) -> Self {
        Self {
            bridge_selector: BridgeSelector::new(config.bridges_per_group),
            config,
            groups: Vec::new(),
            player_groups: HashMap::new(),
            player_peers: HashMap::new(),
            topology_dirty: false,
        }
    }

    /// Add a new player and assign topology
    pub fn add_player(&mut self, player_id: &str, peer_id: &str) -> PeerTopology {
        // Find or create a group with capacity
        let group_id = self.find_or_create_group();

        // Add player to the group
        let group = &mut self.groups[group_id as usize];
        group.add_player(player_id.to_string(), peer_id.to_string());

        // Track player
        self.player_groups.insert(player_id.to_string(), group_id);
        self.player_peers
            .insert(player_id.to_string(), peer_id.to_string());

        // Mark topology as dirty (may need bridge reselection)
        self.topology_dirty = true;

        // Generate topology for the player
        self.generate_topology(player_id)
    }

    /// Remove a player from topology
    pub fn remove_player(&mut self, player_id: &str) {
        if let Some(group_id) = self.player_groups.remove(player_id)
            && let Some(group) = self.groups.get_mut(group_id as usize)
        {
            group.remove_player(player_id);
        }
        self.player_peers.remove(player_id);
        self.bridge_selector.remove_player(player_id);
        self.topology_dirty = true;
    }

    /// Update connection status and check if topology changed
    pub fn update_connection_status(
        &mut self,
        player_id: &str,
        statuses: &[PeerConnectionStatus],
    ) -> Option<PeerTopology> {
        // Update quality scores
        for status in statuses {
            self.bridge_selector.update_quality(
                player_id,
                &status.peer_id,
                status.rtt_ms,
                status.packet_loss,
                status.connected,
            );
        }

        // Check if we need to recalculate bridges
        if self.should_recalculate_bridges() {
            self.recalculate_bridges();
            self.topology_dirty = false;

            // Return new topology for this player
            Some(self.generate_topology(player_id))
        } else {
            None
        }
    }

    /// Get current topology for a player
    pub fn get_topology(&self, player_id: &str) -> Option<PeerTopology> {
        if self.player_groups.contains_key(player_id) {
            Some(self.generate_topology(player_id))
        } else {
            None
        }
    }

    /// Generate topology for a specific player
    fn generate_topology(&self, player_id: &str) -> PeerTopology {
        let group_id = self.player_groups.get(player_id).copied().unwrap_or(0);
        let group = &self.groups[group_id as usize];

        let is_bridge = group.is_bridge(player_id);

        // Get peers to connect within the group
        let connect_to: Vec<PeerConnection> = group
            .select_peers_for_player(player_id, self.config.peer_connections as usize)
            .into_iter()
            .map(|(uid, peer_id)| PeerConnection {
                user_id: uid,
                peer_id,
            })
            .collect();

        // Get bridge peers from other groups (only for bridge nodes)
        let bridge_peers = if is_bridge {
            self.get_other_group_bridges(group_id)
        } else {
            vec![]
        };

        PeerTopology {
            signaling_url: String::new(), // Set by Room
            mesh_group: group_id,
            is_bridge,
            connect_to,
            bridge_peers,
        }
    }

    /// Find a group with capacity or create a new one
    #[allow(clippy::cast_possible_truncation)]
    fn find_or_create_group(&mut self) -> u32 {
        // Find first group with capacity
        for (i, group) in self.groups.iter().enumerate() {
            if group.has_capacity() {
                return i as u32;
            }
        }

        // Create new group
        let new_group_id = self.groups.len() as u32;
        self.groups
            .push(MeshGroup::new(new_group_id, self.config.mesh_group_size));
        new_group_id
    }

    /// Get bridges from other groups
    fn get_other_group_bridges(&self, exclude_group: u32) -> Vec<PeerConnection> {
        let mut result = Vec::new();

        for group in &self.groups {
            if group.group_id == exclude_group {
                continue;
            }

            // Get bridge players from this group
            for (user_id, peer_id) in group.get_bridge_peer_ids() {
                result.push(PeerConnection { user_id, peer_id });
            }
        }

        result
    }

    /// Check if bridges need recalculation
    fn should_recalculate_bridges(&self) -> bool {
        // Recalculate if topology is dirty or periodically
        self.topology_dirty
    }

    /// Recalculate bridge nodes for all groups
    fn recalculate_bridges(&mut self) {
        for group in &mut self.groups {
            let player_ids: Vec<String> = group.players.keys().cloned().collect();
            let bridges = self.bridge_selector.select_bridges(&player_ids);
            group.set_bridges(bridges.into_iter().collect());
        }
    }

    /// Get total player count
    #[allow(dead_code)]
    pub fn player_count(&self) -> usize {
        self.player_groups.len()
    }

    /// Get group count
    #[allow(dead_code)]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Update `peer_id` for a player (returns true if player exists and was updated)
    pub fn update_peer_id(&mut self, player_id: &str, new_peer_id: &str) -> bool {
        if !self.player_peers.contains_key(player_id) {
            return false;
        }
        self.player_peers
            .insert(player_id.to_string(), new_peer_id.to_string());

        if let Some(&group_id) = self.player_groups.get(player_id)
            && let Some(group) = self.groups.get_mut(group_id as usize)
        {
            group.update_peer_id(player_id, new_peer_id);
        }
        self.topology_dirty = true;
        true
    }

    /// Resolve `peer_ids` to `player_ids`
    pub fn resolve_peer_ids(&self, peer_ids: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        // Build reverse map (peer_id → player_id)
        let peer_to_player: HashMap<&str, &str> = self
            .player_peers
            .iter()
            .map(|(player, peer)| (peer.as_str(), player.as_str()))
            .collect();

        for peer_id in peer_ids {
            if let Some(&player_id) = peer_to_player.get(peer_id.as_str()) {
                result.insert(peer_id.clone(), player_id.to_string());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_player() {
        let config = TopologyManagerConfig {
            mesh_group_size: 3,
            peer_connections: 2,
            ..Default::default()
        };
        let mut manager = TopologyManager::new(config);

        let topology1 = manager.add_player("p1", "peer1");
        assert_eq!(topology1.mesh_group, 0);
        assert!(topology1.connect_to.is_empty()); // First player has no one to connect

        let topology2 = manager.add_player("p2", "peer2");
        assert_eq!(topology2.mesh_group, 0);
        assert_eq!(topology2.connect_to.len(), 1); // Connect to p1

        let topology3 = manager.add_player("p3", "peer3");
        assert_eq!(topology3.mesh_group, 0);
        assert_eq!(topology3.connect_to.len(), 2); // Connect to p1 and p2

        // Fourth player should go to new group
        let topology4 = manager.add_player("p4", "peer4");
        assert_eq!(topology4.mesh_group, 1);
    }

    #[test]
    fn test_remove_player() {
        let config = TopologyManagerConfig::default();
        let mut manager = TopologyManager::new(config);

        manager.add_player("p1", "peer1");
        manager.add_player("p2", "peer2");
        assert_eq!(manager.player_count(), 2);

        manager.remove_player("p1");
        assert_eq!(manager.player_count(), 1);
    }

    #[test]
    fn test_bridge_assignment() {
        let config = TopologyManagerConfig {
            mesh_group_size: 5,
            bridges_per_group: 2,
            ..Default::default()
        };
        let mut manager = TopologyManager::new(config);

        // Add players
        for i in 0..5 {
            manager.add_player(&format!("p{}", i), &format!("peer{}", i));
        }

        // Simulate connection reports to trigger bridge selection
        for i in 0..5 {
            let statuses = vec![PeerConnectionStatus {
                peer_id: format!("peer{}", (i + 1) % 5),
                rtt_ms: 50,
                packet_loss: 0.0,
                connected: true,
            }];
            manager.update_connection_status(&format!("p{}", i), &statuses);
        }

        // Check that bridges are assigned
        let group = &manager.groups[0];
        assert_eq!(group.bridge_players.len(), 2);
    }
}
