//! Network manager for P2P game synchronization

use crate::p2p::state::ServerPlayerInfo;
use marble_core::Color;
use matchbox_socket::{PeerId, PeerState, WebRtcSocket};
use prost::Message;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::grpc_web::room::RoomClient;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { room_id: String, player_id: String },
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    StateChanged(ConnectionState),
    PeerJoined(PeerId),
    PeerLeft(PeerId),
    Message { from: PeerId, data: Vec<u8> },
}

pub struct NetworkManager {
    room_client: RoomClient,
    socket: Option<WebRtcSocket>,
    state: ConnectionState,
    peers: HashMap<PeerId, PeerState>,
    signaling_url: Option<String>,
}

impl NetworkManager {
    pub fn new(grpc_base_url: &str) -> Self {
        Self {
            room_client: RoomClient::new(grpc_base_url),
            socket: None,
            state: ConnectionState::Disconnected,
            peers: HashMap::new(),
            signaling_url: None,
        }
    }

    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    pub fn peers(&self) -> &HashMap<PeerId, PeerState> {
        &self.peers
    }

    /// Get our own peer ID from the socket.
    pub fn my_peer_id(&mut self) -> Option<PeerId> {
        self.socket.as_mut().and_then(|s| s.id())
    }

    pub fn room_client(&self) -> &RoomClient {
        &self.room_client
    }

    /// Result of a successful room join containing (room_id, seed, is_game_in_progress, player_id, is_host, server_players).
    pub async fn create_and_join_room(
        &mut self,
        room_name: &str,
        player_name: &str,
        fingerprint: &str,
        color: Color,
    ) -> Result<(String, u64, bool, String, bool, Vec<ServerPlayerInfo>), String> {
        self.state = ConnectionState::Connecting;

        // Create room via gRPC
        let create_resp = self
            .room_client
            .create_room(room_name, 4)
            .await
            .map_err(|e| e.to_string())?;

        let room_id = create_resp.room_id.clone();
        let signaling_url = create_resp.signaling_url.clone();

        // Join the room we just created
        let join_resp = self
            .room_client
            .join_room(&room_id, player_name, fingerprint, color.r as u32, color.g as u32, color.b as u32)
            .await
            .map_err(|e| e.to_string())?;

        if !join_resp.success {
            self.state = ConnectionState::Disconnected;
            return Err(join_resp.error_message);
        }

        // Get seed and status from room info
        let seed = join_resp.room.as_ref().map(|r| r.seed).unwrap_or(0);
        // Room status 2 = Playing (from proto)
        let is_game_in_progress = join_resp.room.as_ref().map(|r| r.status == 2).unwrap_or(false);
        let player_id = join_resp.player_id.clone();

        // Check if this player is the host
        let is_host = join_resp.room.as_ref()
            .and_then(|r| r.players.iter().find(|p| p.id == player_id))
            .map(|p| p.is_host)
            .unwrap_or(false);

        // Extract server players from response
        let server_players = extract_server_players(&join_resp);

        // Connect to signaling server for P2P
        self.signaling_url = Some(signaling_url.clone());
        self.connect_p2p(&signaling_url)?;

        self.state = ConnectionState::Connected {
            room_id: room_id.clone(),
            player_id: join_resp.player_id,
        };

        Ok((room_id, seed, is_game_in_progress, player_id, is_host, server_players))
    }

    /// Join an existing room. Returns (seed, is_game_in_progress, player_id, is_host, server_players) from the room.
    pub async fn join_room(
        &mut self,
        room_id: &str,
        player_name: &str,
        fingerprint: &str,
        color: Color,
    ) -> Result<(u64, bool, String, bool, Vec<ServerPlayerInfo>), String> {
        self.state = ConnectionState::Connecting;

        // First get room info to get signaling URL
        let room_resp = self
            .room_client
            .get_room(room_id)
            .await
            .map_err(|e| e.to_string())?;

        let _room = room_resp
            .room
            .ok_or_else(|| "Room not found".to_string())?;

        let join_resp = self
            .room_client
            .join_room(room_id, player_name, fingerprint, color.r as u32, color.g as u32, color.b as u32)
            .await
            .map_err(|e| e.to_string())?;

        if !join_resp.success {
            self.state = ConnectionState::Disconnected;
            return Err(join_resp.error_message);
        }

        // Get seed and status from room info
        let seed = join_resp.room.as_ref().map(|r| r.seed).unwrap_or(0);
        // Room status 2 = Playing (from proto)
        let is_game_in_progress = join_resp.room.as_ref().map(|r| r.status == 2).unwrap_or(false);
        let player_id = join_resp.player_id.clone();

        // Check if this player is the host
        let is_host = join_resp.room.as_ref()
            .and_then(|r| r.players.iter().find(|p| p.id == player_id))
            .map(|p| p.is_host)
            .unwrap_or(false);

        // Extract server players from response
        let server_players = extract_server_players(&join_resp);

        // Use the signaling_url from join response
        let signaling_url = join_resp.signaling_url.clone();
        self.signaling_url = Some(signaling_url.clone());
        self.connect_p2p(&signaling_url)?;

        self.state = ConnectionState::Connected {
            room_id: room_id.to_string(),
            player_id: join_resp.player_id,
        };

        Ok((seed, is_game_in_progress, player_id, is_host, server_players))
    }

