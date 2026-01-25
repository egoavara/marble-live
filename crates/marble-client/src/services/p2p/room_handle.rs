//! P2P room handle - public API for P2P communication.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::GameState;
use marble_proto::play::p2p_message::Payload;
use marble_proto::play::{ChatMessage, FrameHash, GameStart, Ping, Reaction, SyncRequest, SyncState};
use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{PeerTopology, PlayerAuth, RegisterPeerIdRequest};
use matchbox_socket::{PeerId, WebRtcSocket};
use prost::Message;
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;
use yew::UseStateHandle;

use super::message_loop::{run_message_loop, MessageLoopCallbacks};
use super::room_state::P2pRoomState;
use super::types::{P2pConnectionState, P2pPeerInfo, P2pRoomConfig, ReceivedMessage};
use super::GossipHandler;

/// Handle for P2P room connection
///
/// Provides methods for connection control, message sending, and state queries.
/// State changes automatically trigger component re-renders.
#[derive(Clone)]
pub struct P2pRoomHandle {
    pub(crate) inner: Rc<RefCell<P2pRoomState>>,
    // Yew state handles for reactive updates (trigger re-render)
    pub(crate) state_handle: UseStateHandle<P2pConnectionState>,
    // Version counters to trigger re-renders when inner data changes
    pub(crate) peers_version: UseStateHandle<u32>,
    pub(crate) messages_version: UseStateHandle<u32>,
}

impl PartialEq for P2pRoomHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl P2pRoomHandle {
    // === State Queries (Reactive - auto re-render on change) ===

    /// Get current connection state
    pub fn state(&self) -> P2pConnectionState {
        (*self.state_handle).clone()
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        matches!(*self.state_handle, P2pConnectionState::Connected)
    }

    /// Get connected peers (reads from inner, triggers re-render via version)
    pub fn peers(&self) -> Vec<P2pPeerInfo> {
        let _ = *self.peers_version; // Create dependency for re-render
        self.inner.borrow().peers.clone()
    }

    /// Get message history (reads from inner, triggers re-render via version)
    pub fn messages(&self) -> Vec<ReceivedMessage> {
        let _ = *self.messages_version; // Create dependency for re-render
        self.inner.borrow().messages.clone()
    }

    /// Get my player ID
    pub fn my_player_id(&self) -> String {
        self.inner.borrow().player_id.clone()
    }

    /// Get current room ID
    pub fn room_id(&self) -> String {
        self.inner.borrow().room_id.clone()
    }

    /// Get topology info
    pub fn topology(&self) -> Option<PeerTopology> {
        self.inner.borrow().topology.clone()
    }

    /// Get player secret (for RPC authentication)
    pub fn player_secret(&self) -> Option<String> {
        self.inner.borrow().config.player_secret.clone()
    }

    // === Connection Control ===

    /// Connect to signaling server
    pub fn connect(&self) {
        let inner = self.inner.clone();
        let state_handle = self.state_handle.clone();
        let peers_version = self.peers_version.clone();
        let messages_version = self.messages_version.clone();

        spawn_local(async move {
            Self::do_connect(inner, state_handle, peers_version, messages_version).await;
        });
    }

