//! P2P synchronization systems for Bevy.
//!
//! These systems handle the complete P2P game sync lifecycle:
//! - Socket lifecycle (pickup from WASM, disconnect)
//! - Message polling, dispatch, and gossip relay
//! - Frame hash broadcasting (host → peers)
//! - Desync detection (peer)
//! - Sync snapshot request/response
//! - Game start broadcasting (host → peers)

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use matchbox_socket::PeerId;
use prost::Message as ProstMessage;
use rapier2d::prelude::*;

use marble_proto::play::p2p_message::Payload;
use marble_proto::play::{FrameHash, P2pMessage, Ping, Pong};

use crate::bevy::gossip::GossipHandler;
use crate::bevy::p2p_socket::P2pSocketRes;
use crate::bevy::rapier_plugin::{
    PhysicsBody, PhysicsExternalForce, PhysicsWorldRes, USER_DATA_MARBLE, encode_user_data,
};
use crate::bevy::sync_snapshot::{BevySyncSnapshot, MapObjectTransformSnapshot, MarbleSnapshot};
use crate::bevy::wasm_entry::{take_p2p_disconnect, take_pending_p2p, take_pending_peer_updates};
use crate::bevy::{
    BroadcastGameStartEvent, CommandQueue, DeterministicRng, GameCommand, GameContextRes,
    KeyframeExecutors, KeyframeTarget, Marble, MarbleGameState, MarbleVisual, StateStores,
    SyncSnapshotRequestEvent, SyncState,
};

/// Hash broadcast interval in frames (0.5 seconds at 60 FPS).
const HASH_BROADCAST_INTERVAL: u64 = 30;

/// Sync cooldown in frames (3 seconds at 60 FPS).
const SYNC_COOLDOWN: u64 = 180;

// ============================================================================
// Socket Lifecycle Systems
// ============================================================================

/// Picks up a pending P2P socket from the WASM global slot and inserts it as a Resource.
pub fn pickup_pending_p2p(mut commands: Commands) {
    if let Some(pending) = take_pending_p2p() {
        tracing::info!(
            "[p2p] Picking up pending P2P socket: player={}, host={}",
            pending.player_id,
            pending.is_host
        );

        let gossip = GossipHandler::new(pending.mesh_group, pending.is_bridge);

        commands.insert_resource(P2pSocketRes {
            socket: crate::bevy::p2p_socket::P2pSocketWrapper(pending.socket),
            player_id: pending.player_id,
            is_host: pending.is_host,
            host_peer_id: None,
            connected_peers: Vec::new(),
            peer_player_map: std::collections::HashMap::new(),
        });
        commands.insert_resource(gossip);
    }
}

/// Handles P2P disconnect requests by removing the socket Resource.
pub fn handle_p2p_disconnect(mut commands: Commands) {
    if take_p2p_disconnect() {
        tracing::info!("[p2p] Disconnecting P2P socket");
        commands.remove_resource::<P2pSocketRes>();
        commands.remove_resource::<GossipHandler>();
    }
}

// ============================================================================
// Message Polling System
// ============================================================================

