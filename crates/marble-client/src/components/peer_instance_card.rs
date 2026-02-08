//! Peer instance card component for P2P debug page.
//!
//! Each card represents an independent P2P connection instance.

use marble_proto::play::p2p_message::Payload;
use marble_proto::room::{JoinRoomRequest, PeerTopology};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::network_visualization::PeerNetworkInfo;
use crate::hooks::{
    P2pConnectionState, P2pRoomConfig, use_grpc_room_service, use_p2p_room_with_player_id,
};

/// Configuration for a single peer instance
#[derive(Clone, PartialEq)]
pub struct PeerConfig {
    /// Unique instance ID
    pub id: u32,
    /// Player ID (e.g., "username_1", "username_2")
    pub player_id: String,
    /// Player secret for authentication
    pub player_secret: String,
}

/// Props for the PeerInstanceCard component
#[derive(Properties, PartialEq)]
pub struct PeerInstanceCardProps {
    pub config: PeerConfig,
    /// Shared room ID across all instances
    pub room_id: String,
    /// Callback to remove this peer instance
    pub on_remove: Callback<u32>,
    /// Optional callback to update network info (for visualization)
    #[prop_or_default]
    pub on_network_update: Option<Callback<PeerNetworkInfo>>,
    /// Optional callback when peer successfully joins a room
    #[prop_or_default]
    pub on_joined: Option<Callback<u32>>,
    /// Trigger to join room (when incremented, triggers join)
    pub join_trigger: u32,
}

/// Peer colors for consistent styling
const PEER_COLORS: [&str; 8] = [
    "#667eea", "#4caf50", "#ff9800", "#e91e63", "#00bcd4", "#9c27b0", "#ff5722", "#3f51b5",
];

