//! GameView component - main game view with P2P integration.

use gloo::events::EventListener;
use marble_proto::play::p2p_message::Payload;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use super::reaction_panel::{get_reaction_emoji, REACTION_COOLDOWN_MS};
use super::{CameraControls, ChatPanel, PeerList, ReactionDisplay};
use crate::camera::CameraMode;
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

    // Track if game start was already processed
    let game_start_processed = use_mut_ref(|| false);

    // Auto-start game when host connects
    {
        let game_loop = game_loop.clone();
        let is_host = props.is_host;
        let game_start_processed = game_start_processed.clone();
        use_effect_with(is_connected, move |is_connected| {
            if is_host && *is_connected && !*game_start_processed.borrow() {
                *game_start_processed.borrow_mut() = true;
                game_loop.start_game();
            }
        });
    }

    // Process GameStart messages for non-host
    {
        let game_loop = game_loop.clone();
        let is_host = props.is_host;
        let game_start_processed = game_start_processed.clone();
        let messages_for_game_start = messages.clone();
        use_effect_with(messages.len(), move |_| {
            if is_host || *game_start_processed.borrow() {
                return;
            }

            // Look for GameStart message
            for msg in messages_for_game_start.iter().rev() {
                if let Payload::GameStart(game_start) = &msg.payload {
                    *game_start_processed.borrow_mut() = true;
                    game_loop.init_from_game_start(game_start.seed, &game_start.initial_state, &game_start.gamerule);
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

            // Spawn button (host only)
            if props.is_host && is_connected {
                <div class="spawn-controls">
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
                </div>
            }

            // Left side: Peer list (vertically centered)
            <PeerList
                peers={peers}
                my_player_id={player_id.clone()}
                connection_state={connection_state}
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
