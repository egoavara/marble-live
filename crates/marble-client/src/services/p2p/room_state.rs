//! P2P room internal state management.
//!
//! After refactor: Socket and gossip are managed by Bevy (marble-core).
//! Game synchronization (hash, desync, snapshot) is also in Bevy.
//! This state only tracks UI-relevant data: peers, messages, connection flags.

use std::collections::HashMap;

use marble_proto::room::PeerTopology;
use matchbox_socket::PeerId;

use super::types::{P2pPeerInfo, P2pRoomConfig, ReceivedMessage};

/// Internal state for P2P room connection
pub struct P2pRoomState {
    /// Room ID
    pub room_id: String,
    /// Player ID
    pub player_id: String,
    /// Configuration
    pub config: P2pRoomConfig,
    /// Topology info
    pub topology: Option<PeerTopology>,
    /// Connected peers
    pub peers: Vec<P2pPeerInfo>,
    /// Message history
    pub messages: Vec<ReceivedMessage>,
    /// RTT measurements (peer_id -> rtt_ms)
    pub rtt_map: HashMap<PeerId, u32>,
    /// peer_id → player_id mapping
    pub peer_player_map: HashMap<PeerId, String>,
    /// New messages queue for consume pattern
    pub new_messages_queue: Vec<ReceivedMessage>,
    /// Connection running flag
    pub is_running: bool,

    // === Minimal game state flags ===
    /// Is this client the host?
    pub is_host: bool,
    /// Host's peer ID
    pub host_peer_id: Option<PeerId>,
}

impl P2pRoomState {
    /// Create new room state
    pub fn new(room_id: String, player_id: String, config: P2pRoomConfig) -> Self {
        Self {
            room_id,
            player_id,
            config,
            topology: None,
            peers: Vec::new(),
            messages: Vec::new(),
            rtt_map: HashMap::new(),
            peer_player_map: HashMap::new(),
            new_messages_queue: Vec::new(),
            is_running: false,
            // Game flags
            is_host: false,
            host_peer_id: None,
        }
    }

    /// Add a message to history
    pub fn add_message(&mut self, msg: ReceivedMessage) {
        self.messages.push(msg.clone());
        while self.messages.len() > self.config.max_messages {
            self.messages.remove(0);
        }
        self.new_messages_queue.push(msg);
    }

    /// Add or update a peer
    pub fn add_peer(&mut self, peer_id: PeerId) {
        if !self.peers.iter().any(|p| p.peer_id == peer_id) {
            let player_id = self.peer_player_map.get(&peer_id).cloned();
            self.peers.push(P2pPeerInfo {
                peer_id,
                player_id,
                connected: true,
                rtt_ms: None,
            });
        }
    }

    /// Update peer_id → player_id mapping
    pub fn update_peer_player_id(&mut self, peer_id: PeerId, player_id: String) {
        self.peer_player_map.insert(peer_id, player_id.clone());
        if let Some(peer) = self.peers.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.player_id = Some(player_id);
        }
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: PeerId) {
        self.peers.retain(|p| p.peer_id != peer_id);
    }

    /// Update peer RTT
    pub fn update_peer_rtt(&mut self, peer_id: PeerId, rtt_ms: u32) {
        self.rtt_map.insert(peer_id, rtt_ms);
        if let Some(peer) = self.peers.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.rtt_ms = Some(rtt_ms);
        }
    }

    /// Get peer IDs
    pub fn get_peer_ids(&self) -> Vec<PeerId> {
        self.peers.iter().map(|p| p.peer_id).collect()
    }

    /// Reset connection state
    pub fn reset_connection(&mut self) {
        self.is_running = false;
        self.peers.clear();
    }
}
