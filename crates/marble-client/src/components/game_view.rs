//! GameView component - main game view with P2P integration.
//!
//! Uses the global BevyProvider (from App.rs) for game rendering and
//! send_command() for game control.

use gloo::events::EventListener;
use marble_proto::play::p2p_message::Payload;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;

use super::peer_list::ArrivalInfo;
use super::reaction_panel::{get_reaction_emoji, REACTION_COOLDOWN_MS};
use super::{ChatPanel, PeerList, ReactionDisplay};
use crate::hooks::{
    send_command, use_bevy, use_bevy_game, use_bevy_players,
    use_config_secret, use_config_username,
    use_p2p_room_with_credentials, P2pRoomConfig,
    PlayerInfo,
};

/// Canvas ID for the game view (uses the global canvas from App.rs).
pub use crate::app::BEVY_CANVAS_ID as GAME_VIEW_CANVAS_ID;

/// Game phase - tracks whether we're in lobby or playing
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GamePhase {
    /// Waiting in lobby (map preview visible, no marbles spawned)
    InLobby,
    /// Game is playing (marbles spawned and running)
    Playing,
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
    [255, 0, 0, 255],     // Red
    [0, 0, 255, 255],     // Blue
    [0, 255, 0, 255],     // Green
    [255, 128, 0, 255],   // Orange
    [128, 0, 255, 255],   // Purple
    [255, 0, 128, 255],   // Pink
    [0, 255, 255, 255],   // Cyan
    [255, 255, 0, 255],   // Yellow
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
    let config_secret = use_config_secret();

    let player_id = config_username
        .as_ref()
        .map(|x| x.to_string())
        .unwrap_or_default();
    let player_secret = config_secret.to_string();

    // P2P connection
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

    // Game phase state - start in lobby
    let game_phase = use_state(|| GamePhase::InLobby);

    // Share button copied state
    let share_copied = use_state(|| false);

    // Track last processed session version for non-host
    let last_processed_session = use_mut_ref(|| 0u64);

    // Cooldown state for reactions
    let last_reaction_time = use_mut_ref(|| 0.0f64);
    let cooldown_active = use_state(|| false);

    // Last keyboard emoji state (for syncing with ChatPanel's ReactionPanel)
    let last_keyboard_emoji = use_state(|| None::<String>);

    // Process GameStart messages for non-host
    {
        let is_host = props.is_host;
        let last_processed_session = last_processed_session.clone();
        let messages_for_game_start = messages.clone();
        let game_phase = game_phase.clone();
        let bevy_initialized = bevy.initialized;

        use_effect_with((messages.len(), bevy_initialized), move |(_, initialized)| {
            if is_host || !*initialized {
                return;
            }

            // Look for the latest GameStart message with higher session version
            for msg in messages_for_game_start.iter().rev() {
                if let Payload::GameStart(game_start) = &msg.payload {
                    // Only process if session version is newer
                    if game_start.session_version > *last_processed_session.borrow() {
                        *last_processed_session.borrow_mut() = game_start.session_version;

                        // Send restore command to Bevy with the game state
                        let cmd = serde_json::json!({
                            "type": "restore_game_state",
                            "seed": game_start.seed,
                            "state": game_start.initial_state,
                            "gamerule": game_start.gamerule
                        });
                        if let Err(e) = send_command(&cmd.to_string()) {
                            tracing::error!("Failed to restore game state: {:?}", e);
                        }

                        // Transition to playing phase
                        game_phase.set(GamePhase::Playing);
                        tracing::info!("Non-host: Game started from host (session {})", game_start.session_version);
                    }
                    break;
                }
            }
        });
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
        let p2p = p2p.clone();
        let peers = peers.clone();
        let player_id = player_id.clone();
        let bevy_initialized = bevy.initialized;

        Callback::from(move |_: MouseEvent| {
            if !bevy_initialized {
                tracing::warn!("Bevy not initialized yet");
                return;
            }

            // 1. Clear existing players
            if let Err(e) = send_command(r#"{"type":"clear_players"}"#) {
                tracing::error!("Failed to clear players: {:?}", e);
            }

            // 2. Add self as first player
            let self_color = PLAYER_COLORS[0];
            let cmd = serde_json::json!({
                "type": "add_player",
                "name": player_id,
                "color": self_color
            });
            if let Err(e) = send_command(&cmd.to_string()) {
                tracing::error!("Failed to add self as player: {:?}", e);
            }

            // 3. Add peers as players
            for (i, peer) in peers.iter().enumerate() {
                if let Some(peer_player_id) = &peer.player_id {
                    let color = PLAYER_COLORS[(i + 1) % PLAYER_COLORS.len()];
                    let cmd = serde_json::json!({
                        "type": "add_player",
                        "name": peer_player_id,
                        "color": color
                    });
                    if let Err(e) = send_command(&cmd.to_string()) {
                        tracing::error!("Failed to add peer as player: {:?}", e);
                    }
                }
            }

            // 4. Spawn marbles
            if let Err(e) = send_command(r#"{"type":"spawn_marbles"}"#) {
                tracing::error!("Failed to spawn marbles: {:?}", e);
            }

            // 5. Get game state and broadcast to peers
            // TODO: Need to get snapshot from Bevy and send via P2P
            // For now, we'll send a simplified message
            let seed = js_sys::Date::now() as u64;
            let player_names: Vec<String> = std::iter::once(player_id.clone())
                .chain(peers.iter().filter_map(|p| p.player_id.clone()))
                .collect();

            // Send GameStart with player list (peers will reconstruct)
            let state_json = serde_json::json!({
                "players": player_names,
                "colors": PLAYER_COLORS[..player_names.len().min(8)].to_vec()
            });
            if let Ok(state_bytes) = serde_json::to_vec(&state_json) {
                p2p.send_game_start(seed, state_bytes, String::new());
            }

            // 6. Transition to playing phase
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
    let arrival_info: Vec<ArrivalInfo> = bevy_players.iter().map(|player: &PlayerInfo| {
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
    }).collect();

    let gamerule = bevy_game_state.gamerule.clone();

    // Determine if in lobby phase
    let in_lobby = matches!(*game_phase, GamePhase::InLobby);

    // Build sorted player list for lobby (host → me → others alphabetically)
    let lobby_player_items = {
        let host_peer_id = p2p.host_peer_id();
        let mut sorted_peers: Vec<_> = peers.iter().collect();
        sorted_peers.sort_by(|a, b| {
            let a_name = a.player_id.as_deref().unwrap_or("???");
            let b_name = b.player_id.as_deref().unwrap_or("???");
            a_name.cmp(b_name)
        });

        let mut items = Vec::new();

        // 1. Host first (if I'm host, show myself)
        if props.is_host {
            items.push(html! {
                <div class="lobby-player-item host me">
                    <span class="lobby-player-name">{&player_id}</span>
                    <span class="lobby-host-badge">{"호스트"}</span>
                    <span class="lobby-me-badge">{"나"}</span>
                </div>
            });
        } else {
            // Find and show host peer first
            if let Some(host_peer) = host_peer_id.and_then(|hid| sorted_peers.iter().find(|p| p.peer_id == hid)) {
                let host_name = host_peer.player_id.as_deref().unwrap_or("???");
                items.push(html! {
                    <div class="lobby-player-item host">
                        <span class="lobby-player-name">{host_name}</span>
                        <span class="lobby-host-badge">{"호스트"}</span>
                    </div>
                });
            }
            // 2. Show myself second (when not host)
            items.push(html! {
                <div class="lobby-player-item me">
                    <span class="lobby-player-name">{&player_id}</span>
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
            let peer_name = peer.player_id.as_deref().unwrap_or("???");
            items.push(html! {
                <div class="lobby-player-item">
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
                                if is_connected {
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
                    my_player_id={player_id.clone()}
                    connection_state={connection_state.clone()}
                    arrival_info={arrival_info}
                    gamerule={gamerule}
                />
            }

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