/// Polls the P2P socket for incoming messages and peer updates.
///
/// This is the main message loop, replacing `run_message_loop` from marble-client.
#[allow(clippy::too_many_arguments)]
pub fn poll_p2p_socket(
    mut socket_res: Option<ResMut<P2pSocketRes>>,
    mut gossip: Option<ResMut<GossipHandler>>,
    mut sync_state: ResMut<SyncState>,
    command_queue: Res<CommandQueue>,
    state_stores: Res<StateStores>,
    mut sync_request_events: MessageWriter<SyncSnapshotRequestEvent>,
) {
    let Some(socket_res) = socket_res.as_mut() else {
        return;
    };
    let Some(gossip) = gossip.as_mut() else {
        return;
    };

    // Expose this socket's own peer_id to StateStore (for Yew to use in RegisterPeerId)
    if state_stores.peers.get_my_peer_id().is_none() {
        if let Some(my_id) = socket_res.socket.0.id() {
            state_stores.peers.set_my_peer_id(my_id.to_string());
            tracing::info!("[p2p] My peer_id: {}", my_id);
        }
    }

    // Apply pending peer_id → player_id updates from Yew
    let pending_updates = take_pending_peer_updates();
    let had_updates = !pending_updates.is_empty();
    for (peer_id_str, player_id) in pending_updates {
        if let Ok(uuid) = uuid::Uuid::parse_str(&peer_id_str) {
            let peer_id = PeerId::from(uuid);
            socket_res.peer_player_map.insert(peer_id, player_id);
        }
    }

    // 1. Handle peer updates
    let peer_updates = socket_res.socket.0.update_peers();
    let mut peers_changed = false;

    for (peer_id, peer_state) in peer_updates {
        match peer_state {
            matchbox_socket::PeerState::Connected => {
                if !socket_res.connected_peers.contains(&peer_id) {
                    socket_res.connected_peers.push(peer_id);
                    peers_changed = true;
                    tracing::info!("[p2p] Peer connected: {}", peer_id);

                    // Host: auto-send sync snapshot to newly connected peer
                    // so they can align keyframe animations during lobby
                    if sync_state.is_host {
                        sync_request_events.write(SyncSnapshotRequestEvent {
                            peer_id_bytes: peer_id.0.as_bytes().to_vec(),
                            from_frame: 0,
                        });
                        tracing::info!("[p2p] Queued auto-sync snapshot for new peer {}", peer_id);
                    }
                }
            }
            matchbox_socket::PeerState::Disconnected => {
                socket_res.connected_peers.retain(|p| *p != peer_id);
                peers_changed = true;
                tracing::info!("[p2p] Peer disconnected: {}", peer_id);
            }
        }
    }

    if peers_changed {
        // Update gossip handler with current peers
        gossip.set_peers(socket_res.connected_peers.clone(), vec![]);
    }

    // Update StateStore peers when peers changed OR player_id mappings were updated
    if peers_changed || had_updates {
        let peer_infos: Vec<crate::bevy::state_store::PeerInfo> = socket_res
            .connected_peers
            .iter()
            .map(|pid| crate::bevy::state_store::PeerInfo {
                peer_id: pid.to_string(),
                player_id: socket_res.peer_player_map.get(pid).cloned(),
                is_host: false,
            })
            .collect();
        state_stores.peers.set_peers(peer_infos);
    }

    // 2. Receive messages
    let received = socket_res.socket.0.channel_mut(0).receive();

    for (peer_id, data) in received {
        let Ok(msg) = P2pMessage::decode(&*data) else {
            continue;
        };

        let (should_process, relay_targets) = gossip.handle_incoming(&msg, peer_id);

        if should_process {
            if let Some(payload) = &msg.payload {
                process_p2p_payload(
                    socket_res.as_mut(),
                    gossip.as_mut(),
                    &mut sync_state,
                    &command_queue,
                    &state_stores,
                    &mut sync_request_events,
                    peer_id,
                    &msg,
                    payload,
                );
            }
        }

        // Relay if needed
        if !relay_targets.is_empty() {
            let relay_msg = gossip.prepare_for_relay(&msg);
            let relay_data = relay_msg.encode_to_vec();
            for target in relay_targets {
                socket_res
                    .socket
                    .0
                    .channel_mut(0)
                    .send(relay_data.clone().into_boxed_slice(), target);
            }
        }
    }

    // 3. Process outgoing P2P commands (chat, reaction, ping)
    for cmd in command_queue.drain_p2p_send() {
        match cmd {
            GameCommand::SendChat { content } => {
                let timestamp_ms = js_sys::Date::now() as u64;
                let msg = gossip.create_message(
                    &socket_res.player_id,
                    3,
                    Payload::ChatMessage(marble_proto::play::ChatMessage {
                        user_id: socket_res.player_id.clone(),
                        content: content.clone(),
                        timestamp_ms,
                    }),
                );
                let data = msg.encode_to_vec();
                for peer in gossip.get_all_peers() {
                    socket_res
                        .socket
                        .0
                        .channel_mut(0)
                        .send(data.clone().into_boxed_slice(), peer);
                }
                // Also add to local chat store
                state_stores.chat.add_message(
                    socket_res.player_id.clone(),
                    content,
                    timestamp_ms as f64,
                );
            }
            GameCommand::SendReaction { emoji } => {
                let timestamp_ms = js_sys::Date::now() as u64;
                let msg = gossip.create_message(
                    &socket_res.player_id,
                    3,
                    Payload::Reaction(marble_proto::play::Reaction {
                        user_id: socket_res.player_id.clone(),
                        emoji: emoji.clone(),
                        timestamp_ms,
                    }),
                );
                let data = msg.encode_to_vec();
                for peer in gossip.get_all_peers() {
                    socket_res
                        .socket
                        .0
                        .channel_mut(0)
                        .send(data.clone().into_boxed_slice(), peer);
                }
                // Also add to local reaction store
                state_stores.reactions.add_reaction(
                    socket_res.player_id.clone(),
                    emoji,
                    timestamp_ms as f64,
                );
            }
            GameCommand::SendPing => {
                let msg = gossip.create_message(
                    &socket_res.player_id,
                    1,
                    Payload::Ping(Ping {
                        timestamp: js_sys::Date::now(),
                    }),
                );
                let data = msg.encode_to_vec();
                for peer in gossip.get_all_peers() {
                    socket_res
                        .socket
                        .0
                        .channel_mut(0)
                        .send(data.clone().into_boxed_slice(), peer);
                }
            }
            GameCommand::SendPingTo { peer_id } => {
                if let Ok(uuid) = uuid::Uuid::parse_str(&peer_id) {
                    let target = PeerId::from(uuid);
                    let msg = gossip.create_message(
                        &socket_res.player_id,
                        1,
                        Payload::Ping(Ping {
                            timestamp: js_sys::Date::now(),
                        }),
                    );
                    let data = msg.encode_to_vec();
                    socket_res
                        .socket
                        .0
                        .channel_mut(0)
                        .send(data.into_boxed_slice(), target);
                    tracing::debug!("[p2p] Sent targeted ping to {}", peer_id);
                }
            }
            _ => {}
        }
    }
}

