//! GameView component - main game view with P2P integration.
//!
//! Uses the global BevyProvider (from App.rs) for game rendering and
//! send_command() for game control.

use std::collections::{HashMap, HashSet};

use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_router::prelude::*;

use super::peer_list::ArrivalInfo;
use super::reaction_panel::{REACTION_COOLDOWN_MS, get_reaction_emoji};
use super::room_service::{use_room_service, RoomServiceHandle};
use super::{ChatPanel, PeerList, ReactionDisplay};
use crate::hooks::{
    P2pRoomConfig, PlayerInfo, send_command, use_bevy, use_bevy_chat, use_bevy_game,
    use_bevy_players, use_bevy_reactions, use_config_username,
    use_p2p_room_with_player_id,
};
use crate::routes::Route;

/// Canvas ID for the game view (uses the global canvas from App.rs).
pub use crate::app::BEVY_CANVAS_ID as GAME_VIEW_CANVAS_ID;

/// Game phase - tracks whether we're in lobby or playing
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GamePhase {
    /// Waiting in lobby (map preview visible, no marbles spawned)
    InLobby,
    /// Game is playing (marbles spawned and running)
    Playing,
    /// Game ended — results screen
    Ended,
}

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
/// - Lobby overlay with host controls
///
/// NOTE: The game canvas is managed globally by App.rs to persist across
/// route changes and avoid Bevy's RecreationAttempt error in WASM.
#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    // GameView now uses the global BevyProvider/Canvas from App.rs
    // No local BevyProvider needed
    html! {
        <GameViewInner
            room_id={props.room_id.clone()}
            signaling_url={props.signaling_url.clone()}
            is_host={props.is_host}
        />
    }
}

/// Props for GameViewInner.
#[derive(Properties, PartialEq)]
struct GameViewInnerProps {
    pub room_id: String,
    pub signaling_url: String,
    #[prop_or(false)]
    pub is_host: bool,
}

/// Predefined colors for players.
const PLAYER_COLORS: [[u8; 4]; 8] = [
    [255, 0, 0, 255],   // Red
    [0, 0, 255, 255],   // Blue
    [0, 255, 0, 255],   // Green
    [255, 128, 0, 255], // Orange
    [128, 0, 255, 255], // Purple
    [255, 0, 128, 255], // Pink
    [0, 255, 255, 255], // Cyan
    [255, 255, 0, 255], // Yellow
];

