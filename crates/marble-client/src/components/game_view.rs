//! GameView component - main game view with P2P integration.

use gloo::events::EventListener;
use marble_core::GamePhase;
use marble_proto::play::p2p_message::Payload;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use super::reaction_panel::{get_reaction_emoji, REACTION_COOLDOWN_MS};
use super::{ChatPanel, PeerList, ReactionDisplay};
use crate::hooks::{
    use_config_secret, use_config_username, use_game_loop, use_p2p_room_with_credentials,
    GameLoopState, P2pRoomConfig,
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

    // Game loop hook
    let game_seed = use_state(|| js_sys::Date::now() as u64);
    let game_loop = use_game_loop(&p2p, canvas_ref.clone(), props.is_host, *game_seed);

    // Track if game start was already processed
    let game_start_processed = use_mut_ref(|| false);

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
                    game_loop.init_from_game_start(game_start.seed, &game_start.initial_state);
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

    // Keyboard event handler for reaction shortcuts (1-5)
    {
        let send_reaction = send_reaction.clone();
        let last_keyboard_emoji = last_keyboard_emoji.clone();
        let is_connected = is_connected;
        use_effect_with(is_connected, move |_| {
            let listener = web_sys::window().map(|window| {
                let last_keyboard_emoji = last_keyboard_emoji.clone();
                EventListener::new(&window, "keydown", move |event| {
                    if !is_connected {
                        return;
                    }

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

                    let key = keyboard_event.key();
                    if let Some(emoji) = get_reaction_emoji(&key) {
                        // Update last_keyboard_emoji for ChatPanel sync
                        last_keyboard_emoji.set(Some(emoji.to_string()));
                        send_reaction(emoji);
                    }
                })
            });

            // Keep listener alive until cleanup
            move || drop(listener)
        });
    }

    // Calculate if reactions are on cooldown
    let reaction_disabled = *cooldown_active;

    // Start game handler (host only)
    let on_start_game = {
        let game_loop = game_loop.clone();
        Callback::from(move |_: MouseEvent| {
            game_loop.start_game();
        })
    };

    // Game state info for UI
    let game_phase = game_loop.game_phase();
    let loop_state = (*game_loop.loop_state).clone();

    // Determine what to show
    let show_start_button = props.is_host
        && matches!(loop_state, GameLoopState::Idle)
        && is_connected
        && peers.len() >= 1; // At least one peer (besides self)

    let show_waiting_message = !props.is_host && matches!(loop_state, GameLoopState::Idle);

    let countdown_remaining = match &game_phase {
        GamePhase::Countdown { remaining_frames } => Some(*remaining_frames),
        _ => None,
    };

    let show_countdown = countdown_remaining.is_some();

    let game_finished = matches!(loop_state, GameLoopState::Finished);

    html! {
        <div class="game-view fullscreen">
            // Game canvas
            <canvas
                ref={canvas_ref}
                class="game-canvas"
                width="800"
                height="600"
            />

            // Countdown overlay
            if show_countdown {
                <div class="game-overlay countdown-overlay">
                    <div class="countdown-number">
                        { (countdown_remaining.unwrap_or(0) / 60) + 1 }
                    </div>
                </div>
            }

            // Start button (host only, in lobby)
            if show_start_button {
                <div class="game-overlay start-overlay">
                    <button class="start-game-btn" onclick={on_start_game}>
                        { "게임 시작" }
                    </button>
                    <p class="start-hint">{ format!("{} 플레이어 대기 중", peers.len() + 1) }</p>
                </div>
            }

            // Waiting message (non-host, in lobby)
            if show_waiting_message {
                <div class="game-overlay waiting-overlay">
                    <p class="waiting-text">{ "호스트의 게임 시작을 기다리는 중..." }</p>
                </div>
            }

            // Game finished overlay
            if game_finished {
                <div class="game-overlay finished-overlay">
                    <h2 class="finished-title">{ "게임 종료!" }</h2>
                    if props.is_host {
                        <button class="restart-btn" onclick={
                            let game_loop = game_loop.clone();
                            Callback::from(move |_: MouseEvent| {
                                game_loop.reset_to_lobby();
                            })
                        }>
                            { "다시 시작" }
                        </button>
                    }
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