    /// Connect with topology
    pub fn connect_with_topology(&self, topology: PeerTopology) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.topology = Some(topology);
        }
        self.connect();
    }

    /// Disconnect from P2P network
    pub fn disconnect(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.reset_connection();
        }

        self.state_handle.set(P2pConnectionState::Disconnected);
        self.peers_version.set(*self.peers_version + 1);
    }

    // === Message Sending ===

    /// Send raw payload (gossip relay)
    pub fn send(&self, payload: Payload) {
        let inner = self.inner.borrow();
        if let (Some(socket), Some(gossip)) = (&inner.socket, &inner.gossip) {
            let msg = {
                let mut gossip = gossip.borrow_mut();
                gossip.create_message(&inner.player_id, inner.config.gossip_ttl, payload)
            };

            let data = msg.encode_to_vec();
            let peers_to_send = gossip.borrow().get_all_peers();

            let mut socket = socket.borrow_mut();
            for peer in peers_to_send {
                socket
                    .channel_mut(0)
                    .send(data.clone().into_boxed_slice(), peer);
            }
        }
    }

    /// Send chat message (convenience method)
    pub fn send_chat(&self, content: &str) {
        let player_id = self.inner.borrow().player_id.clone();
        let timestamp_ms = js_sys::Date::now() as u64;

        self.send(Payload::ChatMessage(ChatMessage {
            player_id: player_id.clone(),
            content: content.to_string(),
            timestamp_ms,
        }));

        // Add to local messages
        let msg = ReceivedMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from_player: player_id,
            from_peer: None, // Local message
            payload: Payload::ChatMessage(ChatMessage {
                player_id: self.inner.borrow().player_id.clone(),
                content: content.to_string(),
                timestamp_ms,
            }),
            timestamp: js_sys::Date::now(),
        };

        self.inner.borrow_mut().add_message(msg);
        self.messages_version.set(*self.messages_version + 1);
    }

    /// Send reaction emoji (convenience method)
    pub fn send_reaction(&self, emoji: &str) {
        let player_id = self.inner.borrow().player_id.clone();
        let timestamp_ms = js_sys::Date::now() as u64;

        self.send(Payload::Reaction(Reaction {
            player_id: player_id.clone(),
            emoji: emoji.to_string(),
            timestamp_ms,
        }));

        // Add to local messages for display
        let msg = ReceivedMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from_player: player_id,
            from_peer: None, // Local message
            payload: Payload::Reaction(Reaction {
                player_id: self.inner.borrow().player_id.clone(),
                emoji: emoji.to_string(),
                timestamp_ms,
            }),
            timestamp: js_sys::Date::now(),
        };

        self.inner.borrow_mut().add_message(msg);
        self.messages_version.set(*self.messages_version + 1);
    }

    /// Send to specific peer (direct)
    pub fn send_to(&self, peer_id: PeerId, payload: Payload) {
        let inner = self.inner.borrow();
        if let (Some(socket), Some(gossip)) = (&inner.socket, &inner.gossip) {
            let msg = {
                let mut gossip = gossip.borrow_mut();
                gossip.create_message(&inner.player_id, 1, payload) // TTL 1 for direct
            };

            let data = msg.encode_to_vec();
            let mut socket = socket.borrow_mut();
            socket.channel_mut(0).send(data.into_boxed_slice(), peer_id);
        }
    }

    /// Send ping to all peers (RTT measurement)
    pub fn send_ping(&self) {
        let inner = self.inner.borrow();
        if let (Some(socket), Some(gossip)) = (&inner.socket, &inner.gossip) {
            let msg = {
                let mut gossip = gossip.borrow_mut();
                gossip.create_message(
                    &inner.player_id,
                    1, // TTL 1 for ping
                    Payload::Ping(Ping {
                        timestamp: js_sys::Date::now(),
                    }),
                )
            };

            let data = msg.encode_to_vec();
            let peers_to_send = gossip.borrow().get_all_peers();

            let mut socket = socket.borrow_mut();
            for peer in peers_to_send {
                socket
                    .channel_mut(0)
                    .send(data.clone().into_boxed_slice(), peer);
            }
        }
    }

    // === Message Queries ===

    /// Take new messages and remove from queue (consume pattern)
    pub fn take_new_messages(&self) -> Vec<ReceivedMessage> {
        let mut inner = self.inner.borrow_mut();
        std::mem::take(&mut inner.new_messages_queue)
    }

    /// Get last message
    pub fn last_message(&self) -> Option<ReceivedMessage> {
        let _ = *self.messages_version;
        self.inner.borrow().messages.last().cloned()
    }

    /// Filter messages by type
    pub fn messages_of_type<F>(&self, filter: F) -> Vec<ReceivedMessage>
    where
        F: Fn(&Payload) -> bool,
    {
        let _ = *self.messages_version;
        self.inner
            .borrow()
            .messages
            .iter()
            .filter(|m| filter(&m.payload))
            .cloned()
            .collect()
    }

    /// Get chat messages only
    pub fn chat_messages(&self) -> Vec<ReceivedMessage> {
        self.messages_of_type(|p| matches!(p, Payload::ChatMessage(_)))
    }

    /// Get reaction messages only
    pub fn reaction_messages(&self) -> Vec<ReceivedMessage> {
        self.messages_of_type(|p| matches!(p, Payload::Reaction(_)))
    }

    /// Clear message history
    pub fn clear_messages(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.messages.clear();
            inner.new_messages_queue.clear();
        }
        self.messages_version.set(*self.messages_version + 1);
    }

    // === Game Synchronization API ===

    /// Set host status
    pub fn set_host_status(&self, is_host: bool) {
        self.inner.borrow_mut().set_host_status(is_host);
    }

    /// Check if this client is the host
    pub fn is_host(&self) -> bool {
        self.inner.borrow().is_host
    }

    /// Set host peer ID
    pub fn set_host_peer_id(&self, peer_id: Option<PeerId>) {
        self.inner.borrow_mut().set_host_peer_id(peer_id);
    }

    /// Get host peer ID
    pub fn host_peer_id(&self) -> Option<PeerId> {
        self.inner.borrow().host_peer_id
    }

    /// Set game state reference
    pub fn set_game_state(&self, state: Rc<RefCell<GameState>>) {
        self.inner.borrow_mut().set_game_state(state);
    }

    /// Get game state reference
    pub fn game_state(&self) -> Option<Rc<RefCell<GameState>>> {
        self.inner.borrow().game_state.clone()
    }

    /// Broadcast frame hash to all peers (host only)
    pub fn send_frame_hash(&self, frame: u64, hash: u64) {
        self.send(Payload::FrameHash(FrameHash { frame, hash }));
        self.inner.borrow_mut().last_hash_frame = frame;
    }

    /// Send sync request to host
    pub fn send_sync_request(&self, from_frame: u64) {
        if let Some(host_peer_id) = self.inner.borrow().host_peer_id {
            self.send_to(host_peer_id, Payload::SyncRequest(SyncRequest { from_frame }));
        }
    }

    /// Send sync state to a specific peer (host only)
    pub fn send_sync_state_to(&self, peer_id: PeerId, frame: u64, state: Vec<u8>) {
        self.send_to(peer_id, Payload::SyncState(SyncState { frame, state }));
    }

    /// Broadcast game start to all peers (host only)
    pub fn send_game_start(&self, seed: u64, initial_state: Vec<u8>, gamerule: String) {
        // Increment session version
        let session_version = {
            let mut inner = self.inner.borrow_mut();
            inner.current_session_version += 1;
            inner.current_session_version
        };

        self.send(Payload::GameStart(GameStart {
            seed,
            initial_state,
            gamerule,
            session_version,
        }));
    }

    /// Get current session version
    pub fn current_session_version(&self) -> u64 {
        self.inner.borrow().current_session_version
    }

    /// Get last hash frame
    pub fn last_hash_frame(&self) -> u64 {
        self.inner.borrow().last_hash_frame
    }

    /// Get desync count
    pub fn desync_count(&self) -> u32 {
        self.inner.borrow().desync_count
    }

    /// Increment desync count
    pub fn increment_desync_count(&self) {
        self.inner.borrow_mut().desync_count += 1;
    }

    /// Reset desync count
    pub fn reset_desync_count(&self) {
        self.inner.borrow_mut().desync_count = 0;
    }

    /// Set last sync frame
    pub fn set_last_sync_frame(&self, frame: u64) {
        self.inner.borrow_mut().last_sync_frame = frame;
    }

    /// Get last sync frame
    pub fn last_sync_frame(&self) -> u64 {
        self.inner.borrow().last_sync_frame
    }

    /// Set last host hash
    pub fn set_last_host_hash(&self, frame: u64, hash: u64) {
        self.inner.borrow_mut().last_host_hash = Some((frame, hash));
    }

    /// Get last host hash
    pub fn last_host_hash(&self) -> Option<(u64, u64)> {
        self.inner.borrow().last_host_hash
    }

    // === Internal Methods ===

    pub(crate) async fn do_connect(
        inner: Rc<RefCell<P2pRoomState>>,
        state_handle: UseStateHandle<P2pConnectionState>,
        peers_version: UseStateHandle<u32>,
        messages_version: UseStateHandle<u32>,
    ) {
        let room_id = inner.borrow().room_id.clone();
        if room_id.is_empty() {
            state_handle.set(P2pConnectionState::Error("No room ID set".to_string()));
            return;
        }

        // Build signaling URL
        let signaling_url = inner
            .borrow()
            .config
            .signaling_url
            .clone()
            .unwrap_or_else(|| format!("ws://localhost:3000/signaling/{}", room_id));

        state_handle.set(P2pConnectionState::Connecting);

        // Create WebRTC socket
        let (socket, loop_fut) = WebRtcSocket::new_reliable(&signaling_url);
        let socket = Rc::new(RefCell::new(socket));

        // Initialize gossip handler
        let topology = inner.borrow().topology.clone();
        let topo = topology.unwrap_or(PeerTopology {
            mesh_group: 0,
            is_bridge: false,
            connect_to: vec![],
            bridge_peers: vec![],
        });

        let gossip = Rc::new(RefCell::new(GossipHandler::new(
            topo.mesh_group,
            topo.is_bridge,
        )));

        // Store references
        {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.socket = Some(socket.clone());
            inner_mut.gossip = Some(gossip.clone());
            inner_mut.is_running = true;
        }

        state_handle.set(P2pConnectionState::Connected);

        // Register peer_id with server if credentials are available
        {
            let inner_ref = inner.borrow();
            if let Some(player_secret) = &inner_ref.config.player_secret {
                let room_id = inner_ref.room_id.clone();
                let player_id = inner_ref.player_id.clone();
                let player_secret = player_secret.clone();
                let socket_for_id = socket.clone();

                // Spawn task to register peer_id after getting it from socket
                spawn_local(async move {
                    // Wait for socket to get its ID (poll until available)
                    let mut attempts = 0;
                    let peer_id = loop {
                        if let Some(id) = socket_for_id.borrow_mut().id() {
                            break id.to_string();
                        }
                        attempts += 1;
                        if attempts > 100 {
                            tracing::warn!("Failed to get peer_id from socket after 100 attempts");
                            return;
                        }
                        gloo::timers::future::TimeoutFuture::new(50).await;
                    };

                    // Create gRPC client and register peer_id
                    let Some(window) = web_sys::window() else {
                        tracing::warn!("No window object available for RegisterPeerId");
                        return;
                    };
                    let Ok(origin) = window.location().origin() else {
                        tracing::warn!("Failed to get origin for RegisterPeerId");
                        return;
                    };
                    let client = Client::new(format!("{}/grpc", origin));
                    let mut grpc = RoomServiceClient::new(client);

                    let req = RegisterPeerIdRequest {
                        room_id,
                        player: Some(PlayerAuth {
                            id: player_id.clone(),
                            secret: player_secret,
                        }),
                        peer_id: peer_id.clone(),
                    };

                    match grpc.register_peer_id(req).await {
                        Ok(resp) => {
                            let resp = resp.into_inner();
                            if resp.success {
                                tracing::info!(
                                    player_id = %player_id,
                                    peer_id = %peer_id,
                                    "Successfully registered peer_id with server"
                                );
                            } else {
                                tracing::warn!(
                                    player_id = %player_id,
                                    peer_id = %peer_id,
                                    "Server rejected peer_id registration"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                player_id = %player_id,
                                peer_id = %peer_id,
                                error = %e,
                                "Failed to register peer_id with server"
                            );
                        }
                    }
                });
            }
        }

        // Spawn signaling loop
        let state_handle_for_signaling = state_handle.clone();
        spawn_local(async move {
            if let Err(e) = loop_fut.await {
                state_handle_for_signaling
                    .set(P2pConnectionState::Error(format!("Signaling error: {:?}", e)));
            }
        });

        // Spawn message handling loop
        let peers_version_clone = peers_version.clone();
        let messages_version_clone = messages_version.clone();

        let callbacks = MessageLoopCallbacks {
            on_peers_changed: Box::new(move || {
                peers_version_clone.set(*peers_version_clone + 1);
            }),
            on_messages_changed: Box::new(move || {
                messages_version_clone.set(*messages_version_clone + 1);
            }),
        };

        spawn_local(async move {
            run_message_loop(inner, gossip, callbacks).await;
        });
    }
}
