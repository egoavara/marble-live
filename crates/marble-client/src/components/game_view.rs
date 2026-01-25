//! GameView component - main game view with P2P integration.

use gloo::events::EventListener;
use marble_proto::play::p2p_message::Payload;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use super::peer_list::ArrivalInfo;
use super::reaction_panel::{get_reaction_emoji, REACTION_COOLDOWN_MS};
use super::{CameraControls, ChatPanel, PeerList, ReactionDisplay};
use crate::camera::CameraMode;
use crate::ranking::LiveRankingTracker;
use crate::hooks::{
    use_config_secret, use_config_username, use_game_loop, use_localstorage,
    use_p2p_room_with_credentials, P2pRoomConfig,
};

/// Props for the GameView component.
#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    /// Room ID to connect to
    pub room_id: String,
    /// Signaling server URL
    pub signaling_url: String,
    /// Is this player the host?
    #[prop_or(false)]
    pub is_host: bool,
}

/// GameView component - P2P-enabled game view with overlays.
///
/// Contains:
/// - PeerList (left side, vertically centered)
/// - ReactionDisplay (floating emojis)
/// - ChatPanel with integrated reactions (bottom-right corner)
/// - Game canvas with physics rendering
/// - Host controls (start button)
#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let config_username = use_config_username();
    let config_secret = use_config_secret();

    let player_id = config_username
        .as_ref()
        .map(|x| x.to_string())
        .unwrap_or_default();
    let player_secret = config_secret.to_string();

    let config = P2pRoomConfig {
        signaling_url: Some(props.signaling_url.clone()),
        auto_connect: true,
        ..Default::default()
    };

    let p2p = use_p2p_room_with_credentials(&props.room_id, &player_id, &player_secret, config);

    let peers = p2p.peers();
    let connection_state = p2p.state();
    let messages = p2p.messages();
    let is_connected =
        matches!(connection_state, crate::services::p2p::P2pConnectionState::Connected);

    // Canvas reference
    let canvas_ref = use_node_ref();

    // Camera mode from localStorage
    let camera_mode_storage = use_localstorage("marble-live-camera-mode", || CameraMode::Overview);

    // Game loop hook with initial camera mode
    let game_seed = use_state(|| js_sys::Date::now() as u64);
    let game_loop = use_game_loop(&p2p, canvas_ref.clone(), props.is_host, *game_seed, *camera_mode_storage);

    // Current camera mode for UI (trigger re-render)
    let current_camera_mode = use_state(|| *camera_mode_storage);

    // Callback for camera mode change
    let on_camera_mode_change = {
        let camera_mode_storage = camera_mode_storage.clone();
        let current_camera_mode = current_camera_mode.clone();
        Callback::from(move |mode: CameraMode| {
            camera_mode_storage.set(mode);
            current_camera_mode.set(mode);
        })
    };

    // Track last processed session version (instead of boolean flag)
    let last_processed_session = use_mut_ref(|| 0u64);
    // Track if host has started initial game
    let host_started = use_mut_ref(|| false);
    // Track previous peer count for detecting new peer joins
    let prev_peer_count = use_mut_ref(|| 0usize);

    // Live ranking tracker with hysteresis (300ms cooldown + 30px margin)
    let live_ranking_tracker = use_mut_ref(LiveRankingTracker::new);

    // Auto-start game when host connects
    {
        let game_loop = game_loop.clone();
        let is_host = props.is_host;
        let host_started = host_started.clone();
        use_effect_with(is_connected, move |is_connected| {
            if is_host && *is_connected && !*host_started.borrow() {
                *host_started.borrow_mut() = true;
                game_loop.start_game();
            }
        });
    }

    // Host: resend GameStart when new peers join (so they can start simulation)
    {
        let game_loop = game_loop.clone();
        let is_host = props.is_host;
        let p2p = p2p.clone();
        let prev_peer_count = prev_peer_count.clone();
        let peers_count = peers.len();
        use_effect_with(peers_count, move |&current_count| {
            if !is_host {
                return;
            }

            let prev = *prev_peer_count.borrow();
            *prev_peer_count.borrow_mut() = current_count;

            // If new peers joined and game is running, resend current state
            if current_count > prev && game_loop.is_running() {
                let game_state = game_loop.game_state.borrow();
                let gamerule = game_state.gamerule().to_string();
                let snapshot = game_state.create_snapshot();
                if let Ok(state_bytes) = snapshot.to_bytes() {
                    drop(game_state);
                    p2p.send_game_start(snapshot.rng_seed, state_bytes, gamerule);
                    tracing::info!("Resent GameStart to new peers (peer count: {} -> {})", prev, current_count);
                }
            }
        });
    }

    // Process GameStart messages for non-host
    // Uses session_version to detect new game starts (including respawns)
    {
        let game_loop = game_loop.clone();
        let is_host = props.is_host;
        let last_processed_session = last_processed_session.clone();
        let messages_for_game_start = messages.clone();
        use_effect_with(messages.len(), move |_| {
            if is_host {
                return;
            }

            // Look for the latest GameStart message with higher session version
            for msg in messages_for_game_start.iter().rev() {
                if let Payload::GameStart(game_start) = &msg.payload {
                    // Only process if session version is newer
                    if game_start.session_version > *last_processed_session.borrow() {
                        *last_processed_session.borrow_mut() = game_start.session_version;
                        game_loop.init_from_game_start(game_start.seed, &game_start.initial_state, &game_start.gamerule);
                    }
                    break;
                }
            }
        });
    }

    // Cooldown state - last reaction timestamp
    let last_reaction_time = use_mut_ref(|| 0.0f64);
    let cooldown_active = use_state(|| false);

    // Last keyboard emoji state (for syncing with ChatPanel's ReactionPanel)
    let last_keyboard_emoji = use_state(|| None::<String>);

    // Helper: send reaction with cooldown check
    let send_reaction = {
        let p2p = p2p.clone();
        let last_reaction_time = last_reaction_time.clone();
        let cooldown_active = cooldown_active.clone();
        move |emoji: &str| {
            let now = js_sys::Date::now();
            let last = *last_reaction_time.borrow();

            if now - last < REACTION_COOLDOWN_MS {
                return; // On cooldown
            }

            // Update last reaction time
            *last_reaction_time.borrow_mut() = now;
            cooldown_active.set(true);

            // Send reaction (save_last_emoji is handled by ChatPanel)
            p2p.send_reaction(emoji);

            // Schedule cooldown end
            let cooldown_active = cooldown_active.clone();
            gloo::timers::callback::Timeout::new(REACTION_COOLDOWN_MS as u32, move || {
                cooldown_active.set(false);
            })
            .forget();
        }
    };

    // Callback for ChatPanel's reaction send
    let on_reaction_send = {
        let send_reaction = send_reaction.clone();
        Callback::from(move |emoji: String| {
            send_reaction(&emoji);
        })
    };

    // Keyboard event handler for reaction shortcuts (1-5) and camera controls (Q/W/E)
    {
        let send_reaction = send_reaction.clone();
        let last_keyboard_emoji = last_keyboard_emoji.clone();
        let is_connected = is_connected;
        let camera_state = game_loop.camera();
        let on_camera_mode_change = on_camera_mode_change.clone();
        use_effect_with(is_connected, move |_| {
            let listener = web_sys::window().map(|window| {
                let last_keyboard_emoji = last_keyboard_emoji.clone();
                let camera_state = camera_state.clone();
                let on_camera_mode_change = on_camera_mode_change.clone();
                EventListener::new(&window, "keydown", move |event| {
                    let keyboard_event = match event.dyn_ref::<web_sys::KeyboardEvent>() {
                        Some(e) => e,
                        None => return,
                    };

                    // Don't handle if typing in input field
                    if let Some(target) = keyboard_event.target() {
                        if let Some(element) = target.dyn_ref::<web_sys::HtmlElement>() {
                            let tag_name = element.tag_name().to_lowercase();
                            if tag_name == "input" || tag_name == "textarea" {
                                return;
                            }
                        }
                    }

                    let key = keyboard_event.key().to_lowercase();

                    // Camera controls (Q/W/E)
                    match key.as_str() {
                        "q" => {
                            camera_state.borrow_mut().set_mode(CameraMode::FollowMe);
                            on_camera_mode_change.emit(CameraMode::FollowMe);
                            return;
                        }
                        "w" => {
                            camera_state.borrow_mut().set_mode(CameraMode::FollowLeader);
                            on_camera_mode_change.emit(CameraMode::FollowLeader);
                            return;
                        }
                        "e" => {
                            camera_state.borrow_mut().set_mode(CameraMode::Overview);
                            on_camera_mode_change.emit(CameraMode::Overview);
                            return;
                        }
                        _ => {}
                    }

                    // Reaction shortcuts (only when connected)
                    if is_connected {
                        if let Some(emoji) = get_reaction_emoji(&key) {
                            // Update last_keyboard_emoji for ChatPanel sync
                            last_keyboard_emoji.set(Some(emoji.to_string()));
                            send_reaction(emoji);
                        }
                    }
                })
            });

            // Keep listener alive until cleanup
            move || drop(listener)
        });
    }

    // Calculate if reactions are on cooldown
    let reaction_disabled = *cooldown_active;

    // Calculate arrival info from game state (before html! macro)
    let (arrival_info, gamerule) = {
        let game_state = game_loop.game_state.borrow();
        let mut tracker = live_ranking_tracker.borrow_mut();

        // 1. 쿨타임 틱
        tracker.tick();

        let leaderboard = game_state.leaderboard();
        let gamerule = game_state.gamerule().to_string();
        let arrival_order_list = game_state.arrival_order();

        // 2. 미도착 플레이어 위치 수집
        let non_arrived_positions: Vec<(marble_core::marble::PlayerId, f32)> = game_state.players.iter()
            .filter(|p| !arrival_order_list.contains(&p.id))
            .filter_map(|p| {
                game_state.marble_manager.get_marble_by_owner(p.id).and_then(|marble| {
                    if marble.eliminated {
                        None
                    } else {
                        game_state.physics_world.get_rigid_body(marble.body_handle).map(|body| {
                            let pos = body.translation();
                            let score = game_state.calculate_ranking_score((pos.x, pos.y));
                            (p.id, score)
                        })
                    }
                })
            })
            .collect();

        // 3. 히스테리시스 적용된 순위 획득
        let live_rankings = tracker.update(&non_arrived_positions);

        // 4. live_rank map 생성
        let live_rank_map: std::collections::HashMap<marble_core::marble::PlayerId, u32> = live_rankings
            .iter()
            .copied()
            .collect();

        // Build arrival info: map player_id (u32) to name and rank
        let arrival_info: Vec<ArrivalInfo> = game_state.players.iter().map(|player| {
            let rank = leaderboard.iter()
                .position(|&pid| pid == player.id)
                .map(|pos| (pos + 1) as u32);
            let arrival_order = arrival_order_list
                .iter()
                .position(|&pid| pid == player.id)
                .map(|pos| (pos + 1) as u32);
            let live_rank = live_rank_map.get(&player.id).copied();
            ArrivalInfo {
                player_id: player.name.clone(),
                rank,
                arrival_order,
                live_rank,
            }
        }).collect();

        (arrival_info, gamerule)
    };

    html! {
        <div class="game-view fullscreen">
            // Game canvas
            <canvas
                ref={canvas_ref}
                class="game-canvas"
                width="800"
                height="600"
            />

            // Camera controls (top-center)
            <CameraControls
                camera_state={game_loop.camera()}
                current_mode={*current_camera_mode}
                on_mode_change={on_camera_mode_change.clone()}
            />

            // Spawn button (host only, disabled after first spawn)
            if props.is_host && is_connected {
                <div class="spawn-controls">
                    if game_loop.is_spawned() {
                        <button class="spawn-btn spawned" disabled={true}>
                            { "게임 진행 중" }
                        </button>
                    } else {
                        <button
                            class="spawn-btn"
                            onclick={
                                let game_loop = game_loop.clone();
                                Callback::from(move |_: MouseEvent| {
                                    game_loop.spawn_marbles();
                                })
                            }
                        >
                            { format!("스폰 ({}명)", peers.len() + 1) }
                        </button>
                    }
                </div>
            }

            // Left side: Peer list (vertically centered)
            <PeerList
                peers={peers.clone()}
                my_player_id={player_id.clone()}
                connection_state={connection_state.clone()}
                arrival_info={arrival_info}
                gamerule={gamerule}
            />

            // Floating emoji reactions
            <ReactionDisplay messages={messages.clone()} />

            // Bottom-right: Chat panel with integrated reactions
            <ChatPanel
                p2p={p2p}
                is_connected={is_connected}
                messages={messages}
                my_player_id={player_id}
                on_reaction_send={on_reaction_send}
                reaction_disabled={reaction_disabled}
                last_keyboard_emoji={(*last_keyboard_emoji).clone()}
            />
        </div>
    }
}
