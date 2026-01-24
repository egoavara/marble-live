//! GameView component - main game view with P2P integration.

use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use super::reaction_panel::{get_reaction_emoji, REACTION_COOLDOWN_MS};
use super::{ChatPanel, PeerList, ReactionDisplay};
use crate::hooks::{
    use_config_secret, use_config_username, use_p2p_room_with_credentials, P2pRoomConfig,
};

/// Props for the GameView component.
#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    /// Room ID to connect to
    pub room_id: String,
    /// Signaling server URL
    pub signaling_url: String,
}

/// GameView component - P2P-enabled game view with overlays.
///
/// Contains:
/// - PeerList (left side, vertically centered)
/// - ReactionDisplay (floating emojis)
/// - ChatPanel with integrated reactions (bottom-right corner)
/// - Game canvas area (future implementation)
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

    html! {
        <div class="game-view">
            // Left side: Peer list (vertically centered)
            <PeerList
                peers={peers}
                my_player_id={player_id.clone()}
                connection_state={connection_state}
            />

            // Center: Game canvas area (future)
            <div class="game-canvas-placeholder">
                <span class="placeholder-text">{"Game Area"}</span>
            </div>

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