    /// Start game on the server (host only).
    pub async fn start_game_on_server(&self, room_id: &str, player_id: &str) -> Result<(), String> {
        let resp = self
            .room_client
            .start_game(room_id, player_id)
            .await
            .map_err(|e| e.to_string())?;

        if resp.success {
            Ok(())
        } else {
            Err(resp.error_message)
        }
    }

    fn connect_p2p(&mut self, signaling_url: &str) -> Result<(), String> {
        let (socket, loop_fut) = WebRtcSocket::new_reliable(signaling_url);
        self.socket = Some(socket);

        // Spawn the socket loop
        wasm_bindgen_futures::spawn_local(async move {
            let _ = loop_fut.await;
        });

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.socket = None;
        self.peers.clear();
        self.signaling_url = None;
        self.state = ConnectionState::Disconnected;
    }

    pub fn broadcast(&mut self, data: &[u8]) {
        let Some(socket) = &mut self.socket else {
            return;
        };

        let peers: Vec<_> = socket.connected_peers().collect();
        let channel = socket.channel_mut(0);

        for peer_id in peers {
            channel.send(data.to_vec().into(), peer_id);
        }
    }

    pub fn send_to(&mut self, peer: PeerId, data: &[u8]) {
        let Some(socket) = &mut self.socket else {
            return;
        };

        let channel = socket.channel_mut(0);
        channel.send(data.to_vec().into(), peer);
    }

    /// Encode and broadcast a protobuf message
    pub fn broadcast_proto<M: Message>(&mut self, msg: &M) {
        let data = msg.encode_to_vec();
        self.broadcast(&data);
    }

    /// Encode and send a protobuf message to a specific peer
    pub fn send_proto_to<M: Message>(&mut self, peer: PeerId, msg: &M) {
        let data = msg.encode_to_vec();
        self.send_to(peer, &data);
    }

    /// Poll for network events. Call this in your game loop.
    pub fn poll(&mut self) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        let Some(socket) = &mut self.socket else {
            return events;
        };

        // Check for peer state changes
        for (peer_id, state) in socket.update_peers() {
            match state {
                PeerState::Connected => {
                    self.peers.insert(peer_id, state);
                    events.push(NetworkEvent::PeerJoined(peer_id));
                }
                PeerState::Disconnected => {
                    self.peers.remove(&peer_id);
                    events.push(NetworkEvent::PeerLeft(peer_id));
                }
            }
        }

        // Receive messages
        let channel = socket.channel_mut(0);
        for (peer_id, packet) in channel.receive() {
            events.push(NetworkEvent::Message {
                from: peer_id,
                data: packet.to_vec(),
            });
        }

        events
    }
}

/// Shared network manager for use across components
pub type SharedNetworkManager = Rc<RefCell<NetworkManager>>;

pub fn create_shared_network_manager(grpc_base_url: &str) -> SharedNetworkManager {
    Rc::new(RefCell::new(NetworkManager::new(grpc_base_url)))
}

/// Extract ServerPlayerInfo from a JoinRoomResponse.
fn extract_server_players(resp: &marble_proto::room::JoinRoomResponse) -> Vec<ServerPlayerInfo> {
    resp.room
        .as_ref()
        .map(|room| {
            room.players
                .iter()
                .map(|p| ServerPlayerInfo {
                    player_id: p.id.clone(),
                    name: p.name.clone(),
                    color: Color::rgb(p.color_r as u8, p.color_g as u8, p.color_b as u8),
                    is_host: p.is_host,
                    is_connected: p.is_connected,
                    join_order: p.join_order,
                })
                .collect()
        })
        .unwrap_or_default()
}
