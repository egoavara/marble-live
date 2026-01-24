//! Game synchronization logic for P2P.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::SyncSnapshot;
use marble_proto::play::{FrameHash, GameStart, SyncRequest, SyncState};
use matchbox_socket::PeerId;
use prost::Message;

use super::room_state::P2pRoomState;
use super::GossipHandler;

/// Hash broadcast interval in frames (0.5 seconds at 60 FPS)
pub const HASH_BROADCAST_INTERVAL: u64 = 30;

/// Desync threshold - request resync after this many consecutive mismatches
pub const DESYNC_THRESHOLD: u32 = 3;

/// Sync cooldown in frames (3 seconds at 60 FPS)
pub const SYNC_COOLDOWN: u64 = 180;

/// Handle received frame hash from host
pub fn handle_frame_hash(
    state: &Rc<RefCell<P2pRoomState>>,
    hash: &FrameHash,
) -> bool {
    let mut state_mut = state.borrow_mut();

    // Store the received hash from host
    state_mut.last_host_hash = Some((hash.frame, hash.hash));

    // If we're the host, ignore (we sent this)
    if state_mut.is_host {
        return false;
    }

    // Compare with local game state if available
    let game_state_opt = state_mut.game_state.clone();
    if let Some(game_state) = game_state_opt {
        let game = game_state.borrow();
        let local_frame = game.current_frame();
        let local_hash = game.compute_hash();
        let current_frame = game.current_frame();
        drop(game);

        // Only compare if we're at or past the same frame
        if local_frame >= hash.frame {
            if local_hash == hash.hash {
                // Hashes match - reset desync count
                state_mut.desync_count = 0;
                tracing::debug!(
                    frame = hash.frame,
                    hash = hash.hash,
                    "Frame hash verified OK"
                );
            } else {
                // Hash mismatch
                state_mut.desync_count += 1;
                tracing::warn!(
                    frame = hash.frame,
                    host_hash = hash.hash,
                    local_hash = local_hash,
                    desync_count = state_mut.desync_count,
                    "Frame hash mismatch!"
                );

                // Check if we should request resync
                if state_mut.desync_count >= DESYNC_THRESHOLD {
                    let last_sync = state_mut.last_sync_frame;

                    // Only request if enough time has passed since last sync
                    if current_frame.saturating_sub(last_sync) >= SYNC_COOLDOWN {
                        return true; // Signal that sync request should be sent
                    }
                }
            }
        }
    }

    false
}

/// Handle sync request from a peer (host only)
pub fn handle_sync_request(
    state: &Rc<RefCell<P2pRoomState>>,
    socket: &Rc<RefCell<matchbox_socket::WebRtcSocket>>,
    gossip: &Rc<RefCell<GossipHandler>>,
    peer_id: PeerId,
    request: &SyncRequest,
) {
    let state_ref = state.borrow();

    // Only host can respond to sync requests
    if !state_ref.is_host {
        return;
    }

    // Get current game state
    let game_state_opt = state_ref.game_state.clone();
    let player_id = state_ref.player_id.clone();
    drop(state_ref);

    if let Some(game_state) = game_state_opt {
        let game = game_state.borrow();
        let snapshot = game.create_snapshot();
        let current_frame = game.current_frame();
        drop(game);

        match snapshot.to_bytes() {
            Ok(state_bytes) => {
                // Send sync state directly to the requesting peer
                let msg = {
                    let mut gossip = gossip.borrow_mut();
                    gossip.create_message(
                        &player_id,
                        1, // Direct message, TTL 1
                        marble_proto::play::p2p_message::Payload::SyncState(SyncState {
                            frame: current_frame,
                            state: state_bytes,
                        }),
                    )
                };

                let data = msg.encode_to_vec();
                let mut socket = socket.borrow_mut();
                socket.channel_mut(0).send(data.into_boxed_slice(), peer_id);

                tracing::info!(
                    peer_id = %peer_id,
                    from_frame = request.from_frame,
                    current_frame = current_frame,
                    "Sent sync state to peer"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize game state for sync");
            }
        }
    }
}

/// Handle sync state from host (non-host only)
pub fn handle_sync_state(
    state: &Rc<RefCell<P2pRoomState>>,
    sync_state: &SyncState,
) {
    let mut state_mut = state.borrow_mut();

    // Don't process if we're the host
    if state_mut.is_host {
        return;
    }

    // Deserialize and apply the snapshot
    match SyncSnapshot::from_bytes(&sync_state.state) {
        Ok(snapshot) => {
            let game_state_opt = state_mut.game_state.clone();
            if let Some(game_state) = game_state_opt {
                let mut game = game_state.borrow_mut();
                game.restore_from_snapshot(snapshot);

                tracing::info!(
                    frame = sync_state.frame,
                    "Game state restored from sync"
                );
            }

            // Update sync tracking
            state_mut.last_sync_frame = sync_state.frame;
            state_mut.desync_count = 0;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to deserialize sync state");
        }
    }
}

/// Handle game start from host
#[allow(dead_code)]
pub fn handle_game_start(
    state: &Rc<RefCell<P2pRoomState>>,
    game_start: &GameStart,
) -> bool {
    let state_ref = state.borrow();

    // Don't process if we're the host (we sent this)
    if state_ref.is_host {
        return false;
    }

    tracing::info!(
        seed = game_start.seed,
        state_size = game_start.initial_state.len(),
        "Received game start from host"
    );

    // The actual game initialization will be handled by the game loop hook
    // Return true to signal that game should start
    true
}

/// Check if this frame should broadcast hash (host only)
pub fn should_broadcast_hash(current_frame: u64, last_hash_frame: u64) -> bool {
    current_frame.saturating_sub(last_hash_frame) >= HASH_BROADCAST_INTERVAL
}
