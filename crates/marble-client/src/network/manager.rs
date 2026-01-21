//! Network manager for P2P game synchronization

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

    pub fn room_client(&self) -> &RoomClient {
        &self.room_client
    }

    pub async fn create_and_join_room(
        &mut self,
        room_name: &str,
        player_name: &str,
    ) -> Result<String, String> {
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
            .join_room(&room_id, player_name)
            .await
            .map_err(|e| e.to_string())?;

        if !join_resp.success {
            self.state = ConnectionState::Disconnected;
            return Err(join_resp.error_message);
        }

        // Connect to signaling server for P2P
        self.signaling_url = Some(signaling_url.clone());
        self.connect_p2p(&signaling_url)?;

        self.state = ConnectionState::Connected {
            room_id: room_id.clone(),
            player_id: join_resp.player_id,
        };

        Ok(room_id)
    }

    pub async fn join_room(&mut self, room_id: &str, player_name: &str) -> Result<(), String> {
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
            .join_room(room_id, player_name)
            .await
            .map_err(|e| e.to_string())?;

        if !join_resp.success {
            self.state = ConnectionState::Disconnected;
            return Err(join_resp.error_message);
        }

        // Use the signaling_url from join response
        let signaling_url = join_resp.signaling_url.clone();
        self.signaling_url = Some(signaling_url.clone());
        self.connect_p2p(&signaling_url)?;

        self.state = ConnectionState::Connected {
            room_id: room_id.to_string(),
            player_id: join_resp.player_id,
        };

        Ok(())
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
