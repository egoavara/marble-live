//! Play page with P2P multiplayer game.

use crate::components::{
    CanvasControls, DebugLogToggle, DesyncWarning, Layout, Leaderboard, PlayerLegend, ShareButton,
    WinnerModal,
};
use crate::fingerprint::{generate_hash_code, get_browser_fingerprint};
use crate::hooks::use_room_sync;
use crate::network::NetworkEvent;
use crate::p2p::protocol::P2PMessage;
use crate::p2p::state::{P2PAction, P2PGameState, P2PPhase, P2PStateContext};
use crate::p2p::sync::{RttTracker, SyncTracker, HASH_EXCHANGE_INTERVAL};
use crate::renderer::CanvasRenderer;
use crate::storage::UserSettings;
use gloo::timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Canvas dimensions.
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

/// Props for the PlayPage component.
#[derive(Properties, PartialEq)]
pub struct PlayPageProps {
    pub room_id: String,
}

/// Play page component with P2P multiplayer.
#[function_component(PlayPage)]
pub fn play_page(props: &PlayPageProps) -> Html {
    let room_id = props.room_id.clone();
    let state = use_reducer(P2PGameState::new);
    let canvas_ref = use_node_ref();
    let renderer_ref = use_mut_ref(|| None::<CanvasRenderer>);
    let sync_tracker = use_mut_ref(SyncTracker::new);
    let rtt_tracker = use_mut_ref(RttTracker::new);
    let connection_attempted = use_state(|| false);

    // Load user settings and apply them
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let settings = UserSettings::load().unwrap_or_default();
            state.dispatch(P2PAction::SetMyName(settings.name.clone()));
            state.dispatch(P2PAction::SetMyColor(settings.color));
            // Generate and set hash code
            let hash_code = generate_hash_code(&settings.name);
            state.dispatch(P2PAction::SetMyHashCode(hash_code));
            || ()
        });
    }

    // Auto-connect to room when component mounts
    {
        let state = state.clone();
        let room_id = room_id.clone();
        let connection_attempted = connection_attempted.clone();

        use_effect_with(connection_attempted.clone(), move |attempted| {
            if !**attempted {
                let settings = UserSettings::load().unwrap_or_default();
                let name = if settings.name.is_empty() {
                    "Player".to_string()
                } else {
                    settings.display_name()
                };
                let fingerprint = get_browser_fingerprint();
                let color = settings.color;

                connection_attempted.set(true);
                state.dispatch(P2PAction::SetConnecting);

                let network = state.network.clone();
                let state_clone = state.clone();
                let room_id_clone = room_id.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let result = network.borrow_mut().join_room(&room_id_clone, &name, &fingerprint, color).await;

                    match result {
                        Ok((seed, is_game_in_progress, player_id, is_host, server_players)) => {
                            state_clone.dispatch(P2PAction::SetConnected {
                                room_id: room_id_clone,
                                server_seed: seed,
                                is_game_in_progress,
                                player_id,
                                is_host,
                            });
                            state_clone.dispatch(P2PAction::UpdateServerPlayers(server_players));
                        }
                        Err(_) => {
                            match network
                                .borrow_mut()
                                .create_and_join_room("Marble Race", &name, &fingerprint, color)
                                .await
                            {
                                Ok((created_room_id, seed, is_game_in_progress, player_id, is_host, server_players)) => {
                                    state_clone.dispatch(P2PAction::SetConnected {
                                        room_id: created_room_id,
                                        server_seed: seed,
                                        is_game_in_progress,
                                        player_id,
                                        is_host,
                                    });
                                    state_clone.dispatch(P2PAction::UpdateServerPlayers(server_players));
                                }
                                Err(e) => {
                                    state_clone.dispatch(P2PAction::SetError(e));
                                    state_clone.dispatch(P2PAction::SetDisconnected);
                                }
                            }
                        }
                    }
                });
            }

            || ()
        });
    }

    // Poll GetRoom periodically to sync server_players during lobby
    use_room_sync(&state);

    // Initialize canvas and renderer
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let phase = state.phase.clone();

        use_effect_with(phase, move |_phase| {
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
            // Also poll during Reconnecting phase to receive ReconnectResponse
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

    // Game tick effect
    {
        let state = state.clone();
        let sync_tracker = sync_tracker.clone();

        use_effect_with(state.phase.clone(), move |phase| {
            let should_tick = matches!(
                phase,
                P2PPhase::Countdown { .. } | P2PPhase::Running
            );

            let interval: Option<Interval> = if !should_tick {
                None
            } else {
                Some(Interval::new(16, move || {
                    state.dispatch(P2PAction::Tick);

                    let frame = state.game_state.current_frame();
                    if frame > 0 && frame % HASH_EXCHANGE_INTERVAL == 0 {
                        let hash = state.game_state.compute_hash();
                        let msg = P2PMessage::FrameHash { frame, hash };
                        state.network.borrow_mut().broadcast(&msg.encode());
                        sync_tracker.borrow_mut().mark_hash_sent(frame);
                    }
                }))
            };

            move || drop(interval)
        });
    }

    // Render effect
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let game_state = state.game_state.clone();
        let phase = state.phase.clone();

        use_effect(move || {
            if !matches!(phase, P2PPhase::Disconnected | P2PPhase::Connecting) {
                if renderer_ref.borrow().is_none() {
                    if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                        canvas.set_width(CANVAS_WIDTH);
                        canvas.set_height(CANVAS_HEIGHT);

                        if let Ok(renderer) = CanvasRenderer::new(&canvas) {
                            *renderer_ref.borrow_mut() = Some(renderer);
                        }
                    }
                }

                if let Some(renderer) = renderer_ref.borrow().as_ref() {
                    renderer.render(&game_state);
                }
            }
            || ()
        });
    }

    let is_connecting = matches!(state.phase, P2PPhase::Connecting);
    let show_canvas = !matches!(state.phase, P2PPhase::Disconnected | P2PPhase::Connecting);
    let is_in_lobby = matches!(state.phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby);
    let is_in_gameplay = matches!(
        state.phase,
        P2PPhase::Countdown { .. } | P2PPhase::Running | P2PPhase::Finished
    );
    let is_finished = matches!(state.phase, P2PPhase::Finished);

    html! {
        <ContextProvider<P2PStateContext> context={state.clone()}>
            <Layout>
                <div class="game-fullscreen">
                    <DesyncWarning />

                    // Connecting state overlay
                    { if is_connecting {
                        html! {
                            <div class="connecting-overlay fullscreen">
                                <div class="connecting-spinner" />
                                <p>{ "Connecting to room..." }</p>
                                <p class="room-id-text">{ format!("Room: {}", room_id) }</p>
                            </div>
                        }
                    } else if !show_canvas {
                        html! {
                            <div class="error-overlay fullscreen">
                                <p>{ "Failed to connect" }</p>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    // Fullscreen game canvas
                    <div class={classes!("game-canvas-container", (!show_canvas).then_some("hidden"))}>
                        <canvas
                            ref={canvas_ref.clone()}
                            width={CANVAS_WIDTH.to_string()}
                            height={CANVAS_HEIGHT.to_string()}
                            class="game-canvas fullscreen"
                        />

                        // Left sidebar: PlayerLegend during gameplay, Leaderboard during lobby
                        { if show_canvas {
                            if is_in_gameplay {
                                html! { <PlayerLegend /> }
                            } else {
                                html! { <Leaderboard /> }
                            }
                        } else {
                            html! {}
                        }}

                        // Top-left: Share button
                        { if show_canvas && !state.room_id.is_empty() {
                            html! {
                                <div class="top-left-controls">
                                    <ShareButton />
                                </div>
                            }
                        } else {
                            html! {}
                        }}

                        // Center overlay: Ready/Start (only in lobby, WinnerModal handles finished)
                        { if is_in_lobby {
                            html! { <CanvasControls /> }
                        } else {
                            html! {}
                        }}

                        // Winner modal when game is finished
                        { if is_finished {
                            html! { <WinnerModal /> }
                        } else {
                            html! {}
                        }}
                    </div>

                    // Debug log toggle (bottom-right)
                    { if show_canvas {
                        html! { <DebugLogToggle /> }
                    } else {
                        html! {}
                    }}
                </div>
            </Layout>
        </ContextProvider<P2PStateContext>>
    }
}

/// Poll the network for events.
fn poll_network(
    state: &P2PStateContext,
    sync_tracker: &Rc<RefCell<SyncTracker>>,
    rtt_tracker: &Rc<RefCell<RttTracker>>,
) {
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
            // Use explicit player order from host for game start
            state.dispatch(P2PAction::StartGameFromServer { seed, player_order });
            state.dispatch(P2PAction::StartCountdown);
        }
        P2PMessage::FrameHash { frame, hash } => {
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

                    use crate::p2p::sync::HashCompareResult;
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
        P2PMessage::SyncRequest { from_frame } => {
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
            // Another player is reconnecting and requesting game state
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
                        use crate::p2p::protocol::PlayerStartInfo;
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
        P2PMessage::ReconnectResponse { seed, frame, state: state_data, players } => {
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
    }
}
