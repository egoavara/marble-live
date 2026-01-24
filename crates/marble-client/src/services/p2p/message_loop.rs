//! P2P message handling loop.

use std::cell::RefCell;
use std::rc::Rc;

use marble_proto::play::p2p_message::Payload;
use marble_proto::play::{P2pMessage, Pong};
use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{PlayerAuth, ResolvePeerIdsRequest};
use matchbox_socket::PeerState;
use prost::Message;
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;

use super::game_sync;
use super::room_state::P2pRoomState;
use super::types::ReceivedMessage;
use super::GossipHandler;

/// Context for the message loop containing callbacks for state updates
pub struct MessageLoopCallbacks {
    pub on_peers_changed: Box<dyn Fn()>,
    pub on_messages_changed: Box<dyn Fn()>,
}

/// Process peer updates from socket
pub fn handle_peer_updates(
    state: &Rc<RefCell<P2pRoomState>>,
    gossip: &Rc<RefCell<GossipHandler>>,
    callbacks: &MessageLoopCallbacks,
) {
    let socket = {
        let state_ref = state.borrow();
        state_ref.socket.clone()
    };

    if let Some(socket) = socket {
        let peer_updates = {
            let mut socket = socket.borrow_mut();
            socket.update_peers()
        };

        let mut peers_changed = false;
        let mut new_peer_ids: Vec<matchbox_socket::PeerId> = Vec::new();

        for (peer_id, peer_state) in peer_updates {
            match peer_state {
                PeerState::Connected => {
                    state.borrow_mut().add_peer(peer_id);
                    new_peer_ids.push(peer_id);
                    peers_changed = true;
                }
                PeerState::Disconnected => {
                    state.borrow_mut().remove_peer(peer_id);
                    peers_changed = true;
                }
            }
        }

        if peers_changed {
            // Update gossip handler with current peers
            let peer_ids = state.borrow().get_peer_ids();
            gossip.borrow_mut().set_peers(peer_ids, vec![]);
            // Trigger re-render
            (callbacks.on_peers_changed)();
        }

        // Resolve new peer_ids to player_ids via server API
        if !new_peer_ids.is_empty() {
            resolve_peer_ids_from_server(state.clone(), new_peer_ids);
        }
    }
}

