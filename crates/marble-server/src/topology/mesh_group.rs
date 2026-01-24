use std::collections::{HashMap, HashSet};

/// Mesh group containing players
#[derive(Debug, Clone)]
pub struct MeshGroup {
    /// Group ID
    pub group_id: u32,
    /// Maximum size of the group
    pub max_size: u32,
    /// Players in this group (player_id -> peer_id mapping)
    pub players: HashMap<String, String>,
    /// Bridge node player IDs (typically 2 per group)
    pub bridge_players: HashSet<String>,
}

impl MeshGroup {
    pub fn new(group_id: u32, max_size: u32) -> Self {
        Self {
            group_id,
            max_size,
            players: HashMap::new(),
            bridge_players: HashSet::new(),
        }
    }

    /// Check if the group has room for more players
    pub fn has_capacity(&self) -> bool {
        self.players.len() < self.max_size as usize
    }

    /// Get current player count
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Add a player to the group
    pub fn add_player(&mut self, player_id: String, peer_id: String) -> bool {
        if !self.has_capacity() {
            return false;
        }
        self.players.insert(player_id, peer_id);
        true
    }

    /// Remove a player from the group
    pub fn remove_player(&mut self, player_id: &str) -> bool {
        self.bridge_players.remove(player_id);
        self.players.remove(player_id).is_some()
    }

    /// Set bridge players for this group
    pub fn set_bridges(&mut self, bridge_ids: HashSet<String>) {
        self.bridge_players = bridge_ids;
    }

    /// Check if a player is a bridge
    pub fn is_bridge(&self, player_id: &str) -> bool {
        self.bridge_players.contains(player_id)
    }

    /// Get all player IDs except the given one
    pub fn get_other_players(&self, exclude_player_id: &str) -> Vec<(String, String)> {
        self.players
            .iter()
            .filter(|(pid, _)| *pid != exclude_player_id)
            .map(|(pid, peer_id)| (pid.clone(), peer_id.clone()))
            .collect()
    }

    /// Select random peers for connection (up to max_connections)
    pub fn select_peers_for_player(
        &self,
        player_id: &str,
        max_connections: usize,
    ) -> Vec<(String, String)> {
        let others = self.get_other_players(player_id);

        if others.len() <= max_connections {
            // If fewer than max, connect to all
            others
        } else {
            // Select a subset - prefer bridges and then random
            let mut result: Vec<(String, String)> = Vec::new();

            // First add bridges
            for (pid, peer_id) in &others {
                if self.bridge_players.contains(pid) && result.len() < max_connections {
                    result.push((pid.clone(), peer_id.clone()));
                }
            }

            // Then add non-bridges until we reach max
            for (pid, peer_id) in &others {
                if !self.bridge_players.contains(pid) && result.len() < max_connections {
                    result.push((pid.clone(), peer_id.clone()));
                }
            }

            result
        }
    }

    /// Get bridge players with their peer IDs
    pub fn get_bridge_peer_ids(&self) -> Vec<(String, String)> {
        self.bridge_players
            .iter()
            .filter_map(|pid| self.players.get(pid).map(|peer_id| (pid.clone(), peer_id.clone())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_group_capacity() {
        let mut group = MeshGroup::new(0, 3);
        assert!(group.has_capacity());

        group.add_player("p1".to_string(), "peer1".to_string());
        group.add_player("p2".to_string(), "peer2".to_string());
        assert!(group.has_capacity());

        group.add_player("p3".to_string(), "peer3".to_string());
        assert!(!group.has_capacity());
    }

    #[test]
    fn test_bridge_selection() {
        let mut group = MeshGroup::new(0, 10);
        for i in 0..5 {
            group.add_player(format!("p{}", i), format!("peer{}", i));
        }

        group.set_bridges(["p1".to_string(), "p2".to_string()].into());
        assert!(group.is_bridge("p1"));
        assert!(group.is_bridge("p2"));
        assert!(!group.is_bridge("p0"));
    }
}