/// Process a single P2P payload.
#[allow(clippy::too_many_arguments)]
fn process_p2p_payload(
    socket_res: &mut P2pSocketRes,
    gossip: &mut GossipHandler,
    sync_state: &mut SyncState,
    command_queue: &CommandQueue,
    state_stores: &StateStores,
    sync_request_events: &mut MessageWriter<SyncSnapshotRequestEvent>,
    peer_id: PeerId,
    msg: &P2pMessage,
    payload: &Payload,
) {
    match payload {
        Payload::GameStart(game_start) => {
            // Only peers process GameStart (host sent it)
            if sync_state.is_host {
                return;
            }

            // Check session version
            if game_start.session_version <= sync_state.session_version {
                return;
            }
            sync_state.session_version = game_start.session_version;

            tracing::info!(
                "[p2p] Received GameStart: seed={}, gamerule={}, session={}",
                game_start.seed,
                game_start.gamerule,
                game_start.session_version
            );

            // Parse player list from initial_state
            if let Ok(state_json) =
                serde_json::from_slice::<serde_json::Value>(&game_start.initial_state)
            {
                // Push commands to CommandQueue for next-frame processing
                command_queue.push(GameCommand::SetSyncHost { is_host: false });
                command_queue.push(GameCommand::SetSeed {
                    seed: game_start.seed,
                });
                if !game_start.gamerule.is_empty() {
                    command_queue.push(GameCommand::SetGamerule {
                        gamerule: game_start.gamerule.clone(),
                    });
                }
                command_queue.push(GameCommand::ClearMarbles);
                command_queue.push(GameCommand::ClearPlayers);
                command_queue.push(GameCommand::Yield);

                // Add players from the game start message
                if let Some(players) = state_json["players"].as_array() {
                    let colors = state_json["colors"].as_array();
                    for (i, player_name) in players.iter().enumerate() {
                        if let Some(name) = player_name.as_str() {
                            let color = colors
                                .and_then(|c| c.get(i))
                                .and_then(|c| c.as_array())
                                .map(|arr| {
                                    crate::marble::Color::new(
                                        arr.first().and_then(|v| v.as_u64()).unwrap_or(255) as u8,
                                        arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                                        arr.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                                        arr.get(3).and_then(|v| v.as_u64()).unwrap_or(255) as u8,
                                    )
                                })
                                .unwrap_or(crate::marble::Color::new(255, 255, 255, 255));

                            command_queue.push(GameCommand::AddPlayer {
                                name: name.to_string(),
                                color,
                            });
                        }
                    }
                }

                // Do NOT spawn marbles here — marbles are restored from snapshot only.
                // Instead, send SyncRequest to host to get the full snapshot.
            }

            // Set host peer id
            socket_res.host_peer_id = Some(peer_id);

            // Send SyncRequest to host for full state snapshot (including marbles)
            let sync_msg = gossip.create_message(
                &socket_res.player_id,
                1,
                Payload::SyncRequest(marble_proto::play::SyncRequest { from_frame: 0 }),
            );
            let sync_data = sync_msg.encode_to_vec();
            socket_res
                .socket
                .0
                .channel_mut(0)
                .send(sync_data.into_boxed_slice(), peer_id);

            tracing::info!("[p2p] Sent SyncRequest to host after GameStart");
        }

        Payload::FrameHash(hash) => {
            // Only peers compare hashes
            if sync_state.is_host {
                return;
            }

            // Buffer the received hash for later comparison when we reach that frame
            sync_state.pending_hashes.push((hash.frame, hash.hash));
        }

        Payload::SyncRequest(request) => {
            // Only host processes sync requests
            if !sync_state.is_host {
                return;
            }

            tracing::info!(
                "[p2p] Received SyncRequest from {} at frame {}",
                peer_id,
                request.from_frame
            );

            sync_request_events.write(SyncSnapshotRequestEvent {
                peer_id_bytes: peer_id.0.as_bytes().to_vec(),
                from_frame: request.from_frame,
            });
        }

        Payload::SyncState(sync_state_msg) => {
            // Only peers apply sync state
            if sync_state.is_host {
                return;
            }

            // Set host peer ID if not yet known (e.g., from auto-sync before GameStart)
            if socket_res.host_peer_id.is_none() {
                socket_res.host_peer_id = Some(peer_id);
                tracing::info!("[p2p] Set host_peer_id from SyncState: {}", peer_id);
            }

            tracing::info!("[p2p] Received SyncState at frame {}", sync_state_msg.frame);

            // Store pending snapshot for apply_sync_snapshot system
            sync_state.pending_snapshot = Some(sync_state_msg.state.clone());
            sync_state.last_sync_frame = sync_state_msg.frame;
            // Clear pending hashes since we're about to apply a fresh snapshot
            sync_state.pending_hashes.clear();
        }

        Payload::ChatMessage(chat) => {
            state_stores.chat.add_message(
                chat.user_id.clone(),
                chat.content.clone(),
                chat.timestamp_ms as f64,
            );
        }

        Payload::Reaction(reaction) => {
            state_stores.reactions.add_reaction(
                reaction.user_id.clone(),
                reaction.emoji.clone(),
                reaction.timestamp_ms as f64,
            );
        }

        Payload::Ping(ping) => {
            // Reply with pong
            let pong = gossip.create_message(
                &socket_res.player_id,
                1,
                Payload::Pong(Pong {
                    timestamp: ping.timestamp,
                }),
            );
            let pong_data = pong.encode_to_vec();
            socket_res
                .socket
                .0
                .channel_mut(0)
                .send(pong_data.into_boxed_slice(), peer_id);
        }

        Payload::Pong(pong) => {
            let now = js_sys::Date::now();
            let rtt = (now - pong.timestamp) as u32;
            tracing::debug!("[p2p] RTT to {}: {}ms", peer_id, rtt);
            // Record pong in PongStore for Yew to consume
            state_stores
                .pongs
                .record_pong(peer_id.to_string(), pong.timestamp);
        }

        _ => {
            tracing::debug!(
                "[p2p] Unhandled payload from {} (msg_id={})",
                msg.origin_user,
                msg.message_id
            );
        }
    }
}

