//! P2P multiplayer game page.
//!
//! This page provides a full P2P game experience with lobby, synchronization,
//! and game state management.

use crate::components::{
    ConnectionPanel, DesyncWarning, EventLogPanel, GameStatusPanel, LobbyPanel, PeerStatusPanel,
};
use crate::network::NetworkEvent;
use crate::p2p::protocol::P2PMessage;
use crate::p2p::state::{P2PAction, P2PGameState, P2PPhase, P2PStateContext};
use crate::p2p::sync::{RttTracker, SyncTracker, HASH_EXCHANGE_INTERVAL};
use crate::renderer::CanvasRenderer;
use gloo::timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Canvas dimensions.
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

/// P2P Play page component.
#[function_component(DebugP2PPlayPage)]
pub fn debug_p2p_play_page() -> Html {
    let state = use_reducer(P2PGameState::new);
    let canvas_ref = use_node_ref();
    let renderer_ref = use_mut_ref(|| None::<CanvasRenderer>);
    let sync_tracker = use_mut_ref(SyncTracker::new);
    let rtt_tracker = use_mut_ref(RttTracker::new);

    // Initialize canvas and renderer - depends on phase so it re-runs when canvas becomes visible
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let phase = state.phase.clone();

        use_effect_with(phase, move |_phase| {
            // Try to initialize the renderer if we haven't yet
            if renderer_ref.borrow().is_none() {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    canvas.set_width(CANVAS_WIDTH);
                    canvas.set_height(CANVAS_HEIGHT);

                    if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                        *renderer_ref.borrow_mut() = Some(renderer);
                    }
                }
            }
            || ()
        });
    }

    // Network polling effect
    {
        let state = state.clone();
        let sync_tracker = sync_tracker.clone();
        let rtt_tracker = rtt_tracker.clone();

        use_effect_with(state.phase.clone(), move |phase| {
            // Only poll when connected
            let interval: Option<Interval> = if *phase == P2PPhase::Disconnected {
                None
            } else {
                Some(Interval::new(16, move || {
                    poll_network(&state, &sync_tracker, &rtt_tracker);
                }))
            };

            move || drop(interval)
        });
    }

    // Game tick effect
    {
        let state = state.clone();
        let sync_tracker = sync_tracker.clone();

        use_effect_with(state.phase.clone(), move |phase| {
            // Only tick when game is running
            let should_tick = matches!(
                phase,
                P2PPhase::Countdown { .. } | P2PPhase::Running
            );

            let interval: Option<Interval> = if !should_tick {
                None
            } else {
                Some(Interval::new(16, move || {
                    state.dispatch(P2PAction::Tick);

                    // Check if we should send frame hash
                    let frame = state.game_state.current_frame();
                    if frame > 0 && frame % HASH_EXCHANGE_INTERVAL == 0 {
                        let hash = state.game_state.compute_hash();
                        let msg = P2PMessage::FrameHash { frame, hash };
                        state.network.borrow_mut().broadcast(&msg.encode());

                        // Record that we sent the hash
                        sync_tracker.borrow_mut().mark_hash_sent(frame);
                    }
                }))
            };

            move || drop(interval)
        });
    }

    // Render effect - always render when we have a canvas
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let game_state = state.game_state.clone();
        let phase = state.phase.clone();

        use_effect(move || {
            // Render for any phase where canvas is visible
            if !matches!(phase, P2PPhase::Disconnected | P2PPhase::Connecting) {
                // Try to initialize renderer if not done yet
                if renderer_ref.borrow().is_none() {
                    if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                        canvas.set_width(CANVAS_WIDTH);
                        canvas.set_height(CANVAS_HEIGHT);

                        if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                            *renderer_ref.borrow_mut() = Some(renderer);
                        }
                    }
                }

                // Now render
                if let Some(renderer) = renderer_ref.borrow().as_ref() {
                    renderer.render(&game_state);
                }
            }
            || ()
        });
    }

    html! {
        <ContextProvider<P2PStateContext> context={state.clone()}>
            <main class="page debug-p2p-play-page" style="min-height: 100vh; background: #f0f0f0; color: #333;">
                <DesyncWarning />

                <header style="background: #333; color: white; padding: 15px 20px;">
                    <h1 style="margin: 0; font-size: 20px;">{"P2P Multiplayer Game"}</h1>
                </header>

                <div style="display: flex; padding: 20px; gap: 20px; max-width: 1400px; margin: 0 auto;">
                    // Left side: Game canvas or connection panel
                    <div style="flex: 1;">
                        // Connection panel (shown when disconnected)
                        {if matches!(state.phase, P2PPhase::Disconnected | P2PPhase::Connecting) {
                            html! { <ConnectionPanel /> }
                        } else {
                            html! {}
                        }}

                        // Canvas (always the same element, just hidden when disconnected)
                        <div style={if matches!(state.phase, P2PPhase::Disconnected | P2PPhase::Connecting) {
                            "display: none;"
                        } else {
                            "background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
                        }}>
                            <canvas
                                ref={canvas_ref.clone()}
                                width={CANVAS_WIDTH.to_string()}
                                height={CANVAS_HEIGHT.to_string()}
                                style="display: block; max-width: 100%; height: auto; border-radius: 4px;"
                            />
                            {if matches!(state.phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby) {
                                html! {
                                    <div style="text-align: center; padding: 20px; color: #666;">
                                        {"Game preview - waiting in lobby"}
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    </div>

                    // Right side: Control panels
                    <div style="width: 320px; display: flex; flex-direction: column; gap: 15px;">
                        {match state.phase {
                            P2PPhase::Disconnected | P2PPhase::Connecting => {
                                html! {}
                            }
                            P2PPhase::WaitingForPeers | P2PPhase::Lobby => {
                                html! {
                                    <>
                                        <PeerStatusPanel />
                                        <LobbyPanel />
                                        <EventLogPanel />
                                    </>
                                }
                            }
                            _ => {
                                html! {
                                    <>
                                        <PeerStatusPanel />
                                        <GameStatusPanel />
                                        <EventLogPanel />
                                    </>
                                }
                            }
                        }}
                    </div>
                </div>
            </main>
        </ContextProvider<P2PStateContext>>
    }
}

/// Poll the network for events.
fn poll_network(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
) {
    // Check if we need to set our peer ID
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

                // Send our player info to the new peer
                let msg = P2PMessage::PlayerInfo {
                    name: if state.my_name.is_empty() {
                        "Player".to_string()
                    } else {
                        state.my_name.clone()
                    },
                    color: state.my_color,
                };
                state.network.borrow_mut().send_to(peer_id, &msg.encode());

                // Send our ready status
                let ready_msg = P2PMessage::PlayerReady {
                    ready: state.my_ready,
                };
                state.network.borrow_mut().send_to(peer_id, &ready_msg.encode());
            }
            NetworkEvent::PeerLeft(peer_id) => {
                state.dispatch(P2PAction::PeerLeft(peer_id));
                rtt_tracker.borrow_mut().remove_peer(peer_id);
            }
            NetworkEvent::Message { from, data } => {
                handle_message(state, sync_tracker, rtt_tracker, from, &data);
            }
            NetworkEvent::StateChanged(_) => {
                // Connection state changes are handled elsewhere
            }
        }
    }

    // Periodic ping for RTT measurement
    let now = js_sys::Date::now();
    if rtt_tracker.borrow().should_ping(now) {
        for &peer_id in state.peers.keys() {
            let msg = P2PMessage::Ping { timestamp: now };
            state.network.borrow_mut().send_to(peer_id, &msg.encode());
            rtt_tracker.borrow_mut().record_ping_sent(peer_id, now);
        }
    }
}

