//! P2P room internal state management.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use marble_core::GameState;
use marble_proto::room::PeerTopology;
use matchbox_socket::{PeerId, WebRtcSocket};

use super::types::{P2pPeerInfo, P2pRoomConfig, ReceivedMessage};
use super::GossipHandler;

/// Internal state for P2P room connection
pub struct P2pRoomState {
    /// Room ID
    pub room_id: String,
    /// Player ID
    pub player_id: String,
    /// Configuration
    pub config: P2pRoomConfig,
    /// WebRTC socket
    pub socket: Option<Rc<RefCell<WebRtcSocket>>>,
    /// Gossip handler
    pub gossip: Option<Rc<RefCell<GossipHandler>>>,
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
    /// Flag to signal peers data changed (for async updates)
    pub peers_dirty: bool,

    // === Game synchronization fields ===
    /// Is this client the host?
    pub is_host: bool,
    /// Host's peer ID
    pub host_peer_id: Option<PeerId>,
    /// Shared game state reference
    pub game_state: Option<Rc<RefCell<GameState>>>,
    /// Last frame number when hash was broadcast
    pub last_hash_frame: u64,
    /// Consecutive desync count
    pub desync_count: u32,
    /// Last frame number when sync was performed
    pub last_sync_frame: u64,
    /// Last received host hash (frame, hash)
    pub last_host_hash: Option<(u64, u64)>,
    /// Current session version (incremented on each game start)
    pub current_session_version: u64,
}

impl P2pRoomState {
    /// Create new room state
    pub fn new(room_id: String, player_id: String, config: P2pRoomConfig) -> Self {
        Self {
            room_id,
            player_id,
            config,
            socket: None,
            gossip: None,
            topology: None,
            peers: Vec::new(),
            messages: Vec::new(),
            rtt_map: HashMap::new(),
            peer_player_map: HashMap::new(),
            new_messages_queue: Vec::new(),
            is_running: false,
            peers_dirty: false,
            // Game synchronization
            is_host: false,
            host_peer_id: None,
            game_state: None,
            last_hash_frame: 0,
            desync_count: 0,
            last_sync_frame: 0,
            last_host_hash: None,
            current_session_version: 0,
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
            self.peers_dirty = true;
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

    /// Get peer IDs for gossip handler
    pub fn get_peer_ids(&self) -> Vec<PeerId> {
        self.peers.iter().map(|p| p.peer_id).collect()
    }

    /// Reset connection state
    pub fn reset_connection(&mut self) {
        self.is_running = false;
        self.socket = None;
        self.gossip = None;
        self.peers.clear();
        // Reset game sync state
        self.game_state = None;
        self.last_hash_frame = 0;
        self.desync_count = 0;
        self.last_sync_frame = 0;
        self.last_host_hash = None;
        self.current_session_version = 0;
    }

    /// Set host status
    pub fn set_host_status(&mut self, is_host: bool) {
        self.is_host = is_host;
    }

    /// Set host peer ID
    pub fn set_host_peer_id(&mut self, peer_id: Option<PeerId>) {
        self.host_peer_id = peer_id;
    }

    /// Set game state reference
    pub fn set_game_state(&mut self, state: Rc<RefCell<GameState>>) {
        self.game_state = Some(state);
    }
}
