//! P2P message protocol handling.
//!
//! Defines the message types and serialization for P2P communication.

use marble_core::Color;
use matchbox_socket::PeerId;
use uuid::Uuid;

/// Message type identifiers.
pub mod msg_type {
    pub const PLAYER_INFO: u8 = 0x01;
    pub const PEER_ANNOUNCE: u8 = 0x02;
    pub const FRAME_HASH: u8 = 0x04;
    pub const SYNC_REQUEST: u8 = 0x05;
    pub const SYNC_STATE: u8 = 0x06;
    pub const RECONNECT_REQUEST: u8 = 0x07;
    pub const RECONNECT_RESPONSE: u8 = 0x08;
    pub const GAME_START_ORDER: u8 = 0x09;
    pub const PING: u8 = 0xFE;
    pub const PONG: u8 = 0xFF;
}

/// P2P message types.
#[derive(Debug, Clone)]
pub enum P2PMessage {
    /// Player info message (name, color, hash code).
    /// Used for display purposes after PeerAnnounce.
    PlayerInfo {
        name: String,
        color: Color,
        hash_code: String,
    },
    /// Peer announcement - maps peer_id to server player_id.
    /// Sent when P2P connection is established.
    PeerAnnounce {
        player_id: String,
    },
    /// Game start using server-defined player order.
    /// Host sends this after StartGame succeeds on server.
    GameStartOrder {
        seed: u64,
        player_order: Vec<String>,  // player_ids in join_order
    },
    /// Frame hash for sync verification.
    FrameHash {
        frame: u64,
        hash: u64,
    },
    /// Request sync from a specific frame.
    SyncRequest {
        from_frame: u64,
    },
    /// Sync state response.
    SyncState {
        frame: u64,
        state: Vec<u8>,
    },
    /// Reconnection request - sent when a player rejoins during gameplay.
    ReconnectRequest {
        /// Player's name
        name: String,
        /// Player's color
        color: Color,
        /// Player's hash code
        hash_code: String,
    },
    /// Reconnection response - contains full game state for reconnecting player.
    ReconnectResponse {
        /// Current game seed
        seed: u64,
        /// Current frame number
        frame: u64,
        /// Serialized game state
        state: Vec<u8>,
        /// Player list with peer_id mappings
        players: Vec<PlayerStartInfo>,
    },
    /// Ping message for RTT measurement.
    Ping {
        timestamp: f64,
    },
    /// Pong response to ping.
    Pong {
        timestamp: f64,
    },
}

/// Player info for game start.
#[derive(Debug, Clone)]
pub struct PlayerStartInfo {
    pub peer_id_bytes: [u8; 16],
    pub name: String,
    pub color: Color,
}

impl PlayerStartInfo {
    pub fn new(peer_id: PeerId, name: String, color: Color) -> Self {
        Self {
            peer_id_bytes: *peer_id.0.as_bytes(),
            name,
            color,
        }
    }

    pub fn peer_id(&self) -> PeerId {
        PeerId(Uuid::from_bytes(self.peer_id_bytes))
    }
}