// ============================================================================
// Hash Computation
// ============================================================================

/// Computes a deterministic hash of the current game state.
///
/// Uses the PhysicsWorld's own hash computation for body state,
/// plus map object transforms for keyframe-animated objects.
fn compute_bevy_hash(physics: &PhysicsWorldRes, map_objects: &[(String, Vec2, f32)]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    // Use PhysicsWorld's deterministic hash (includes frame, all body positions/velocities)
    physics.world.compute_hash().hash(&mut hasher);

    // Also hash map object transforms
    let mut sorted_objects: Vec<_> = map_objects.to_vec();
    sorted_objects.sort_by(|a, b| a.0.cmp(&b.0));

    for (id, pos, rot) in &sorted_objects {
        id.hash(&mut hasher);
        pos.x.to_bits().hash(&mut hasher);
        pos.y.to_bits().hash(&mut hasher);
        rot.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// Collects keyframe-animated map object transforms for hashing.
fn collect_map_object_data(
    keyframe_targets: &Query<(&KeyframeTarget, &Transform), Without<Marble>>,
) -> Vec<(String, Vec2, f32)> {
    keyframe_targets
        .iter()
        .map(|(kt, t)| {
            (
                kt.object_id.clone(),
                t.translation.truncate(),
                t.rotation.to_euler(EulerRot::ZYX).0,
            )
        })
        .collect()
}

// ============================================================================
// Frame Hash Broadcasting (Host only, FixedUpdate)
// ============================================================================

/// Broadcasts frame hash to all peers at regular intervals.
///
/// Only runs when this client is the host.
pub fn broadcast_frame_hash(
    mut socket_res: Option<ResMut<P2pSocketRes>>,
    mut gossip: Option<ResMut<GossipHandler>>,
    sync_state: Res<SyncState>,
    game_state: Res<MarbleGameState>,
    physics: Res<PhysicsWorldRes>,
    keyframe_targets: Query<(&KeyframeTarget, &Transform), Without<Marble>>,
) {
    if !sync_state.is_host {
        return;
    }

    let Some(socket_res) = socket_res.as_mut() else {
        return;
    };
    let Some(gossip) = gossip.as_mut() else {
        return;
    };

    // Only broadcast at intervals
    if game_state.frame == 0 || game_state.frame % HASH_BROADCAST_INTERVAL != 0 {
        return;
    }

    let map_object_data = collect_map_object_data(&keyframe_targets);
    let hash = compute_bevy_hash(&physics, &map_object_data);

    let msg = gossip.create_message(
        &socket_res.player_id,
        3,
        Payload::FrameHash(FrameHash {
            frame: game_state.frame,
            hash,
        }),
    );

    let data = msg.encode_to_vec();
    for peer in gossip.get_all_peers() {
        socket_res
            .socket
            .0
            .channel_mut(0)
            .send(data.clone().into_boxed_slice(), peer);
    }
}

// ============================================================================
// Desync Detection (Peer only, FixedUpdate)
// ============================================================================

/// Checks for desync by comparing local hash with buffered host hashes.
///
/// Compares hashes only when the peer reaches the exact frame the host hashed.
/// On mismatch, immediately sends a SyncRequest (with cooldown).
pub fn check_desync(
    mut socket_res: Option<ResMut<P2pSocketRes>>,
    mut gossip: Option<ResMut<GossipHandler>>,
    mut sync_state: ResMut<SyncState>,
    game_state: Res<MarbleGameState>,
    physics: Res<PhysicsWorldRes>,
    keyframe_targets: Query<(&KeyframeTarget, &Transform), Without<Marble>>,
) {
    if sync_state.is_host {
        return;
    }

    let current_frame = game_state.frame;

    // Extract hashes for the current frame; retain future ones, discard old ones
    let mut to_check = Vec::new();
    sync_state.pending_hashes.retain(|&(frame, hash)| {
        if frame == current_frame {
            to_check.push((frame, hash));
            false
        } else if frame < current_frame {
            // Already passed this frame, discard
            false
        } else {
            // Future frame, keep
            true
        }
    });

    if to_check.is_empty() {
        return;
    }

    let map_object_data = collect_map_object_data(&keyframe_targets);

    let mut need_resync = false;

    for (_host_frame, host_hash) in to_check {
        let local_hash = compute_bevy_hash(&physics, &map_object_data);
        if local_hash == host_hash {
            continue;
        }

        tracing::warn!(
            "[p2p] DESYNC at frame {}: host={:#x} local={:#x}",
            current_frame,
            host_hash,
            local_hash
        );
        need_resync = true;
    }

    if !need_resync {
        return;
    }

    // Check cooldown before requesting resync
    if current_frame.saturating_sub(sync_state.last_sync_frame) < SYNC_COOLDOWN {
        return;
    }

    let Some(socket_res) = socket_res.as_mut() else {
        return;
    };
    let Some(gossip) = gossip.as_mut() else {
        return;
    };

    // Send sync request to host
    if let Some(host_peer) = socket_res.host_peer_id {
        let msg = gossip.create_message(
            &socket_res.player_id,
            1,
            Payload::SyncRequest(marble_proto::play::SyncRequest {
                from_frame: current_frame,
            }),
        );
        let data = msg.encode_to_vec();
        socket_res
            .socket
            .0
            .channel_mut(0)
            .send(data.into_boxed_slice(), host_peer);

        sync_state.last_sync_frame = current_frame;
        tracing::info!(
            "[p2p] Sent resync request to host at frame {}",
            current_frame
        );
    }
}

// ============================================================================
// Sync Snapshot (Host: create & send, Peer: apply)
// ============================================================================

/// Handles sync snapshot requests from peers (host only).
///
/// Creates a `BevySyncSnapshot` from current ECS state and sends it to the requesting peer.
/// Now includes serialized PhysicsWorld for complete state restoration.
#[allow(clippy::too_many_arguments)]
pub fn handle_sync_request(
    mut events: MessageReader<SyncSnapshotRequestEvent>,
    mut socket_res: Option<ResMut<P2pSocketRes>>,
    mut gossip: Option<ResMut<GossipHandler>>,
    game_state: Res<MarbleGameState>,
    sync_state: Res<SyncState>,
    rng: Res<DeterministicRng>,
    game_context: Res<GameContextRes>,
    physics: Res<PhysicsWorldRes>,
    marbles: Query<(&Marble, &MarbleVisual, &Transform, &PhysicsBody)>,
    keyframe_targets: Query<(&KeyframeTarget, &Transform), Without<Marble>>,
    keyframe_executors: Res<KeyframeExecutors>,
) {
    if !sync_state.is_host {
        // Drain events even if not host
        for _ in events.read() {}
        return;
    }

    let Some(socket_res) = socket_res.as_mut() else {
        for _ in events.read() {}
        return;
    };
    let Some(gossip) = gossip.as_mut() else {
        for _ in events.read() {}
        return;
    };

    for event in events.read() {
        // Create marble snapshots (reading velocity from physics world)
        let marble_snapshots: Vec<MarbleSnapshot> = marbles
            .iter()
            .map(|(marble, visual, transform, body)| {
                let (linvel, angvel) = physics
                    .world
                    .get_rigid_body(body.0)
                    .map(|b| {
                        let lv = b.linvel();
                        let av = b.angvel();
                        ([lv.x, lv.y], av)
                    })
                    .unwrap_or(([0.0, 0.0], 0.0));

                MarbleSnapshot {
                    owner_id: marble.owner_id,
                    eliminated: marble.eliminated,
                    color: visual.color,
                    radius: visual.radius,
                    position: [transform.translation.x, transform.translation.y],
                    rotation: transform.rotation.to_euler(EulerRot::ZYX).0,
                    linear_velocity: linvel,
                    angular_velocity: angvel,
                }
            })
            .collect();

        // Collect keyframe-animated map object transforms
        let map_object_transforms: Vec<MapObjectTransformSnapshot> = keyframe_targets
            .iter()
            .map(|(kt, t)| MapObjectTransformSnapshot {
                object_id: kt.object_id.clone(),
                position: [t.translation.x, t.translation.y],
                rotation: t.rotation.to_euler(EulerRot::ZYX).0,
            })
            .collect();

        // Serialize the entire PhysicsWorld for complete state restoration
        let physics_world_bytes = postcard::to_allocvec(&physics.world).unwrap_or_else(|e| {
            tracing::error!("[p2p] Failed to serialize PhysicsWorld: {}", e);
            Vec::new()
        });
        let physics_world_bytes_len = physics_world_bytes.len();

        let snapshot = BevySyncSnapshot {
            frame: game_state.frame,
            rng_seed: game_state.rng_seed,
            det_rng: Some(rng.rng.clone()),
            game_ctx_rng: game_context.context.capture_rng(),
            game_ctx_time: game_context.context.time,
            players: game_state.players.clone(),
            arrival_order: game_state.arrival_order.clone(),
            arrival_frames: game_state.arrival_frames.clone(),
            selected_gamerule: game_state.selected_gamerule.clone(),
            marbles: marble_snapshots,
            keyframe_executors: keyframe_executors.executors.clone(),
            activated_keyframes: keyframe_executors.activated.clone(),
            map_object_transforms,
            physics_world_bytes,
        };

        match snapshot.to_bytes() {
            Ok(state_bytes) => {
                // Reconstruct PeerId from bytes
                if event.peer_id_bytes.len() == 16 {
                    let mut bytes = [0u8; 16];
                    bytes.copy_from_slice(&event.peer_id_bytes);
                    let uuid = uuid::Uuid::from_bytes(bytes);
                    let target_peer = PeerId::from(uuid);

                    let msg = gossip.create_message(
                        &socket_res.player_id,
                        1,
                        Payload::SyncState(marble_proto::play::SyncState {
                            frame: game_state.frame,
                            state: state_bytes,
                        }),
                    );
                    let data = msg.encode_to_vec();
                    socket_res
                        .socket
                        .0
                        .channel_mut(0)
                        .send(data.into_boxed_slice(), target_peer);

                    tracing::info!(
                        "[p2p] Sent sync snapshot to peer {} at frame {} (physics_world: {} bytes)",
                        target_peer,
                        game_state.frame,
                        physics_world_bytes_len
                    );
                }
            }
            Err(e) => {
                tracing::error!("[p2p] Failed to serialize sync snapshot: {}", e);
            }
        }
    }
}

/// Applies a pending sync snapshot (peer only).
///
/// Now uses PhysicsWorld deserialization for complete state restoration,
/// preserving all Rapier internal state (NarrowPhase, warm-starting, etc.).
#[allow(clippy::too_many_arguments)]
pub fn apply_sync_snapshot(
    mut commands: Commands,
    mut game_state: ResMut<MarbleGameState>,
    mut rng: ResMut<DeterministicRng>,
    mut game_context: ResMut<GameContextRes>,
    mut sync_state: ResMut<SyncState>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    mut physics: ResMut<PhysicsWorldRes>,
    existing_marbles: Query<Entity, With<Marble>>,
    mut marble_bodies: Query<(&Marble, &mut PhysicsBody)>,
    mut keyframe_targets: Query<(&KeyframeTarget, &mut Transform), Without<Marble>>,
) {
    let Some(snapshot_bytes) = sync_state.pending_snapshot.take() else {
        return;
    };

    if sync_state.is_host {
        return;
    }

    let snapshot = match BevySyncSnapshot::from_bytes(&snapshot_bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("[p2p] Failed to deserialize sync snapshot: {}", e);
            return;
        }
    };

    tracing::info!(
        "[p2p] Applying sync snapshot: frame={}, {} players, {} marbles, {} executors, {} map_objects, physics_world={} bytes",
        snapshot.frame,
        snapshot.players.len(),
        snapshot.marbles.len(),
        snapshot.keyframe_executors.len(),
        snapshot.map_object_transforms.len(),
        snapshot.physics_world_bytes.len()
    );

    // 1. Try to restore PhysicsWorld from serialized bytes
    if !snapshot.physics_world_bytes.is_empty() {
        match postcard::from_bytes::<crate::physics::PhysicsWorld>(&snapshot.physics_world_bytes) {
            Ok(restored_world) => {
                tracing::info!(
                    "[p2p] Restored PhysicsWorld: frame={}, {} bodies, {} colliders",
                    restored_world.frame,
                    restored_world.rigid_body_set.len(),
                    restored_world.collider_set.len()
                );

                // Replace the entire physics world
                physics.world = restored_world;

                // Rebuild entity mappings: update PhysicsBody handles for existing marbles
                // by matching user_data (which stores Entity bits)
                for (marble, mut body_comp) in marble_bodies.iter_mut() {
                    // Find the matching body in the restored world
                    let mut found = false;
                    for (handle, body) in physics.world.rigid_body_set.iter() {
                        // Check if this body belongs to this marble entity
                        // We match by checking if it's a dynamic body (marble)
                        // and checking user_data against entity bits
                        if body.is_dynamic() && body.user_data != 0 {
                            let stored_entity = Entity::from_bits(body.user_data as u64);
                            // This won't match because the entity was from the host.
                            // Instead, we need to update user_data to point to our entities.
                            let _ = stored_entity;
                        }
                        let _ = handle;
                    }

                    if !found {
                        let _ = marble;
                    }
                }

                // Since entity handles from host differ from ours, we need to
                // despawn existing marbles and respawn them with correct mapping.
                // But first update the body user_data to match our entities.
            }
            Err(e) => {
                tracing::warn!(
                    "[p2p] Failed to deserialize PhysicsWorld, falling back to marble-level sync: {}",
                    e
                );
            }
        }
    }

    // 2. Despawn all existing marbles (they'll be respawned with correct physics handles)
    for entity in existing_marbles.iter() {
        commands.entity(entity).despawn();
    }

    // 3. Restore game state
    game_state.players = snapshot.players;
    game_state.arrival_order = snapshot.arrival_order;
    game_state.arrival_frames = snapshot.arrival_frames;
    game_state.frame = snapshot.frame;
    game_state.rng_seed = snapshot.rng_seed;
    game_state.selected_gamerule = snapshot.selected_gamerule;

    // 4. Restore RNG and GameContext with full internal state
    if let Some(det_rng) = snapshot.det_rng {
        rng.rng = det_rng;
    } else {
        *rng = DeterministicRng::new(snapshot.rng_seed);
    }

    if let Some(ctx_rng) = snapshot.game_ctx_rng {
        game_context.context.restore_rng(ctx_rng);
    } else {
        *game_context = GameContextRes::new(snapshot.rng_seed);
    }
    game_context.update(snapshot.game_ctx_time, snapshot.frame);

    // 5. Restore keyframe executor state (always restore activation state)
    keyframe_executors.activated = snapshot.activated_keyframes;
    if !snapshot.keyframe_executors.is_empty() {
        keyframe_executors.executors = snapshot.keyframe_executors;
    }
    tracing::info!(
        "[p2p] Restored keyframe state: {} executors, activated={:?}",
        keyframe_executors.executors.len(),
        keyframe_executors.activated
    );

    // 6. Restore map object transforms from snapshot
    if !snapshot.map_object_transforms.is_empty() {
        for obj_snap in &snapshot.map_object_transforms {
            for (kt, mut transform) in keyframe_targets.iter_mut() {
                if kt.object_id == obj_snap.object_id {
                    transform.translation.x = obj_snap.position[0];
                    transform.translation.y = obj_snap.position[1];
                    transform.rotation = Quat::from_rotation_z(obj_snap.rotation);
                }
            }
        }
        tracing::info!(
            "[p2p] Restored {} map object transforms",
            snapshot.map_object_transforms.len()
        );
    }

    // 7. Respawn marbles from snapshot, creating new physics bodies in the restored world
    //    If physics_world_bytes was restored, we need to find matching bodies.
    //    If not, we create new bodies.
    let has_restored_world = !snapshot.physics_world_bytes.is_empty();

    for marble_snap in &snapshot.marbles {
        let mut transform = Transform::from_translation(
            Vec2::new(marble_snap.position[0], marble_snap.position[1]).extend(0.0),
        );
        transform.rotation = Quat::from_rotation_z(marble_snap.rotation);

        let entity = commands
            .spawn((
                Marble {
                    owner_id: marble_snap.owner_id,
                    eliminated: marble_snap.eliminated,
                },
                MarbleVisual {
                    color: marble_snap.color,
                    radius: marble_snap.radius,
                },
                transform,
                PhysicsExternalForce::default(),
            ))
            .id();

        if has_restored_world {
            // Find the matching dynamic body by position (approximate match)
            let mut best_handle = None;
            let mut best_dist = f32::MAX;

            for (handle, body) in physics.world.rigid_body_set.iter() {
                if !body.is_dynamic() {
                    continue;
                }
                let pos = body.translation();
                let dx = pos.x - marble_snap.position[0];
                let dy = pos.y - marble_snap.position[1];
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_handle = Some(handle);
                }
            }

            if let Some(handle) = best_handle {
                // Collect collider handles first to avoid borrow conflict
                let collider_handles: Vec<ColliderHandle> = physics
                    .world
                    .rigid_body_set
                    .get(handle)
                    .map(|b| b.colliders().to_vec())
                    .unwrap_or_default();

                // Update the body's user_data to point to the new entity
                if let Some(body) = physics.world.rigid_body_set.get_mut(handle) {
                    body.user_data = entity.to_bits() as u128;
                }
                // Also update collider user_data
                for collider_handle in collider_handles {
                    if let Some(collider) = physics.world.collider_set.get_mut(collider_handle) {
                        collider.user_data = entity.to_bits() as u128;
                    }
                }
                commands.entity(entity).insert(PhysicsBody(handle));
            } else {
                // Fallback: create a new body
                let body_handle = create_marble_body(&mut physics.world, entity, marble_snap);
                commands.entity(entity).insert(PhysicsBody(body_handle));
            }
        } else {
            // No restored world, create new physics body
            let body_handle = create_marble_body(&mut physics.world, entity, marble_snap);
            commands.entity(entity).insert(PhysicsBody(body_handle));
        }
    }

    // 8. Clear pending hashes after snapshot restore
    sync_state.pending_hashes.clear();
}