/// A single peer instance card component
/// Each card has its own P2P connection and state
#[function_component(PeerInstanceCard)]
pub fn peer_instance_card(props: &PeerInstanceCardProps) -> Html {
    let grpc = use_grpc_room_service();

    // Local state for this peer instance
    let local_room_id = use_state(|| String::new());
    let room_id_input = use_state(|| props.room_id.clone());
    let chat_input = use_state(|| String::new());
    let local_topology = use_state(|| Option::<PeerTopology>::None);

    // Sync room_id_input with props.room_id when it changes
    {
        let room_id_input = room_id_input.clone();
        let props_room_id = props.room_id.clone();
        use_effect_with(props_room_id.clone(), move |new_room_id| {
            if !new_room_id.is_empty() {
                room_id_input.set(new_room_id.clone());
            }
            || ()
        });
    }

    // P2P Room Hook - each card has its own independent P2P connection
    let p2p = use_p2p_room_with_player_id(
        &*local_room_id,
        &props.config.player_id,
        P2pRoomConfig {
            auto_connect: true,
            max_messages: 50,
            ..Default::default()
        },
    );

    // Track if we're in the process of joining
    let is_joining = use_state(|| false);

    // Get reactive state from p2p hook
    let connection_state = p2p.state();
    let is_connected = p2p.is_connected();
    let peers = p2p.peers();
    let messages = p2p.messages();

    // Update network info when peers or topology change (if callback is provided)
    {
        let on_network_update = props.on_network_update.clone();
        let instance_id = props.config.id;
        let player_id = props.config.player_id.clone();
        let peers = peers.clone();
        let is_connected = is_connected;
        let local_topology = local_topology.clone();

        use_effect_with(
            (peers.clone(), is_connected, (*local_topology).clone()),
            move |(peers, is_connected, topology)| {
                if let Some(callback) = &on_network_update {
                    let connected_peers: Vec<String> = peers
                        .iter()
                        .map(|p| {
                            p.player_id
                                .clone()
                                .unwrap_or_else(|| format!("{:?}", p.peer_id))
                        })
                        .collect();

                    let (mesh_group, is_bridge) = topology
                        .as_ref()
                        .map(|t| (Some(t.mesh_group), t.is_bridge))
                        .unwrap_or((None, false));

                    callback.emit(PeerNetworkInfo {
                        instance_id,
                        player_id: player_id.clone(),
                        connected_peers,
                        is_connected: *is_connected,
                        mesh_group,
                        is_bridge,
                    });
                }

                || ()
            },
        );
    }

    // Shared join logic (used by both button and trigger)
    let do_join_room: std::rc::Rc<dyn Fn()> = {
        let grpc = grpc.clone();
        let instance_id = props.config.id;
        let player_id = props.config.player_id.clone();
        let player_secret = props.config.player_secret.clone();
        let local_room_id = local_room_id.clone();
        let room_id_input = room_id_input.clone();
        let is_joining = is_joining.clone();
        let local_topology = local_topology.clone();
        let on_joined = props.on_joined.clone();

        std::rc::Rc::new(move || {
            let grpc = grpc.clone();
            let player_id = player_id.clone();
            let player_secret = player_secret.clone();
            let local_room_id = local_room_id.clone();
            let target_room = (*room_id_input).clone();
            let is_joining = is_joining.clone();
            let local_topology = local_topology.clone();
            let on_joined = on_joined.clone();

            if target_room.is_empty() {
                web_sys::console::warn_1(&"Room ID is empty".into());
                return;
            }

            is_joining.set(true);

            spawn_local(async move {
                let req = JoinRoomRequest {
                    room_id: target_room.clone(),
                    role: None,
                };

                match grpc.borrow_mut().join_room(req).await {
                    Ok(resp) => {
                        let resp = resp.into_inner();
                        // Store topology from JoinRoom response
                        if let Some(topology) = resp.topology {
                            local_topology.set(Some(topology));
                        }
                        local_room_id.set(target_room);
                        // Notify parent that join succeeded
                        if let Some(callback) = &on_joined {
                            callback.emit(instance_id);
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to join room: {e}").into());
                    }
                }
                is_joining.set(false);
            });
        })
    };

    // Join Room handler - wraps shared logic for button click
    let on_join_room = {
        let do_join = do_join_room.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            do_join();
        })
    };

    // Join trigger from parent (for "Join All" button)
    {
        let join_trigger = props.join_trigger;
        let do_join = do_join_room.clone();
        let is_connected = is_connected;
        let is_joining = is_joining.clone();
        let room_id_input = room_id_input.clone();

        use_effect_with(join_trigger, move |trigger| {
            if *trigger > 0 && !is_connected && !*is_joining && !(*room_id_input).is_empty() {
                do_join();
            }
            || ()
        });
    }

    // Disconnect handler
    let on_disconnect = {
        let p2p = p2p.clone();
        Callback::from(move |_| {
            p2p.disconnect();
        })
    };

    // Send chat message
    let on_send_chat = {
        let chat_input = chat_input.clone();
        let p2p = p2p.clone();

        Callback::from(move |_| {
            let content = (*chat_input).clone();
            if content.is_empty() {
                return;
            }
            p2p.send_chat(&content);
            chat_input.set(String::new());
        })
    };

    // Input handlers
    let on_room_id_change = {
        let room_id_input = room_id_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            room_id_input.set(input.value());
        })
    };

    let on_chat_change = {
        let chat_input = chat_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            chat_input.set(input.value());
        })
    };

    // Remove this peer instance
    let on_remove_click = {
        let on_remove = props.on_remove.clone();
        let id = props.config.id;
        Callback::from(move |_| {
            on_remove.emit(id);
        })
    };

    // State indicator styling
    let (state_icon, state_color) = match &connection_state {
        P2pConnectionState::Disconnected => ("○", "#aaa"),
        P2pConnectionState::Connecting => ("◐", "#ffd43b"),
        P2pConnectionState::Connected => ("●", "#69db7c"),
        P2pConnectionState::Error(_) => ("✕", "#ff6b6b"),
    };

    // Assign color based on instance_id
    let peer_color = PEER_COLORS[(props.config.id as usize - 1) % PEER_COLORS.len()];

    html! {
        <div style={format!("
            background: #1a1a2e;
            border: 2px solid {};
            border-radius: 8px;
            padding: 12px;
            min-width: 280px;
            max-width: 350px;
            display: flex;
            flex-direction: column;
            gap: 8px;
        ", peer_color)}>
            // Header with peer info and remove button
            <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #333; padding-bottom: 8px;">
                <div>
                    <strong style={format!("color: {};", peer_color)}>{ format!("Peer #{}", props.config.id) }</strong>
                    <div style="font-size: 12px; color: #aaa;">{ &props.config.player_id }</div>
                </div>
                <button
                    onclick={on_remove_click}
                    style="
                        background: #ff6b6b;
                        border: none;
                        border-radius: 4px;
                        color: #fff;
                        cursor: pointer;
                        width: 24px;
                        height: 24px;
                        font-size: 14px;
                    "
                >{ "×" }</button>
            </div>

            // Connection state
            <div style="display: flex; align-items: center; gap: 8px;">
                <span style={format!("color: {}; font-size: 16px;", state_color)}>{ state_icon }</span>
                <span style="color: #ccc; font-size: 12px;">
                    { match &connection_state {
                        P2pConnectionState::Disconnected => "Disconnected".to_string(),
                        P2pConnectionState::Connecting => "Connecting...".to_string(),
                        P2pConnectionState::Connected => format!("Connected ({} peers)", peers.len()),
                        P2pConnectionState::Error(e) => format!("Error: {}", e),
                    }}
                </span>
            </div>

            // Room controls
            <div style="display: flex; gap: 4px; flex-wrap: wrap;">
                <input
                    type="text"
                    value={(*room_id_input).clone()}
                    oninput={on_room_id_change}
                    placeholder="Room ID"
                    disabled={is_connected || *is_joining}
                    style="
                        flex: 1;
                        min-width: 100px;
                        padding: 4px 8px;
                        background: #2a2a3e;
                        border: 1px solid #444;
                        border-radius: 4px;
                        color: #fff;
                        font-size: 12px;
                    "
                />
                if is_connected {
                    <button
                        onclick={on_disconnect}
                        style="
                            padding: 4px 8px;
                            background: #ff6b6b;
                            border: none;
                            border-radius: 4px;
                            color: #fff;
                            cursor: pointer;
                            font-size: 12px;
                        "
                    >{ "Disconnect" }</button>
                } else {
                    <button
                        onclick={on_join_room}
                        disabled={*is_joining || (*room_id_input).is_empty()}
                        style={format!("
                            padding: 4px 8px;
                            background: {};
                            border: none;
                            border-radius: 4px;
                            color: #fff;
                            cursor: pointer;
                            font-size: 12px;
                        ", peer_color)}
                    >{ if *is_joining { "Joining..." } else { "Join & Connect" } }</button>
                }
            </div>

            // Chat area
            <div style="
                flex: 1;
                min-height: 80px;
                max-height: 120px;
                overflow-y: auto;
                background: #0d0d1a;
                padding: 6px;
                border-radius: 4px;
                font-size: 11px;
            ">
                { for messages.iter().filter_map(|msg| {
                    if let Payload::ChatMessage(chat) = &msg.payload {
                        Some(html! {
                            <div style="margin-bottom: 2px; color: #e0e0e0;">
                                <strong style="color: #667eea;">{ format!("[{}]", chat.user_id) }</strong>
                                { format!(": {}", chat.content) }
                            </div>
                        })
                    } else {
                        None
                    }
                }) }
            </div>

            // Chat input
            <div style="display: flex; gap: 4px;">
                <input
                    type="text"
                    value={(*chat_input).clone()}
                    oninput={on_chat_change}
                    placeholder="Message..."
                    style="
                        flex: 1;
                        padding: 4px 8px;
                        background: #2a2a3e;
                        border: 1px solid #444;
                        border-radius: 4px;
                        color: #fff;
                        font-size: 12px;
                    "
                />
                <button
                    onclick={on_send_chat}
                    disabled={!is_connected}
                    style="
                        padding: 4px 8px;
                        background: #4caf50;
                        border: none;
                        border-radius: 4px;
                        color: #fff;
                        cursor: pointer;
                        font-size: 12px;
                    "
                >{ "Send" }</button>
            </div>
        </div>
    }
}