/// Resolve peer_ids to player_ids via server API
fn resolve_peer_ids_from_server(
    state: Rc<RefCell<P2pRoomState>>,
    peer_ids: Vec<matchbox_socket::PeerId>,
) {
    let (room_id, player_id, player_secret) = {
        let state_ref = state.borrow();
        (
            state_ref.room_id.clone(),
            state_ref.player_id.clone(),
            state_ref.config.player_secret.clone(),
        )
    };

    let Some(player_secret) = player_secret else {
        tracing::warn!("Cannot resolve peer_ids: no player_secret configured");
        return;
    };

    // Convert PeerIds to strings
    let peer_id_strings: Vec<String> = peer_ids.iter().map(|p| p.to_string()).collect();

    // Clone callback trigger for use in async task
    // We need to use a channel or similar mechanism, but for simplicity
    // we'll trigger the callback after the state is updated
    let state_clone = state.clone();

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
                let mut state_mut = state_clone.borrow_mut();

                for (peer_id_str, player_id) in resp.peer_to_player {
                    if let Ok(uuid) = uuid::Uuid::parse_str(&peer_id_str) {
                        let peer_id = matchbox_socket::PeerId::from(uuid);
                        state_mut.update_peer_player_id(peer_id, player_id);
                    }
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

/// Process received messages from socket
pub fn handle_received_messages(
    state: &Rc<RefCell<P2pRoomState>>,
    gossip: &Rc<RefCell<GossipHandler>>,
    callbacks: &MessageLoopCallbacks,
) {
    let (socket, store_ping_pong, player_id) = {
        let state_ref = state.borrow();
        (
            state_ref.socket.clone(),
            state_ref.config.store_ping_pong,
            state_ref.player_id.clone(),
        )
    };

    if let Some(socket) = socket {
        let received = {
            let mut socket = socket.borrow_mut();
            socket.channel_mut(0).receive()
        };

        let mut messages_changed = false;
        let mut peers_changed = false;

        for (peer_id, data) in received {
            if let Ok(msg) = P2pMessage::decode(&*data) {
                let (should_process, relay_targets) = {
                    let mut gossip = gossip.borrow_mut();
                    gossip.handle_incoming(&msg, peer_id)
                };

                if should_process {
                    if let Some(payload) = &msg.payload {
                        let (msg_changed, peer_changed) = process_payload(
                            state,
                            gossip,
                            &socket,
                            peer_id,
                            &msg,
                            payload,
                            store_ping_pong,
                            &player_id,
                        );
                        messages_changed |= msg_changed;
                        peers_changed |= peer_changed;
                    }
                }

                // Relay if needed
                if !relay_targets.is_empty() {
                    let relay_msg = gossip.borrow().prepare_for_relay(&msg);
                    let relay_data = relay_msg.encode_to_vec();
                    let mut socket_inner = socket.borrow_mut();
                    for target in relay_targets {
                        socket_inner
                            .channel_mut(0)
                            .send(relay_data.clone().into_boxed_slice(), target);
                    }
                }
            }
        }

        // Trigger re-renders if needed
        if messages_changed {
            (callbacks.on_messages_changed)();
        }
        if peers_changed {
            (callbacks.on_peers_changed)();
        }
    }
}

/// Process a single payload based on its type
/// Returns (messages_changed, peers_changed)
fn process_payload(
    state: &Rc<RefCell<P2pRoomState>>,
    gossip: &Rc<RefCell<GossipHandler>>,
    socket: &Rc<RefCell<matchbox_socket::WebRtcSocket>>,
    peer_id: matchbox_socket::PeerId,
    msg: &P2pMessage,
    payload: &Payload,
    store_ping_pong: bool,
    player_id: &str,
) -> (bool, bool) {
    let mut messages_changed = false;
    let mut peers_changed = false;

    match payload {
        Payload::ChatMessage(_) => {
            let received_msg = ReceivedMessage {
                id: msg.message_id.clone(),
                from_player: msg.origin_player.clone(),
                from_peer: Some(peer_id),
                payload: payload.clone(),
                timestamp: js_sys::Date::now(),
            };
            state.borrow_mut().add_message(received_msg);
            messages_changed = true;
        }
        Payload::Ping(ping) => {
            // Reply with pong
            let pong = {
                let mut gossip = gossip.borrow_mut();
                gossip.create_message(
                    player_id,
                    1,
                    Payload::Pong(Pong {
                        timestamp: ping.timestamp,
                    }),
                )
            };
            let pong_data = pong.encode_to_vec();
            let mut socket_inner = socket.borrow_mut();
            socket_inner
                .channel_mut(0)
                .send(pong_data.into_boxed_slice(), peer_id);

            if store_ping_pong {
                let received_msg = ReceivedMessage {
                    id: msg.message_id.clone(),
                    from_player: msg.origin_player.clone(),
                    from_peer: Some(peer_id),
                    payload: payload.clone(),
                    timestamp: js_sys::Date::now(),
                };
                state.borrow_mut().add_message(received_msg);
                messages_changed = true;
            }
        }
        Payload::Pong(pong) => {
            let now = js_sys::Date::now();
            let rtt = (now - pong.timestamp) as u32;

            // Update RTT in state
            state.borrow_mut().update_peer_rtt(peer_id, rtt);
            peers_changed = true;

            if store_ping_pong {
                let received_msg = ReceivedMessage {
                    id: msg.message_id.clone(),
                    from_player: msg.origin_player.clone(),
                    from_peer: Some(peer_id),
                    payload: payload.clone(),
                    timestamp: js_sys::Date::now(),
                };
                state.borrow_mut().add_message(received_msg);
                messages_changed = true;
            }
        }
        // === Game synchronization messages ===
        Payload::FrameHash(hash) => {
            let should_request_sync = game_sync::handle_frame_hash(state, hash);
            if should_request_sync {
                // Request sync from host
                if let Some(host_peer_id) = state.borrow().host_peer_id {
                    let sync_req = {
                        let mut gossip = gossip.borrow_mut();
                        gossip.create_message(
                            player_id,
                            1,
                            Payload::SyncRequest(marble_proto::play::SyncRequest {
                                from_frame: hash.frame,
                            }),
                        )
                    };
                    let data = sync_req.encode_to_vec();
                    let mut socket_inner = socket.borrow_mut();
                    socket_inner.channel_mut(0).send(data.into_boxed_slice(), host_peer_id);

                    state.borrow_mut().last_sync_frame = hash.frame;
                    tracing::info!(frame = hash.frame, "Sent sync request to host");
                }
            }
            // Don't store in message history
        }
        Payload::SyncRequest(request) => {
            // Only host processes sync requests
            if state.borrow().is_host {
                game_sync::handle_sync_request(state, socket, gossip, peer_id, request);
            }
            // Don't store in message history
        }
        Payload::SyncState(sync_state) => {
            game_sync::handle_sync_state(state, sync_state);
            // Don't store in message history
        }
        Payload::GameStart(game_start) => {
            // Store in message history so game loop can pick it up
            let received_msg = ReceivedMessage {
                id: msg.message_id.clone(),
                from_player: msg.origin_player.clone(),
                from_peer: Some(peer_id),
                payload: payload.clone(),
                timestamp: js_sys::Date::now(),
            };
            state.borrow_mut().add_message(received_msg);
            messages_changed = true;

            tracing::info!(
                seed = game_start.seed,
                "Received GameStart message"
            );
        }
        _ => {
            // Store other message types
            let received_msg = ReceivedMessage {
                id: msg.message_id.clone(),
                from_player: msg.origin_player.clone(),
                from_peer: Some(peer_id),
                payload: payload.clone(),
                timestamp: js_sys::Date::now(),
            };
            state.borrow_mut().add_message(received_msg);
            messages_changed = true;
        }
    }

    (messages_changed, peers_changed)
}

/// Run the main message loop
pub async fn run_message_loop(
    state: Rc<RefCell<P2pRoomState>>,
    gossip: Rc<RefCell<GossipHandler>>,
    callbacks: MessageLoopCallbacks,
) {
    loop {
        // Check if we should stop
        if !state.borrow().is_running {
            break;
        }

        // Handle peer updates
        handle_peer_updates(&state, &gossip, &callbacks);

        // Handle received messages
        handle_received_messages(&state, &gossip, &callbacks);

        // Check for async peer updates (from ResolvePeerIds)
        {
            let mut state_mut = state.borrow_mut();
            if state_mut.peers_dirty {
                state_mut.peers_dirty = false;
                drop(state_mut);
                (callbacks.on_peers_changed)();
            }
        }

        // Yield to other tasks
        gloo::timers::future::TimeoutFuture::new(16).await;
    }
}
