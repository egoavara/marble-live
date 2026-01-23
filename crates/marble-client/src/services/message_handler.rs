//! P2P message handling logic.

use crate::p2p::protocol::{P2PMessage, PlayerStartInfo};
use crate::p2p::state::{P2PAction, P2PStateContext};
use crate::p2p::sync::{HashCompareResult, RttTracker, SyncTracker};
use matchbox_socket::PeerId;
use std::cell::RefCell;
use std::rc::Rc;

/// Handle incoming P2P messages.
pub fn handle_message(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
    from: PeerId,
    data: &[u8],
) {
    let Some(msg) = P2PMessage::decode(data) else {
        return;
    };

    match msg {
        P2PMessage::PlayerInfo { name, color, hash_code } => {
            state.dispatch(P2PAction::UpdatePeerInfo {
                peer_id: from,
                name,
                color,
                hash_code,
            });
        }
        P2PMessage::PeerAnnounce { player_id } => {
            // Map peer_id to player_id for server-authoritative player lookup
            state.dispatch(P2PAction::MapPeerToPlayer {
                peer_id: from,
                player_id,
            });
        }
        P2PMessage::GameStartOrder { seed, player_order } => {
            // New game start - uses explicit player order from host
            state.dispatch(P2PAction::StartGameFromServer { seed, player_order });
            state.dispatch(P2PAction::StartCountdown);
        }
        P2PMessage::FrameHash { frame, hash } => {
            handle_frame_hash(state, sync_tracker, from, frame, hash);
        }
        P2PMessage::SyncRequest { from_frame } => {
            handle_sync_request(state, from, from_frame);
        }
        P2PMessage::SyncState { frame, state: state_data } => {
            state.dispatch(P2PAction::ApplySyncState {
                frame,
                state_data,
            });
        }
        P2PMessage::Ping { timestamp } => {
            let msg = P2PMessage::Pong { timestamp };
            state.network.borrow_mut().send_to(from, &msg.encode());
        }
        P2PMessage::Pong { timestamp } => {
            let now = js_sys::Date::now();
            if let Some(rtt) = rtt_tracker.borrow_mut().process_pong(from, timestamp, now) {
                state.dispatch(P2PAction::UpdatePeerRtt {
                    peer_id: from,
                    rtt_ms: rtt,
                });
            }
        }
        P2PMessage::ReconnectRequest { name, color, hash_code } => {
            handle_reconnect_request(state, from, name, color, hash_code);
        }
        P2PMessage::ReconnectResponse { seed, frame, state: state_data, players } => {
            handle_reconnect_response(state, seed, frame, state_data, players);
        }
    }
}

fn handle_frame_hash(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    from: PeerId,
    frame: u64,
    hash: u64,
) {
    use crate::p2p::state::P2PPhase;

    sync_tracker.borrow_mut().record_peer_hash(frame, from, hash);
    state.dispatch(P2PAction::ReceiveFrameHash {
        peer_id: from,
        frame,
        hash,
    });

    if matches!(state.phase, P2PPhase::Running) && !state.desync_detected {
        let my_frame = state.game_state.current_frame();
        let my_hash = state.game_state.compute_hash();

        if frame == my_frame {
            let connected_peer_count = state.peers.values().filter(|p| p.connected).count();
            let result = sync_tracker.borrow_mut().compare_hashes(
                frame,
                my_hash,
                connected_peer_count,
            );

            match result {
                HashCompareResult::Match => {}
                HashCompareResult::Waiting => {}
                HashCompareResult::Desync { majority_hash } => {
                    state.dispatch(P2PAction::AddLog(format!(
                        "Desync detected at frame {}: mine={:016X}, majority={:016X}",
                        frame, my_hash, majority_hash
                    )));
                    state.dispatch(P2PAction::DetectDesync);
                    state.dispatch(P2PAction::StartResync);

                    let sync_source = state.peer_hashes.iter()
                        .find(|&(_, (f, h))| *f == frame && *h == majority_hash)
                        .map(|(peer_id, _)| *peer_id);

                    if let Some(source_peer) = sync_source {
                        let msg = P2PMessage::SyncRequest { from_frame: my_frame };
                        state.network.borrow_mut().send_to(source_peer, &msg.encode());
                    }
                }
            }
        }
    }
}

fn handle_sync_request(state: &P2PStateContext, from: PeerId, from_frame: u64) {
    let snapshot = state.game_state.create_snapshot();
    match snapshot.to_bytes() {
        Ok(state_data) => {
            let msg = P2PMessage::SyncState {
                frame: state.game_state.current_frame(),
                state: state_data,
            };
            state.network.borrow_mut().send_to(from, &msg.encode());
            state.dispatch(P2PAction::AddLog(format!(
                "Sent sync state (requested from frame {})",
                from_frame
            )));
        }
        Err(e) => {
            state.dispatch(P2PAction::AddLog(format!(
                "Failed to serialize sync state: {}",
                e
            )));
        }
    }
}

fn handle_reconnect_request(
    state: &P2PStateContext,
    from: PeerId,
    name: String,
    color: marble_core::Color,
    hash_code: String,
) {
    use crate::p2p::state::P2PPhase;

    // Only respond if we're in a game (Running, Countdown, or Finished)
    let is_in_game = matches!(
        state.phase,
        P2PPhase::Running | P2PPhase::Countdown { .. } | P2PPhase::Finished
    );

    if is_in_game {
        // Update peer info
        state.dispatch(P2PAction::UpdatePeerInfo {
            peer_id: from,
            name: name.clone(),
            color,
            hash_code,
        });

        // Send the current game state
        let snapshot = state.game_state.create_snapshot();
        match snapshot.to_bytes() {
            Ok(state_data) => {
                // Build player list from peer_player_map
                let mut players: Vec<PlayerStartInfo> = Vec::new();

                // Add self
                if let Some(my_peer_id) = state.my_peer_id {
                    players.push(PlayerStartInfo::new(
                        my_peer_id,
                        state.my_name.clone(),
                        state.my_color,
                    ));
                }

                // Add other peers
                for (peer_id, info) in state.peers.iter() {
                    players.push(PlayerStartInfo::new(
                        *peer_id,
                        info.name.clone(),
                        info.color,
                    ));
                }

                let msg = P2PMessage::ReconnectResponse {
                    seed: state.game_seed,
                    frame: state.game_state.current_frame(),
                    state: state_data,
                    players,
                };
                state.network.borrow_mut().send_to(from, &msg.encode());
                state.dispatch(P2PAction::AddLog(format!(
                    "Sent reconnect response to {}",
                    &from.0.to_string()[..8]
                )));
            }
            Err(e) => {
                state.dispatch(P2PAction::AddLog(format!(
                    "Failed to serialize state for reconnect: {}",
                    e
                )));
            }
        }
    }
}

fn handle_reconnect_response(
    state: &P2PStateContext,
    seed: u64,
    frame: u64,
    state_data: Vec<u8>,
    players: Vec<PlayerStartInfo>,
) {
    use crate::p2p::state::P2PPhase;

    // We received game state from a peer - apply it if we're reconnecting
    if matches!(state.phase, P2PPhase::Reconnecting) {
        let player_list: Vec<_> = players
            .iter()
            .map(|p| (p.peer_id(), p.name.clone(), p.color))
            .collect();

        state.dispatch(P2PAction::ApplyReconnectState {
            seed,
            frame,
            state_data,
            players: player_list,
        });
    }
}
