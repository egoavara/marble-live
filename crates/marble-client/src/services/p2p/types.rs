//! P2P room types and configuration.

use marble_proto::play::p2p_message::Payload;
use matchbox_socket::PeerId;

/// P2P connection state
#[derive(Clone, Debug, PartialEq)]
pub enum P2pConnectionState {
    /// Initial state, not connected
    Disconnected,
    /// Connecting to signaling server
    Connecting,
    /// Connected and ready for P2P communication
    Connected,
    /// Connection failed
    Error(String),
}

impl Default for P2pConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Peer information
#[derive(Clone, Debug, PartialEq)]
pub struct P2pPeerInfo {
    pub peer_id: PeerId,
    pub player_id: Option<String>,
    pub connected: bool,
    pub rtt_ms: Option<u32>,
}

/// Received message from P2P network
#[derive(Clone, Debug, PartialEq)]
pub struct ReceivedMessage {
    /// Unique message ID for deduplication
    pub id: String,
    /// Sender's player ID
    pub from_player: String,
    /// Sender's peer ID (None for local messages)
    pub from_peer: Option<PeerId>,
    /// Message payload
    pub payload: Payload,
    /// Receive timestamp (ms)
    pub timestamp: f64,
}

/// Configuration for P2P room connection
#[derive(Clone, Debug)]
pub struct P2pRoomConfig {
    /// Signaling server URL (default: ws://localhost:3000/signaling/{room_id})
    pub signaling_url: Option<String>,
    /// Gossip TTL (default: 10)
    pub gossip_ttl: u32,
    /// Auto connect on room_id change (default: false)
    pub auto_connect: bool,
    /// Maximum message history size (default: 100)
    pub max_messages: usize,
    /// Whether to store Ping/Pong in message history (default: false)
    pub store_ping_pong: bool,
    /// Player secret for authentication (used for RegisterPeerId)
    pub player_secret: Option<String>,
}

impl Default for P2pRoomConfig {
    fn default() -> Self {
        Self {
            signaling_url: None,
            gossip_ttl: 10,
            auto_connect: false,
            max_messages: 100,
            store_ping_pong: false,
            player_secret: None,
        }
    }
}
