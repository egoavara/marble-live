//! Hook for P2P network handling.

use crate::network::NetworkEvent;
use crate::p2p::protocol::P2PMessage;
use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use crate::p2p::sync::{RttTracker, SyncTracker};
use crate::services::handle_message;
use gloo::timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

/// Manage P2P network polling and message handling.
///
/// This hook handles:
/// - Polling for network events (peer join/leave, messages)
/// - Dispatching peer events to state
/// - Processing incoming P2P messages
/// - RTT ping/pong
/// - Broadcasting PlayerInfo when phase changes to lobby
#[hook]
pub fn use_p2p_network(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
) {
    // Network polling effect
    {
        let state = state.clone();
        let sync_tracker = sync_tracker.clone();
        let rtt_tracker = rtt_tracker.clone();
        let phase = state.phase.clone();

        use_effect_with(phase.clone(), move |phase| {
            // Poll during all phases except Disconnected
            let interval: Option<Interval> = if *phase == P2PPhase::Disconnected {
                None
            } else {
                let state_inner = state.clone();
                let sync_tracker_inner = sync_tracker.clone();
                let rtt_tracker_inner = rtt_tracker.clone();

                Some(Interval::new(16, move || {
                    poll_network(&state_inner, &sync_tracker_inner, &rtt_tracker_inner);
                }))
            };

            move || drop(interval)
        });
    }

    // Broadcast PlayerInfo when phase changes to lobby
    {
        let state = state.clone();
        let phase = state.phase.clone();

        use_effect_with(phase, move |phase| {
            // Only broadcast if we're in lobby and have peers
            let in_lobby = matches!(phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby);
            if in_lobby && !state.peers.is_empty() {
                let msg = P2PMessage::PlayerInfo {
                    name: if state.my_name.is_empty() {
                        "Player".to_string()
                    } else {
                        state.my_name.clone()
                    },
                    color: state.my_color,
                    hash_code: state.my_hash_code.clone(),
                };
                state.network.borrow_mut().broadcast(&msg.encode());
            }
            || ()
        });
    }
}

/// Poll the network for events.
fn poll_network(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
) {
    // Get our peer ID if we don't have it yet
    if state.my_peer_id.is_none() {
        if let Some(my_id) = state.network.borrow_mut().my_peer_id() {
            state.dispatch(P2PAction::SetMyPeerId(my_id));
        }
    }

    let events = state.network.borrow_mut().poll();

    for event in events {
        match event {
            NetworkEvent::PeerJoined(peer_id) => {
                state.dispatch(P2PAction::PeerJoined(peer_id));

                // If we're in Reconnecting phase, send a ReconnectRequest instead of normal info
                if matches!(state.phase, P2PPhase::Reconnecting) {
                    let msg = P2PMessage::ReconnectRequest {
                        name: if state.my_name.is_empty() {
                            "Player".to_string()
                        } else {
                            state.my_name.clone()
                        },
                        color: state.my_color,
                        hash_code: state.my_hash_code.clone(),
                    };
                    state.network.borrow_mut().send_to(peer_id, &msg.encode());
                    state.dispatch(P2PAction::AddLog(format!(
                        "Sent reconnect request to peer {}",
                        &peer_id.0.to_string()[..8]
                    )));
                } else {
                    // Send PeerAnnounce to map peer_id <-> player_id (server-authoritative)
                    if !state.my_player_id.is_empty() {
                        let announce_msg = P2PMessage::PeerAnnounce {
                            player_id: state.my_player_id.clone(),
                        };
                        state.network.borrow_mut().send_to(peer_id, &announce_msg.encode());
                    }

                    // Also send PlayerInfo for display purposes (name, color, hash_code)
                    let msg = P2PMessage::PlayerInfo {
                        name: if state.my_name.is_empty() {
                            "Player".to_string()
                        } else {
                            state.my_name.clone()
                        },
                        color: state.my_color,
                        hash_code: state.my_hash_code.clone(),
                    };
                    state.network.borrow_mut().send_to(peer_id, &msg.encode());
                }
            }
            NetworkEvent::PeerLeft(peer_id) => {
                state.dispatch(P2PAction::PeerLeft(peer_id));
                rtt_tracker.borrow_mut().remove_peer(peer_id);
            }
            NetworkEvent::Message { from, data } => {
                handle_message(state, sync_tracker, rtt_tracker, from, &data);
            }
            NetworkEvent::StateChanged(_) => {}
        }
    }

    // Send periodic pings for RTT measurement
    let now = js_sys::Date::now();
    if rtt_tracker.borrow().should_ping(now) {
        for &peer_id in state.peers.keys() {
            let msg = P2PMessage::Ping { timestamp: now };
            state.network.borrow_mut().send_to(peer_id, &msg.encode());
            rtt_tracker.borrow_mut().record_ping_sent(peer_id, now);
        }
    }
}