impl P2PMessage {
    /// Encode the message to bytes.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            P2PMessage::PlayerInfo { name, color, hash_code } => {
                let mut buf = vec![msg_type::PLAYER_INFO];
                // Color (3 bytes: r, g, b)
                buf.push(color.r);
                buf.push(color.g);
                buf.push(color.b);
                // Name length (2 bytes) + name bytes
                let name_bytes = name.as_bytes();
                buf.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
                buf.extend_from_slice(name_bytes);
                // Hash code length (1 byte) + hash code bytes
                let hash_bytes = hash_code.as_bytes();
                buf.push(hash_bytes.len() as u8);
                buf.extend_from_slice(hash_bytes);
                buf
            }
            P2PMessage::PeerAnnounce { player_id } => {
                let mut buf = vec![msg_type::PEER_ANNOUNCE];
                // Player ID length (2 bytes) + player_id bytes
                let id_bytes = player_id.as_bytes();
                buf.extend_from_slice(&(id_bytes.len() as u16).to_be_bytes());
                buf.extend_from_slice(id_bytes);
                buf
            }
            P2PMessage::GameStartOrder { seed, player_order } => {
                let mut buf = vec![msg_type::GAME_START_ORDER];
                buf.extend_from_slice(&seed.to_be_bytes());
                buf.push(player_order.len() as u8);
                for player_id in player_order {
                    // Player ID length (2 bytes) + player_id bytes
                    let id_bytes = player_id.as_bytes();
                    buf.extend_from_slice(&(id_bytes.len() as u16).to_be_bytes());
                    buf.extend_from_slice(id_bytes);
                }
                buf
            }
            P2PMessage::FrameHash { frame, hash } => {
                let mut buf = vec![msg_type::FRAME_HASH];
                buf.extend_from_slice(&frame.to_be_bytes());
                buf.extend_from_slice(&hash.to_be_bytes());
                buf
            }
            P2PMessage::SyncRequest { from_frame } => {
                let mut buf = vec![msg_type::SYNC_REQUEST];
                buf.extend_from_slice(&from_frame.to_be_bytes());
                buf
            }
            P2PMessage::SyncState { frame, state } => {
                let mut buf = vec![msg_type::SYNC_STATE];
                buf.extend_from_slice(&frame.to_be_bytes());
                buf.extend_from_slice(&(state.len() as u32).to_be_bytes());
                buf.extend_from_slice(state);
                buf
            }
            P2PMessage::ReconnectRequest { name, color, hash_code } => {
                let mut buf = vec![msg_type::RECONNECT_REQUEST];
                // Color (3 bytes)
                buf.push(color.r);
                buf.push(color.g);
                buf.push(color.b);
                // Name length (2 bytes) + name bytes
                let name_bytes = name.as_bytes();
                buf.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
                buf.extend_from_slice(name_bytes);
                // Hash code length (1 byte) + hash code bytes
                let hash_bytes = hash_code.as_bytes();
                buf.push(hash_bytes.len() as u8);
                buf.extend_from_slice(hash_bytes);
                buf
            }
            P2PMessage::ReconnectResponse { seed, frame, state, players } => {
                let mut buf = vec![msg_type::RECONNECT_RESPONSE];
                // Seed (8 bytes)
                buf.extend_from_slice(&seed.to_be_bytes());
                // Frame (8 bytes)
                buf.extend_from_slice(&frame.to_be_bytes());
                // State length (4 bytes) + state
                buf.extend_from_slice(&(state.len() as u32).to_be_bytes());
                buf.extend_from_slice(state);
                // Player count (1 byte) + players
                buf.push(players.len() as u8);
                for player in players {
                    // Peer ID (16 bytes)
                    buf.extend_from_slice(&player.peer_id_bytes);
                    // Color (3 bytes)
                    buf.push(player.color.r);
                    buf.push(player.color.g);
                    buf.push(player.color.b);
                    // Name length (1 byte) + name bytes
                    let name_bytes = player.name.as_bytes();
                    buf.push(name_bytes.len() as u8);
                    buf.extend_from_slice(name_bytes);
                }
                buf
            }
            P2PMessage::Ping { timestamp } => {
                let mut buf = vec![msg_type::PING];
                buf.extend_from_slice(&timestamp.to_be_bytes());
                buf
            }
            P2PMessage::Pong { timestamp } => {
                let mut buf = vec![msg_type::PONG];
                buf.extend_from_slice(&timestamp.to_be_bytes());
                buf
            }
        }
    }

    /// Decode a message from bytes.
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        match data[0] {
            msg_type::PLAYER_INFO if data.len() >= 6 => {
                let color = Color::rgb(data[1], data[2], data[3]);
                let name_len = u16::from_be_bytes([data[4], data[5]]) as usize;
                if data.len() < 6 + name_len + 1 {
                    return None;
                }
                let name = String::from_utf8_lossy(&data[6..6 + name_len]).to_string();
                let hash_offset = 6 + name_len;
                let hash_len = data[hash_offset] as usize;
                if data.len() < hash_offset + 1 + hash_len {
                    return None;
                }
                let hash_code = String::from_utf8_lossy(&data[hash_offset + 1..hash_offset + 1 + hash_len]).to_string();
                Some(P2PMessage::PlayerInfo { name, color, hash_code })
            }
            msg_type::PEER_ANNOUNCE if data.len() >= 3 => {
                let id_len = u16::from_be_bytes([data[1], data[2]]) as usize;
                if data.len() < 3 + id_len {
                    return None;
                }
                let player_id = String::from_utf8_lossy(&data[3..3 + id_len]).to_string();
                Some(P2PMessage::PeerAnnounce { player_id })
            }
            msg_type::GAME_START_ORDER if data.len() >= 10 => {
                let seed = u64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                let player_count = data[9] as usize;
                let mut offset = 10;
                let mut player_order = Vec::with_capacity(player_count);

                for _ in 0..player_count {
                    if offset + 2 > data.len() {
                        return None;
                    }
                    let id_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                    offset += 2;

                    if offset + id_len > data.len() {
                        return None;
                    }
                    let player_id = String::from_utf8_lossy(&data[offset..offset + id_len]).to_string();
                    offset += id_len;
                    player_order.push(player_id);
                }

                Some(P2PMessage::GameStartOrder { seed, player_order })
            }
            msg_type::FRAME_HASH if data.len() >= 17 => {
                let frame = u64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                let hash = u64::from_be_bytes([
                    data[9], data[10], data[11], data[12],
                    data[13], data[14], data[15], data[16],
                ]);
                Some(P2PMessage::FrameHash { frame, hash })
            }
            msg_type::SYNC_REQUEST if data.len() >= 9 => {
                let from_frame = u64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                Some(P2PMessage::SyncRequest { from_frame })
            }
            msg_type::SYNC_STATE if data.len() >= 13 => {
                let frame = u64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                let state_len = u32::from_be_bytes([
                    data[9], data[10], data[11], data[12],
                ]) as usize;
                if data.len() >= 13 + state_len {
                    let state = data[13..13 + state_len].to_vec();
                    Some(P2PMessage::SyncState { frame, state })
                } else {
                    None
                }
            }
            msg_type::RECONNECT_REQUEST if data.len() >= 6 => {
                let color = Color::rgb(data[1], data[2], data[3]);
                let name_len = u16::from_be_bytes([data[4], data[5]]) as usize;
                if data.len() < 6 + name_len + 1 {
                    return None;
                }
                let name = String::from_utf8_lossy(&data[6..6 + name_len]).to_string();
                let hash_offset = 6 + name_len;
                let hash_len = data[hash_offset] as usize;
                if data.len() < hash_offset + 1 + hash_len {
                    return None;
                }
                let hash_code = String::from_utf8_lossy(&data[hash_offset + 1..hash_offset + 1 + hash_len]).to_string();
                Some(P2PMessage::ReconnectRequest { name, color, hash_code })
            }
            msg_type::RECONNECT_RESPONSE if data.len() >= 21 => {
                let seed = u64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                let frame = u64::from_be_bytes([
                    data[9], data[10], data[11], data[12],
                    data[13], data[14], data[15], data[16],
                ]);
                let state_len = u32::from_be_bytes([
                    data[17], data[18], data[19], data[20],
                ]) as usize;
                if data.len() < 21 + state_len + 1 {
                    return None;
                }
                let state = data[21..21 + state_len].to_vec();
                let player_offset = 21 + state_len;
                let player_count = data[player_offset] as usize;
                let mut offset = player_offset + 1;
                let mut players = Vec::with_capacity(player_count);

                for _ in 0..player_count {
                    if offset + 20 > data.len() {
                        return None;
                    }
                    // Peer ID (16 bytes)
                    let mut peer_id_bytes = [0u8; 16];
                    peer_id_bytes.copy_from_slice(&data[offset..offset + 16]);
                    offset += 16;

                    // Color (3 bytes)
                    let color = Color::rgb(data[offset], data[offset + 1], data[offset + 2]);
                    offset += 3;

                    // Name length (1 byte) + name
                    let name_len = data[offset] as usize;
                    offset += 1;

                    if offset + name_len > data.len() {
                        return None;
                    }
                    let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
                    offset += name_len;

                    players.push(PlayerStartInfo {
                        peer_id_bytes,
                        name,
                        color,
                    });
                }

                Some(P2PMessage::ReconnectResponse { seed, frame, state, players })
            }
            msg_type::PING if data.len() >= 9 => {
                let timestamp = f64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                Some(P2PMessage::Ping { timestamp })
            }
            msg_type::PONG if data.len() >= 9 => {
                let timestamp = f64::from_be_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                Some(P2PMessage::Pong { timestamp })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_info_roundtrip() {
        let msg = P2PMessage::PlayerInfo {
            name: "TestPlayer".to_string(),
            color: Color::RED,
            hash_code: "1A2B".to_string(),
        };
        let encoded = msg.encode();
        let decoded = P2PMessage::decode(&encoded).unwrap();

        if let P2PMessage::PlayerInfo { name, color, hash_code } = decoded {
            assert_eq!(name, "TestPlayer");
            assert_eq!(color, Color::RED);
            assert_eq!(hash_code, "1A2B");
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_frame_hash_roundtrip() {
        let msg = P2PMessage::FrameHash {
            frame: 12345,
            hash: 0xDEADBEEF,
        };
        let encoded = msg.encode();
        let decoded = P2PMessage::decode(&encoded).unwrap();

        if let P2PMessage::FrameHash { frame, hash } = decoded {
            assert_eq!(frame, 12345);
            assert_eq!(hash, 0xDEADBEEF);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_ping_pong_roundtrip() {
        let ts = 1234567890.123;

        let ping = P2PMessage::Ping { timestamp: ts };
        let encoded = ping.encode();
        let decoded = P2PMessage::decode(&encoded).unwrap();

        if let P2PMessage::Ping { timestamp } = decoded {
            assert!((timestamp - ts).abs() < 0.001);
        } else {
            panic!("Wrong message type");
        }

        let pong = P2PMessage::Pong { timestamp: ts };
        let encoded = pong.encode();
        let decoded = P2PMessage::decode(&encoded).unwrap();

        if let P2PMessage::Pong { timestamp } = decoded {
            assert!((timestamp - ts).abs() < 0.001);
        } else {
            panic!("Wrong message type");
        }
    }
}
