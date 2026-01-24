//! ChatPanel component for P2P chat messaging.

use marble_proto::play::p2p_message::Payload;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::services::p2p::{P2pRoomHandle, ReceivedMessage};

/// Inactivity timeout in milliseconds (15 seconds).
const INACTIVITY_TIMEOUT_MS: u32 = 15000;

/// Props for the ChatPanel component.
#[derive(Properties, PartialEq)]
pub struct ChatPanelProps {
    /// P2P room handle for sending messages
    pub p2p: P2pRoomHandle,
    /// Whether P2P is connected (passed separately for reactivity)
    pub is_connected: bool,
    /// Chat messages (passed separately for reactivity)
    pub messages: Vec<ReceivedMessage>,
    /// Current player ID
    pub my_player_id: String,
}

/// ChatPanel component - P2P chat interface.
///
/// Positioned as an overlay in the bottom-right corner.
/// Game-style: transparent background, text shadows for readability.
/// Auto-fades message area after 5 seconds of inactivity.
#[function_component(ChatPanel)]
pub fn chat_panel(props: &ChatPanelProps) -> Html {
    let input_ref = use_node_ref();
    let input_value = use_state(String::new);
    let is_active = use_state(|| true);

    // Filter chat messages from props
    let messages: Vec<_> = props
        .messages
        .iter()
        .filter(|m| matches!(&m.payload, Payload::ChatMessage(_)))
        .collect();
    let messages_len = messages.len();
    let my_player_id = &props.my_player_id;

    // Helper to reset inactivity timer
    let reset_timer = {
        let is_active = is_active.clone();
        move || {
            is_active.set(true);
            let is_active = is_active.clone();
            gloo::timers::callback::Timeout::new(INACTIVITY_TIMEOUT_MS, move || {
                is_active.set(false);
            })
            .forget();
        }
    };

    // Reset timer when new messages arrive
    {
        let reset_timer = reset_timer.clone();
        use_effect_with(messages_len, move |_| {
            reset_timer();
            || {}
        });
    }

    let on_input = {
        let input_value = input_value.clone();
        let reset_timer = reset_timer.clone();
        Callback::from(move |e: InputEvent| {
            reset_timer();
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                input_value.set(input.value());
            }
        })
    };

    let on_focus = {
        let reset_timer = reset_timer.clone();
        Callback::from(move |_: FocusEvent| {
            reset_timer();
        })
    };

    let on_submit = {
        let p2p = props.p2p.clone();
        let input_value = input_value.clone();
        let input_ref = input_ref.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let content = (*input_value).trim().to_string();
            if !content.is_empty() {
                p2p.send_chat(&content);
                input_value.set(String::new());
                // Clear input field
                if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                    input.set_value("");
                }
            }
        })
    };

    let panel_class = if *is_active {
        "chat-panel"
    } else {
        "chat-panel inactive"
    };

    html! {
        <div class={panel_class}>
            <div class="chat-panel-messages">
                { for messages.iter().map(|msg| {
                    let is_self = match &msg.payload {
                        Payload::ChatMessage(chat) => chat.player_id == *my_player_id,
                        _ => false,
                    };
                    let sender = match &msg.payload {
                        Payload::ChatMessage(chat) => chat.player_id.clone(),
                        _ => "Unknown".to_string(),
                    };
                    let content = match &msg.payload {
                        Payload::ChatMessage(chat) => chat.content.clone(),
                        _ => String::new(),
                    };
                    let msg_class = if is_self { "chat-message self" } else { "chat-message" };

                    html! {
                        <div class={msg_class} key={msg.id.clone()}>
                            <span class="chat-sender">{sender}{":"}</span>
                            <span class="chat-content">{content}</span>
                        </div>
                    }
                })}
            </div>
            <form class="chat-panel-input" onsubmit={on_submit}>
                <input
                    ref={input_ref}
                    type="text"
                    placeholder="Type a message..."
                    oninput={on_input}
                    onfocus={on_focus}
                    disabled={!props.is_connected}
                />
                <button type="submit" disabled={!props.is_connected}>
                    {"Send"}
                </button>
            </form>
        </div>
    }
}