/// Inner component that uses Bevy context and P2P hooks.
#[function_component(GameViewInner)]
fn game_view_inner(props: &GameViewInnerProps) -> Html {
    // Bevy state
    let bevy = use_bevy();
    let bevy_game_state = use_bevy_game();
    let (bevy_players, bevy_arrival_order) = use_bevy_players();

    // User config
    let config_username = use_config_username();

    let player_id = config_username
        .as_ref()
        .map(|x| x.to_string())
        .unwrap_or_default();

    // Room service for peer name resolution
    let room_service = use_room_service();

    // P2P connection
    let config = P2pRoomConfig {
        signaling_url: Some(props.signaling_url.clone()),
        auto_connect: true,
        ..Default::default()
    };

    let p2p = use_p2p_room_with_player_id(&props.room_id, &player_id, config);

    let peers = p2p.peers();
    let connection_state = p2p.state();
    let chat_messages = use_bevy_chat();
    let reactions = use_bevy_reactions();
    let is_connected = matches!(
        connection_state,
        crate::services::p2p::P2pConnectionState::Connected
    );

    // Game phase state - if server already says ENDED, start in Ended directly
    let initial_server_state = room_service.server_room_state();
    let entered_ended = initial_server_state == Some(3);
    let game_phase = use_state(move || {
        if entered_ended {
            GamePhase::Ended
        } else {
            GamePhase::InLobby
        }
    });
    // Stable flag: room was already ended when we joined (never changes)
    let entered_ended = use_mut_ref(move || entered_ended);

    // Host: display_name → user_id mapping (populated at game start, used for arrival reports)
    let player_name_map: std::rc::Rc<std::cell::RefCell<HashMap<String, String>>> =
        use_mut_ref(HashMap::new);

    // Host: track which player names have already been reported as arrived
    let reported_arrivals: std::rc::Rc<std::cell::RefCell<HashSet<String>>> =
        use_mut_ref(HashSet::new);

    // 7단계: 재접속 시 서버 상태 반영
    {
        let game_phase = game_phase.clone();
        let server_state = room_service.server_room_state();
        let entered_ended = entered_ended.clone();
        use_effect_with(server_state, move |state| {
            match state {
                Some(3) => {
                    *entered_ended.borrow_mut() = true;
                    game_phase.set(GamePhase::Ended);
                }
                Some(2) => { /* PLAYING: P2P sync로 처리 */ }
                _ => { /* WAITING: 로비 유지 */ }
            }
        });
    }

    // 8단계A: 호스트 — room_service.is_game_ended() 모니터링
    {
        let game_phase = game_phase.clone();
        let is_ended = room_service.is_game_ended();
        use_effect_with(is_ended, move |ended| {
            if *ended {
                game_phase.set(GamePhase::Ended);
            }
        });
    }

    // 8단계B: Bevy 로컬 상태에서 전원 도착 감지 (호스트 + 비호스트 모두)
    {
        let game_phase = game_phase.clone();
        let all_arrived = !bevy_players.is_empty()
            && bevy_players.iter().all(|p| p.arrived);
        use_effect_with(all_arrived, move |all_arrived| {
            if *all_arrived {
                game_phase.set(GamePhase::Ended);
            }
        });
    }

    // 호스트: Bevy 플레이어 상태를 감시하여 도착 보고 (gRPC ReportArrival)
    // deps를 도착한 플레이어 (name, arrival_frame) 집합으로 한정하여 live_rank 등 변경 시 재실행 방지
    {
        let is_host = props.is_host;
        let room_service = room_service.clone();
        let player_name_map = player_name_map.clone();
        let reported_arrivals = reported_arrivals.clone();

        let arrived_players: Vec<(String, Option<u64>)> = bevy_players
            .iter()
            .filter(|p| p.arrived)
            .map(|p| (p.name.clone(), p.arrival_frame))
            .collect();

        use_effect_with(arrived_players.clone(), move |arrived_players| {
            if !is_host || arrived_players.is_empty() {
                return;
            }

            let name_map = player_name_map.borrow();
            let mut reported = reported_arrivals.borrow_mut();

            for (name, arrival_frame) in arrived_players.iter() {
                if reported.contains(name) {
                    continue;
                }
                reported.insert(name.clone());

                let rank = reported.len() as u32;
                let frame = arrival_frame.unwrap_or(0);

                if let Some(user_id) = name_map.get(name) {
                    room_service.report_arrival(user_id, frame, rank);
                    tracing::info!(
                        player = %name,
                        user_id = %user_id,
                        frame,
                        rank,
                        "Host: reported arrival"
                    );
                } else {
                    tracing::warn!(
                        player = %name,
                        "Host: no user_id mapping found for arrived player"
                    );
                }
            }
        });
    }

    // Share button copied state
    let share_copied = use_state(|| false);

    // Cooldown state for reactions
    let last_reaction_time = use_mut_ref(|| 0.0f64);
    let cooldown_active = use_state(|| false);

    // Last keyboard emoji state (for syncing with ChatPanel's ReactionPanel)
    let last_keyboard_emoji = use_state(|| None::<String>);

    // Non-host: Monitor Bevy game state to detect when GameStart was processed by Bevy P2P sync.
    // The Bevy poll_p2p_socket system handles GameStart directly now.
    {
        let game_phase = game_phase.clone();
        let bevy_initialized = bevy.initialized;
        let is_host = props.is_host;
        let entered_ended = entered_ended.clone();

        use_effect_with(
            (bevy_players.len(), bevy_initialized),
            move |(player_count, initialized)| {
                // Skip if room was already ended when we joined
                if *entered_ended.borrow() {
                    return;
                }
                // When players appear in Bevy state and we're not the host,
                // it means GameStart was processed by Bevy's P2P system
                if !is_host && *initialized && *player_count > 0 {
                    game_phase.set(GamePhase::Playing);
                }
            },
        );
    }

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

            // Send reaction
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

    // Keyboard event handler for reaction shortcuts (1-5)
    {
        let send_reaction = send_reaction.clone();
        let last_keyboard_emoji = last_keyboard_emoji.clone();
        let is_connected = is_connected;

        use_effect_with(is_connected, move |_| {
            let listener = web_sys::window().map(|window| {
                let last_keyboard_emoji = last_keyboard_emoji.clone();
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

                    // Reaction shortcuts (only when connected)
                    if is_connected {
                        if let Some(emoji) = get_reaction_emoji(&key) {
                            last_keyboard_emoji.set(Some(emoji.to_string()));
                            send_reaction(emoji);
                        }
                    }
                })
            });

            move || drop(listener)
        });
    }

    // Start game callback (host only)
    let on_start_game = {
        let game_phase = game_phase.clone();
        let peers = peers.clone();
        let player_id = player_id.clone();
        let bevy_initialized = bevy.initialized;
        let bevy_game_state = bevy_game_state.clone();
        let room_service = room_service.clone();
        let player_name_map = player_name_map.clone();
        let reported_arrivals = reported_arrivals.clone();

        Callback::from(move |_: MouseEvent| {
            if !bevy_initialized {
                tracing::warn!("Bevy not initialized yet");
                return;
            }

            // Generate deterministic seed
            let seed = js_sys::Date::now() as u64;

            // 1. Set sync host status
            let cmd = serde_json::json!({"type": "set_sync_host", "is_host": true});
            if let Err(e) = send_command(&cmd.to_string()) {
                tracing::error!("Failed to set sync host: {:?}", e);
            }

            // 2. Set seed
            let cmd = serde_json::json!({"type": "set_seed", "seed": seed});
            if let Err(e) = send_command(&cmd.to_string()) {
                tracing::error!("Failed to set seed: {:?}", e);
            }

            // 3. Clear existing players
            if let Err(e) = send_command(r#"{"type":"clear_players"}"#) {
                tracing::error!("Failed to clear players: {:?}", e);
            }

            // 4. Build display_name → user_id mapping and clear reported arrivals
            let mut name_map = player_name_map.borrow_mut();
            name_map.clear();
            reported_arrivals.borrow_mut().clear();

            // 5. Add self as first player (use display name)
            let self_color = PLAYER_COLORS[0];
            let my_uid = room_service.player_id();
            let my_name = room_service
                .display_name(&my_uid)
                .unwrap_or_else(|| player_id.clone());
            name_map.insert(my_name.clone(), my_uid.clone());
            let cmd = serde_json::json!({
                "type": "add_player",
                "name": my_name,
                "color": self_color
            });
            if let Err(e) = send_command(&cmd.to_string()) {
                tracing::error!("Failed to add self as player: {:?}", e);
            }

            // 6. Add peers as players — use display name via room_service
            for (i, peer) in peers.iter().enumerate() {
                let peer_id_str = peer.peer_id.to_string();
                // Resolve peer_id → user_id
                let peer_user_id = room_service
                    .player_name(&peer_id_str)
                    .or_else(|| peer.player_id.clone());

                if let Some(ref uid) = peer_user_id {
                    let name = room_service.display_name_or_fallback(uid);
                    name_map.insert(name.clone(), uid.clone());
                    let color = PLAYER_COLORS[(i + 1) % PLAYER_COLORS.len()];
                    let cmd = serde_json::json!({
                        "type": "add_player",
                        "name": name,
                        "color": color
                    });
                    if let Err(e) = send_command(&cmd.to_string()) {
                        tracing::error!("Failed to add peer as player: {:?}", e);
                    }
                }
            }
            drop(name_map);

            // 7. Spawn marbles
            if let Err(e) = send_command(r#"{"type":"spawn_marbles"}"#) {
                tracing::error!("Failed to spawn marbles: {:?}", e);
            }

            // 8. Broadcast game start to peers via Bevy P2P
            if let Err(e) = send_command(r#"{"type":"broadcast_game_start"}"#) {
                tracing::error!("Failed to broadcast game start: {:?}", e);
            }

            // 9. Report game start to server via gRPC
            room_service.start_game(bevy_game_state.frame);

            // 10. Transition to playing phase
            game_phase.set(GamePhase::Playing);
            tracing::info!("Host: Game started with {} players", peers.len() + 1);
        })
    };

    // Share room URL callback
    let on_share = {
        let room_id = props.room_id.clone();
        let share_copied = share_copied.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(window) = web_sys::window() {
                let origin = window.location().origin().unwrap_or_default();
                let url = format!("{}/play/{}", origin, room_id);
                let clipboard = window.navigator().clipboard();
                let _ = clipboard.write_text(&url);
                share_copied.set(true);
                // Reset copied state after 2 seconds
                let share_copied = share_copied.clone();
                gloo::timers::callback::Timeout::new(2000, move || {
                    share_copied.set(false);
                })
                .forget();
            }
        })
    };

    // Calculate reaction cooldown
    let reaction_disabled = *cooldown_active;

    // Build arrival info from Bevy state
    let arrival_info: Vec<ArrivalInfo> = bevy_players
        .iter()
        .map(|player: &PlayerInfo| {
            let arrival_order = bevy_arrival_order
                .iter()
                .position(|&id| id == player.id)
                .map(|pos| (pos + 1) as u32);
            ArrivalInfo {
                player_id: player.name.clone(),
                rank: player.rank,
                arrival_order,
                live_rank: player.live_rank,
            }
        })
        .collect();

    let gamerule = bevy_game_state.gamerule.clone();

    // Determine phases
    let in_lobby = matches!(*game_phase, GamePhase::InLobby);
    let in_ended = matches!(*game_phase, GamePhase::Ended);

    // Helper: resolve peer display name (peer_id → user_id → display_name)
    let resolve_display_name = |peer_id_str: &str, peer: &crate::services::p2p::P2pPeerInfo| -> String {
        if let Some(user_id) = room_service.player_name(peer_id_str) {
            room_service.display_name_or_fallback(&user_id)
        } else {
            peer.player_id
                .as_deref()
                .map(|pid| room_service.display_name_or_fallback(pid))
                .unwrap_or_else(|| format!("Peer-{}", &peer_id_str[..peer_id_str.len().min(8)]))
        }
    };

    // Own display name (prefer cache, fallback to config_username)
    let my_user_id = room_service.player_id();
    let my_display_name = room_service
        .display_name(&my_user_id)
        .unwrap_or_else(|| player_id.clone());

    // Build sorted player list for lobby (host → me → others alphabetically)
    let lobby_player_items = {
        let host_peer_id = p2p.host_peer_id();
        let mut sorted_peers: Vec<_> = peers.iter().collect();
        sorted_peers.sort_by(|a, b| {
            let a_id = a.peer_id.to_string();
            let b_id = b.peer_id.to_string();
            let a_name = resolve_display_name(&a_id, a);
            let b_name = resolve_display_name(&b_id, b);
            a_name.cmp(&b_name)
        });

        let mut items = Vec::new();

        // 1. Host first (if I'm host, show myself)
        if props.is_host {
            items.push(html! {
                <div class="lobby-player-item host me">
                    <span class="lobby-connection-indicator connected">{"\u{25CF}"}</span>
                    <span class="lobby-player-name">{&my_display_name}</span>
                    <span class="lobby-host-badge">{"호스트"}</span>
                    <span class="lobby-me-badge">{"나"}</span>
                </div>
            });
        } else {
            // Find and show host peer first
            if let Some(host_peer) =
                host_peer_id.and_then(|hid| sorted_peers.iter().find(|p| p.peer_id == hid))
            {
                let host_id = host_peer.peer_id.to_string();
                let host_name = resolve_display_name(&host_id, host_peer);
                let conn_class = if host_peer.connected { "connected" } else { "disconnected" };
                let conn_dot = if host_peer.connected { "\u{25CF}" } else { "\u{25CB}" };
                items.push(html! {
                    <div class="lobby-player-item host">
                        <span class={classes!("lobby-connection-indicator", conn_class)}>{conn_dot}</span>
                        <span class="lobby-player-name">{host_name}</span>
                        <span class="lobby-host-badge">{"호스트"}</span>
                    </div>
                });
            }
            // 2. Show myself second (when not host)
            items.push(html! {
                <div class="lobby-player-item me">
                    <span class="lobby-connection-indicator connected">{"\u{25CF}"}</span>
                    <span class="lobby-player-name">{&my_display_name}</span>
                    <span class="lobby-me-badge">{"나"}</span>
                </div>
            });
        }

        // 3. Show remaining peers (alphabetically sorted, excluding host)
        for peer in sorted_peers.iter() {
            // Skip if this is the host peer (already shown)
            if let Some(hid) = host_peer_id {
                if peer.peer_id == hid {
                    continue;
                }
            }
            let peer_id_str = peer.peer_id.to_string();
            let peer_name = resolve_display_name(&peer_id_str, peer);
            let conn_class = if peer.connected { "connected" } else { "disconnected" };
            let conn_dot = if peer.connected { "\u{25CF}" } else { "\u{25CB}" };
            items.push(html! {
                <div class="lobby-player-item">
                    <span class={classes!("lobby-connection-indicator", conn_class)}>{conn_dot}</span>
                    <span class="lobby-player-name">{peer_name}</span>
                </div>
            });
        }

        items.into_iter().collect::<Html>()
    };

    html! {
        <div class="game-view fullscreen">
            // NOTE: Game canvas is now managed globally by App.rs
            // Canvas element is rendered by BevyProvider in the global container

            // Ended: show only results overlay
            if in_ended {
                <GameResultsOverlay
                    room_service={room_service.clone()}
                    bevy_players={bevy_players.clone()}
                    my_player_id={my_user_id.clone()}
                />
            } else {
                // Loading indicator
                if !bevy.initialized {
                    <div class="game-loading">
                        <div class="game-loading__spinner"></div>
                        <p>{"게임 로딩 중..."}</p>
                    </div>
                }

                // Lobby overlay (when in lobby phase)
                if in_lobby && bevy.initialized {
                    <div class="lobby-overlay">
                        <div class="lobby-panel">
                            <h2 class="lobby-title">{"대기실"}</h2>

                            // Room info with share button
                            <div class="lobby-room-info">
                                <span class="lobby-room-id">{&props.room_id}</span>
                                <button
                                    class={classes!("lobby-share-btn", share_copied.then_some("copied"))}
                                    onclick={on_share.clone()}
                                    title="URL 복사"
                                >
                                    if *share_copied {
                                        {"복사됨!"}
                                    } else {
                                        {"공유"}
                                    }
                                </button>
                            </div>

                            // Player list in lobby (sorted: host → me → others alphabetically)
                            <div class="lobby-players">
                                <div class="lobby-players-header">
                                    {format!("접속된 플레이어 ({}명)", peers.len() + 1)}
                                </div>
                                <div class="lobby-player-list">
                                    {lobby_player_items.clone()}
                                </div>
                            </div>

                            // Start button (host) or waiting message (non-host)
                            <div class="lobby-actions">
                                if props.is_host {
                                    if matches!(room_service.server_room_state(), Some(2) | Some(3)) {
                                        // 게임이 이미 시작/종료된 상태
                                        <p class="lobby-connecting">{"게임이 이미 진행/종료된 상태입니다. 재연결 대기 중..."}</p>
                                    } else if is_connected {
                                        <button
                                            class="lobby-start-btn"
                                            onclick={on_start_game}
                                        >
                                            {format!("게임 시작 ({}명)", peers.len() + 1)}
                                        </button>
                                    } else {
                                        <p class="lobby-connecting">{"서버에 연결 중..."}</p>
                                    }
                                } else {
                                    <p class="lobby-waiting">{"호스트가 게임을 시작할 때까지 대기 중..."}</p>
                                }
                            </div>
                        </div>
                    </div>
                }

                // Left side: Peer list (only when playing)
                if !in_lobby {
                    <PeerList
                        peers={peers.clone()}
                        my_player_id={my_display_name.clone()}
                        connection_state={connection_state.clone()}
                        arrival_info={arrival_info}
                        gamerule={gamerule}
                    />
                }

                // Floating emoji reactions
                <ReactionDisplay reactions={reactions} />

                // Bottom-right: Chat panel with integrated reactions
                <ChatPanel
                    p2p={p2p}
                    is_connected={is_connected}
                    messages={chat_messages}
                    my_player_id={player_id}
                    on_reaction_send={on_reaction_send}
                    reaction_disabled={reaction_disabled}
                    last_keyboard_emoji={(*last_keyboard_emoji).clone()}
                />
            }
        </div>
    }
}