/// Creates a new marble body in the physics world.
fn create_marble_body(
    world: &mut crate::physics::PhysicsWorld,
    entity: Entity,
    snap: &MarbleSnapshot,
) -> RigidBodyHandle {
    let body = RigidBodyBuilder::dynamic()
        .translation(Vector::new(snap.position[0], snap.position[1]))
        .rotation(snap.rotation)
        .linvel(Vector::new(
            snap.linear_velocity[0],
            snap.linear_velocity[1],
        ))
        .angvel(snap.angular_velocity)
        .ccd_enabled(true)
        .linear_damping(0.5)
        .angular_damping(0.5)
        .user_data(entity.to_bits() as u128)
        .build();
    let handle = world.add_rigid_body(body);

    let collider = ColliderBuilder::ball(snap.radius)
        .restitution(0.7)
        .friction(0.3)
        .density(1.0)
        .active_events(ActiveEvents::COLLISION_EVENTS)
        .user_data(entity.to_bits() as u128)
        .build();
    world.add_collider(collider, handle);

    handle
}

// ============================================================================
// Game Start Broadcasting (Host only, Update)
// ============================================================================

/// Broadcasts a GameStart message to all peers when triggered.
pub fn broadcast_game_start(
    mut events: MessageReader<BroadcastGameStartEvent>,
    mut socket_res: Option<ResMut<P2pSocketRes>>,
    mut gossip: Option<ResMut<GossipHandler>>,
    game_state: Res<MarbleGameState>,
    mut sync_state: ResMut<SyncState>,
    marbles: Query<(&Marble, &MarbleVisual, &Transform)>,
) {
    let Some(socket_res) = socket_res.as_mut() else {
        for _ in events.read() {}
        return;
    };
    let Some(gossip) = gossip.as_mut() else {
        for _ in events.read() {}
        return;
    };

    for _ in events.read() {
        // Increment session version
        sync_state.session_version += 1;

        // Build player list as JSON for initial_state
        let player_names: Vec<&str> = game_state.players.iter().map(|p| p.name.as_str()).collect();
        let player_colors: Vec<[u8; 4]> = game_state
            .players
            .iter()
            .map(|p| [p.color.r, p.color.g, p.color.b, p.color.a])
            .collect();

        // Collect marble positions sorted by owner_id
        let mut marble_data: Vec<_> = marbles
            .iter()
            .map(|(m, _vis, t)| {
                let pos = t.translation.truncate();
                (m.owner_id, [pos.x, pos.y])
            })
            .collect();
        marble_data.sort_by_key(|(id, _)| *id);

        let state_json = serde_json::json!({
            "players": player_names,
            "colors": player_colors,
            "marble_positions": marble_data.iter()
                .map(|(_, pos)| pos)
                .collect::<Vec<_>>(),
        });
        let initial_state = serde_json::to_vec(&state_json).unwrap_or_default();

        let msg = gossip.create_message(
            &socket_res.player_id,
            3,
            Payload::GameStart(marble_proto::play::GameStart {
                seed: game_state.rng_seed,
                initial_state,
                gamerule: game_state.selected_gamerule.clone(),
                session_version: sync_state.session_version,
            }),
        );

        let data = msg.encode_to_vec();
        for peer in gossip.get_all_peers() {
            socket_res
                .socket
                .0
                .channel_mut(0)
                .send(data.clone().into_boxed_slice(), peer);
        }

        tracing::info!(
            "[p2p] Broadcast GameStart: seed={}, {} players, session={}",
            game_state.rng_seed,
            game_state.players.len(),
            sync_state.session_version,
        );
    }
}
