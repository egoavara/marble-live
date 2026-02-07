//! P2P room handle - public API for P2P communication.
//!
//! After refactor: P2P socket lives in Bevy (marble-core).
//! Chat/reaction/ping are sent via `send_command()` → Bevy P2P system.
//! Game sync (FrameHash, SyncRequest, SyncState, GameStart) is handled entirely in Bevy.

use std::cell::RefCell;
use std::rc::Rc;

use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{PeerTopology, PlayerAuth, RegisterPeerIdRequest, ResolvePeerIdsRequest};
use matchbox_socket::PeerId;
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;
use yew::UseStateHandle;

use super::room_state::P2pRoomState;
use super::types::{P2pConnectionState, P2pPeerInfo, ReceivedMessage};

/// Handle for P2P room connection
///
/// Provides methods for connection control, message sending, and state queries.
/// State changes automatically trigger component re-renders.
///
/// After refactor: The WebRTC socket is managed by Bevy (marble-core).
/// This handle delegates P2P messaging to Bevy via `send_command()`.
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

/// Send a command to Bevy via the global WASM function.
fn bevy_send_command(json: &str) {
    if let Err(e) = marble_core::bevy::wasm_entry::send_command(json) {
        tracing::error!("Failed to send command to Bevy: {:?}", e);
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

        // Tell Bevy to disconnect P2P socket
        marble_core::bevy::wasm_entry::disconnect_p2p();

        self.state_handle.set(P2pConnectionState::Disconnected);
        self.peers_version.set(*self.peers_version + 1);
    }

    // === Message Sending (via Bevy send_command) ===

    /// Send chat message (via Bevy P2P system)
    pub fn send_chat(&self, content: &str) {
        let cmd = serde_json::json!({
            "type": "send_chat",
            "content": content
        });
        bevy_send_command(&cmd.to_string());
    }

    /// Send reaction emoji (via Bevy P2P system)
    pub fn send_reaction(&self, emoji: &str) {
        let cmd = serde_json::json!({
            "type": "send_reaction",
            "emoji": emoji
        });
        bevy_send_command(&cmd.to_string());
    }

    /// Send ping to all peers (via Bevy P2P system)
    pub fn send_ping(&self) {
        bevy_send_command(r#"{"type":"send_ping"}"#);
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
        F: Fn(&marble_proto::play::p2p_message::Payload) -> bool,
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
        self.messages_of_type(|p| {
            matches!(p, marble_proto::play::p2p_message::Payload::ChatMessage(_))
        })
    }

    /// Get reaction messages only
    pub fn reaction_messages(&self) -> Vec<ReceivedMessage> {
        self.messages_of_type(|p| {
            matches!(p, marble_proto::play::p2p_message::Payload::Reaction(_))
        })
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

    // === Game Synchronization API (simplified - Bevy handles the heavy lifting) ===

    /// Set host status
    pub fn set_host_status(&self, is_host: bool) {
        self.inner.borrow_mut().is_host = is_host;
    }

    /// Check if this client is the host
    pub fn is_host(&self) -> bool {
        self.inner.borrow().is_host
    }

    /// Set host peer ID
    pub fn set_host_peer_id(&self, peer_id: Option<PeerId>) {
        self.inner.borrow_mut().host_peer_id = peer_id;
    }

    /// Get host peer ID
    pub fn host_peer_id(&self) -> Option<PeerId> {
        self.inner.borrow().host_peer_id
    }

    // === Internal Methods ===

    pub(crate) async fn do_connect(
        inner: Rc<RefCell<P2pRoomState>>,
        state_handle: UseStateHandle<P2pConnectionState>,
        peers_version: UseStateHandle<u32>,
        _messages_version: UseStateHandle<u32>,
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

        // Get topology info
        let topology = inner.borrow().topology.clone();
        let topo = topology.unwrap_or(PeerTopology {
            mesh_group: 0,
            is_bridge: false,
            connect_to: vec![],
            bridge_peers: vec![],
        });

        let player_id = inner.borrow().player_id.clone();
        let is_host = inner.borrow().is_host;

        // Initialize P2P socket in Bevy instead of creating it here
        marble_core::bevy::wasm_entry::init_p2p_socket(
            &signaling_url,
            topo.mesh_group,
            topo.is_bridge,
            &player_id,
            is_host,
        );

        // Mark as running and connected
        {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.is_running = true;
        }

        state_handle.set(P2pConnectionState::Connected);

        // Register peer_id with server after Bevy socket gets its ID from signaling.
        {
            let inner_ref = inner.borrow();
            if let Some(player_secret) = &inner_ref.config.player_secret {
                let room_id = inner_ref.room_id.clone();
                let player_id = inner_ref.player_id.clone();
                let player_secret = player_secret.clone();

                spawn_local(async move {
                    // Poll until Bevy exposes this socket's peer_id
                    let my_peer_id = loop {
                        let id = marble_core::bevy::wasm_entry::get_my_peer_id();
                        if !id.is_empty() {
                            break id;
                        }
                        gloo::timers::future::TimeoutFuture::new(100).await;
                    };

                    tracing::info!(peer_id = %my_peer_id, "Registering peer_id with server");

                    // Call RegisterPeerId gRPC
                    let Some(window) = web_sys::window() else {
                        return;
                    };
                    let Ok(origin) = window.location().origin() else {
                        return;
                    };
                    let client = Client::new(format!("{}/grpc", origin));
                    let mut grpc = RoomServiceClient::new(client);

                    let req = RegisterPeerIdRequest {
                        room_id: room_id.clone(),
                        player: Some(PlayerAuth {
                            id: player_id.clone(),
                            secret: player_secret,
                        }),
                        peer_id: my_peer_id.clone(),
                    };

                    match grpc.register_peer_id(req).await {
                        Ok(_) => {
                            tracing::info!(
                                peer_id = %my_peer_id,
                                player_id = %player_id,
                                "Successfully registered peer_id with server"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to register peer_id with server");
                        }
                    }
                });
            }
        }

        // Start polling loop for peer updates from Bevy StateStore
        let peers_version_clone = peers_version.clone();
        let inner_for_poll = inner.clone();
        spawn_local(async move {
            let mut last_version: u64 = 0;
            let mut resolve_retry_tick: u32 = 0;

            loop {
                // Check if we should stop
                if !inner_for_poll.borrow().is_running {
                    break;
                }

                // Poll Bevy's peer store for updates
                let current_version = marble_core::bevy::wasm_entry::get_peers_version();
                let version_changed = current_version != last_version;

                // Retry unresolved peers every ~2 seconds (20 ticks × 100ms)
                resolve_retry_tick += 1;
                let should_retry_resolve = resolve_retry_tick >= 20;
                if should_retry_resolve {
                    resolve_retry_tick = 0;
                }

                if version_changed || should_retry_resolve {
                    if version_changed {
                        last_version = current_version;
                    }

                    // Get peers from Bevy store and update local state
                    let peers_js = marble_core::bevy::wasm_entry::get_peers();
                    if let Ok(bevy_peers) = serde_wasm_bindgen::from_value::<
                        Vec<marble_core::bevy::state_store::PeerInfo>,
                    >(peers_js)
                    {
                        // Collect peer_ids that need player_id resolution
                        let unresolved_peer_ids: Vec<String> = bevy_peers
                            .iter()
                            .filter(|bp| bp.player_id.is_none())
                            .map(|bp| bp.peer_id.clone())
                            .collect();

                        let mut inner_mut = inner_for_poll.borrow_mut();
                        inner_mut.peers = bevy_peers
                            .iter()
                            .map(|bp| {
                                let peer_id = uuid::Uuid::parse_str(&bp.peer_id)
                                    .map(PeerId::from)
                                    .unwrap_or_else(|_| PeerId::from(uuid::Uuid::nil()));
                                P2pPeerInfo {
                                    peer_id,
                                    player_id: bp.player_id.clone(),
                                    connected: true,
                                    rtt_ms: None,
                                }
                            })
                            .collect();

                        // Resolve unresolved peers via gRPC
                        if !unresolved_peer_ids.is_empty() {
                            if let Some(player_secret) = &inner_mut.config.player_secret {
                                let room_id = inner_mut.room_id.clone();
                                let player_id = inner_mut.player_id.clone();
                                let player_secret = player_secret.clone();
                                let peer_ids = unresolved_peer_ids;

                                drop(inner_mut);

                                resolve_peer_ids(room_id, player_id, player_secret, peer_ids);
                            } else {
                                drop(inner_mut);
                            }
                        } else {
                            drop(inner_mut);
                            // All peers resolved, stop retry timer
                            resolve_retry_tick = 0;
                        }

                        if version_changed {
                            peers_version_clone.set(*peers_version_clone + 1);
                        }
                    }
                }

                // Yield to other tasks
                gloo::timers::future::TimeoutFuture::new(100).await;
            }
        });
    }
}

/// Resolve peer_ids to player_ids via server gRPC and forward results to Bevy.
fn resolve_peer_ids(
    room_id: String,
    player_id: String,
    player_secret: String,
    peer_id_strings: Vec<String>,
) {
    if peer_id_strings.is_empty() {
        return;
    }

    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            tracing::warn!("No window object available for ResolvePeerIds");
            return;
        };
        let Ok(origin) = window.location().origin() else {
            tracing::warn!("Failed to get origin for ResolvePeerIds");
            return;
        };

        let client = Client::new(format!("{}/grpc", origin));
        let mut grpc = RoomServiceClient::new(client);

        let req = ResolvePeerIdsRequest {
            room_id,
            player: Some(PlayerAuth {
                id: player_id,
                secret: player_secret,
            }),
            peer_ids: peer_id_strings.clone(),
        };

        match grpc.resolve_peer_ids(req).await {
            Ok(resp) => {
                let resp = resp.into_inner();
                let resolved_count = resp.peer_to_player.len();

                for (peer_id_str, player_id) in resp.peer_to_player {
                    // Forward to Bevy so P2pSocketRes gets updated
                    marble_core::bevy::wasm_entry::update_peer_player_id(&peer_id_str, &player_id);
                }

                tracing::debug!(
                    resolved = resolved_count,
                    requested = peer_id_strings.len(),
                    "Resolved peer_ids from server"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to resolve peer_ids from server");
            }
        }
    });
}