// ---------------------------------------------------------------------------
// GameResultsOverlay component
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
struct GameResultsOverlayProps {
    room_service: RoomServiceHandle,
    bevy_players: Vec<PlayerInfo>,
    my_player_id: String,
}

#[function_component(GameResultsOverlay)]
fn game_results_overlay(props: &GameResultsOverlayProps) -> Html {
    let navigator = use_navigator().expect("Navigator not found");

    let server_results = props.room_service.game_results();

    // Build rankings: prefer server results, fallback to Bevy local state
    let rankings: Vec<(u32, String, Option<[u8; 4]>, bool)> = if !server_results.is_empty() {
        // Server results available — use them (sorted by rank)
        server_results
            .iter()
            .map(|r| {
                let name = props.room_service.display_name_or_fallback(&r.user_id);
                let is_me = r.user_id == props.my_player_id;
                // Try to find color from Bevy players by matching display name
                let color = props
                    .bevy_players
                    .iter()
                    .find(|bp| {
                        bp.name == name
                            || props.room_service.display_name(&r.user_id).as_deref() == Some(&bp.name)
                    })
                    .map(|bp| bp.color);
                (r.rank, name, color, is_me)
            })
            .collect()
    } else {
        // No server results — build from Bevy local state (arrival order)
        let mut players_with_rank: Vec<_> = props
            .bevy_players
            .iter()
            .filter(|p| p.arrived)
            .collect();
        players_with_rank.sort_by_key(|p| p.rank.unwrap_or(u32::MAX));

        players_with_rank
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let rank = p.rank.unwrap_or((i + 1) as u32);
                let is_me = props.room_service
                    .display_name(&props.my_player_id)
                    .map(|dn| dn == p.name)
                    .unwrap_or(false);
                (rank, p.name.clone(), Some(p.color), is_me)
            })
            .collect()
    };

    let on_leave = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Home);
        })
    };

    html! {
        <div class="winner-modal-overlay">
            <div class="winner-modal">
                <div class="winner-modal-header">
                    {"GAME OVER"}
                </div>

                <div class="winner-modal-content">
                    // Winner announcement (1st place)
                    { if let Some((_, name, color, is_me)) = rankings.first() {
                        html! {
                            <div class="winner-announcement">
                                { if let Some(c) = color {
                                    html! {
                                        <span
                                            class="winner-color"
                                            style={format!("background: rgb({}, {}, {});", c[0], c[1], c[2])}
                                        />
                                    }
                                } else {
                                    html! {}
                                }}
                                <span class="winner-name">
                                    {name}
                                    { if *is_me {
                                        html! { <span class="tag you">{"YOU"}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </span>
                                <span class="winner-text">{" Wins!"}</span>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="winner-announcement">
                                {"No Winner"}
                            </div>
                        }
                    }}

                    // Rankings
                    <div class="rankings-section">
                        <div class="rankings-header">
                            {"Final Rankings"}
                        </div>
                        <div class="rankings-list">
                            { for rankings.iter().map(|(rank, name, color, is_me)| {
                                let medal = match rank {
                                    1 => "1st",
                                    2 => "2nd",
                                    3 => "3rd",
                                    n => return html! {
                                        <div class="rankings-item">
                                            <span class="rankings-rank">{format!("{}th", n)}</span>
                                            { if let Some(c) = color {
                                                html! {
                                                    <span
                                                        class="rankings-color"
                                                        style={format!("background: rgb({}, {}, {});", c[0], c[1], c[2])}
                                                    />
                                                }
                                            } else {
                                                html! { <span class="rankings-color" /> }
                                            }}
                                            <span class="rankings-name">
                                                {name}
                                                { if *is_me {
                                                    html! { <span class="tag you">{"YOU"}</span> }
                                                } else {
                                                    html! {}
                                                }}
                                            </span>
                                        </div>
                                    },
                                };

                                let is_winner = *rank == 1;

                                html! {
                                    <div class={classes!(
                                        "rankings-item",
                                        is_winner.then_some("winner")
                                    )}>
                                        <span class="rankings-rank">{medal}</span>
                                        { if let Some(c) = color {
                                            html! {
                                                <span
                                                    class="rankings-color"
                                                    style={format!("background: rgb({}, {}, {});", c[0], c[1], c[2])}
                                                />
                                            }
                                        } else {
                                            html! { <span class="rankings-color" /> }
                                        }}
                                        <span class="rankings-name">
                                            {name}
                                            { if *is_me {
                                                html! { <span class="tag you">{"YOU"}</span> }
                                            } else {
                                                html! {}
                                            }}
                                        </span>
                                    </div>
                                }
                            })}
                        </div>
                    </div>
                </div>

                <div class="winner-modal-actions">
                    <button
                        class="btn leave-btn"
                        onclick={on_leave}
                    >
                        {"나가기"}
                    </button>
                </div>
            </div>
        </div>
    }
}