/// Handle incoming P2P messages.
fn handle_message(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
    from: matchbox_socket::PeerId,
    data: &[u8],
) {
    let Some(msg) = P2PMessage::decode(data) else {
        return;
    };

    match msg {
        P2PMessage::PlayerInfo { name, color } => {
            state.dispatch(P2PAction::UpdatePeerInfo {
                peer_id: from,
                name,
                color,
            });
        }
        P2PMessage::PlayerReady { ready } => {
            state.dispatch(P2PAction::UpdatePeerReady {
                peer_id: from,
                ready,
            });
        }
        P2PMessage::GameStart { seed, players } => {
            // Convert PlayerStartInfo to tuples
            let player_list: Vec<_> = players
                .iter()
                .map(|p| (p.peer_id(), p.name.clone(), p.color))
                .collect();

            state.dispatch(P2PAction::StartGame {
                seed,
                players: player_list,
            });
            state.dispatch(P2PAction::StartCountdown);
        }
        P2PMessage::FrameHash { frame, hash } => {
            // Record the hash from this peer
            sync_tracker.borrow_mut().record_peer_hash(frame, from, hash);
            state.dispatch(P2PAction::ReceiveFrameHash {
                peer_id: from,
                frame,
                hash,
            });

            // Check for desync using majority vote - only when game is running
            if matches!(state.phase, P2PPhase::Running) && !state.desync_detected {
                let my_frame = state.game_state.current_frame();
                let my_hash = state.game_state.compute_hash();

                // Only compare if we're at the same frame
                if frame == my_frame {
                    let connected_peer_count = state.peers.values().filter(|p| p.connected).count();
                    let result = sync_tracker.borrow_mut().compare_hashes(
                        frame,
                        my_hash,
                        connected_peer_count,
                    );

                    use crate::p2p::sync::HashCompareResult;
                    match result {
                        HashCompareResult::Match => {
                            // All good, hashes match
                        }
                        HashCompareResult::Waiting => {
                            // Still waiting for more peers to report
                        }
                        HashCompareResult::Desync { majority_hash } => {
                            // My hash doesn't match the majority
                            state.dispatch(P2PAction::AddLog(format!(
                                "Desync detected at frame {}: mine={:016X}, majority={:016X}",
                                frame, my_hash, majority_hash
                            )));
                            state.dispatch(P2PAction::DetectDesync);
                            state.dispatch(P2PAction::StartResync);

                            // Find a peer with the majority hash to request sync from
                            let sync_source = state.peer_hashes.iter()
                                .find(|&(_, (f, h))| *f == frame && *h == majority_hash)
                                .map(|(peer_id, _)| *peer_id);

                            if let Some(source_peer) = sync_source {
                                let msg = P2PMessage::SyncRequest { from_frame: my_frame };
                                state.network.borrow_mut().send_to(source_peer, &msg.encode());
                                state.dispatch(P2PAction::AddLog(format!(
                                    "Requested sync from peer {} (majority holder)",
                                    &source_peer.0.to_string()[..8]
                                )));
                            }
                        }
                    }
                }
            }
        }
        P2PMessage::SyncRequest { from_frame } => {
            // Someone is requesting sync - respond if we're in the majority
            // (The requester selected us because we have the majority hash)
            // Serialize and send our game state
            let snapshot = state.game_state.create_snapshot();
            match snapshot.to_bytes() {
                Ok(state_data) => {
                    let msg = P2PMessage::SyncState {
                        frame: state.game_state.current_frame(),
                        state: state_data,
                    };
                    state.network.borrow_mut().send_to(from, &msg.encode());
                    state.dispatch(P2PAction::AddLog(format!(
                        "Sent sync state to peer {} (requested from frame {}, current frame {})",
                        &from.0.to_string()[..8],
                        from_frame,
                        state.game_state.current_frame()
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
        P2PMessage::SyncState { frame, state: state_data } => {
            // Received sync state - apply it
            state.dispatch(P2PAction::ApplySyncState {
                frame,
                state_data,
            });
        }
        P2PMessage::Ping { timestamp } => {
            // Respond with pong
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
    }
}
