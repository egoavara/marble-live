//! P2P Debug page for testing Partial Mesh + Gossip communication.

use std::cell::RefCell;
use std::rc::Rc;

use marble_proto::play::p2p_message::Payload;
use marble_proto::play::{ChatMessage, P2pMessage, Ping, Pong};
use marble_proto::room::{CreateRoomRequest, JoinRoomRequest, PeerTopology, PlayerAuth};
use matchbox_socket::{PeerId, PeerState, WebRtcSocket};
use prost::Message;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::hooks::{use_config_secret, use_grpc_room_service};
use crate::services::p2p::{ConnectionReporter, GossipHandler, TopologyHandler};

/// Log entry for debug display
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: f64,
    level: String,
    message: String,
}

/// P2P connection state for a peer
#[derive(Debug, Clone)]
struct PeerInfo {
    peer_id: PeerId,
    state: String,
}

/// P2P Debug Page Component
#[function_component(DebugP2pPage)]
pub fn debug_p2p_page() -> Html {
    let grpc = use_grpc_room_service();
    let config_secret = use_config_secret();
    let player_id = config_secret.to_string();

    // State
    let room_id = use_state(|| String::new());
    let room_id_input = use_state(|| String::new());
    let max_players_input = use_state(|| "10".to_string());
    let is_connected = use_state(|| false);
    let is_host = use_state(|| false);
    let topology = use_state(|| Option::<PeerTopology>::None);
    let peers = use_state(|| Vec::<PeerInfo>::new());
    let logs = use_state(|| Vec::<LogEntry>::new());
    let chat_input = use_state(|| String::new());
    let messages = use_state(|| Vec::<(String, String)>::new()); // (player_id, content)

    // Refs for socket and handlers
    let socket_ref: UseStateHandle<Option<Rc<RefCell<WebRtcSocket>>>> = use_state(|| None);
    let gossip_ref: UseStateHandle<Option<Rc<RefCell<GossipHandler>>>> = use_state(|| None);

    // Helper to add log
    let add_log = {
        let logs = logs.clone();
        Callback::from(move |(level, message): (String, String)| {
            let timestamp = js_sys::Date::now();
            let mut new_logs = (*logs).clone();
            new_logs.push(LogEntry {
                timestamp,
                level,
                message,
            });
            // Keep last 100 logs
            if new_logs.len() > 100 {
                new_logs.remove(0);
            }
            logs.set(new_logs);
        })
    };

    // Create Room handler
    let on_create_room = {
        let grpc = grpc.clone();
        let player_id = player_id.clone();
        let room_id = room_id.clone();
        let is_host = is_host.clone();
        let max_players_input = max_players_input.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let grpc = grpc.clone();
            let player_id = player_id.clone();
            let room_id = room_id.clone();
            let is_host = is_host.clone();
            let max_players = max_players_input.parse::<u32>().unwrap_or(10);
            let add_log = add_log.clone();

            spawn_local(async move {
                add_log.emit(("INFO".to_string(), "Creating room...".to_string()));

                let req = CreateRoomRequest {
                    host: Some(PlayerAuth {
                        id: player_id.clone(),
                        secret: player_id.clone(),
                    }),
                    max_players,
                };

                match grpc.borrow_mut().create_room(req).await {
                    Ok(resp) => {
                        let resp = resp.into_inner();
                        room_id.set(resp.room_id.clone());
                        is_host.set(true);
                        add_log.emit((
                            "SUCCESS".to_string(),
                            format!("Room created: {}", resp.room_id),
                        ));
                    }
                    Err(e) => {
                        add_log.emit(("ERROR".to_string(), format!("Failed to create room: {e}")));
                    }
                }
            });
        })
    };

    // Join Room handler
    let on_join_room = {
        let grpc = grpc.clone();
        let player_id = player_id.clone();
        let room_id = room_id.clone();
        let room_id_input = room_id_input.clone();
        let topology = topology.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let grpc = grpc.clone();
            let player_id = player_id.clone();
            let room_id = room_id.clone();
            let target_room = (*room_id_input).clone();
            let topology = topology.clone();
            let add_log = add_log.clone();

            spawn_local(async move {
                add_log.emit((
                    "INFO".to_string(),
                    format!("Joining room: {target_room}..."),
                ));

                let req = JoinRoomRequest {
                    room_id: target_room.clone(),
                    player: Some(PlayerAuth {
                        id: player_id.clone(),
                        secret: player_id.clone(),
                    }),
                };

                match grpc.borrow_mut().join_room(req).await {
                    Ok(resp) => {
                        let resp = resp.into_inner();
                        room_id.set(target_room);
                        if let Some(topo) = resp.topology {
                            add_log.emit((
                                "SUCCESS".to_string(),
                                format!(
                                    "Joined! Group: {}, Bridge: {}, Peers: {}",
                                    topo.mesh_group,
                                    topo.is_bridge,
                                    topo.connect_to.len()
                                ),
                            ));
                            topology.set(Some(topo));
                        } else {
                            add_log.emit(("SUCCESS".to_string(), "Joined! (no topology)".to_string()));
                        }
                    }
                    Err(e) => {
                        add_log.emit(("ERROR".to_string(), format!("Failed to join room: {e}")));
                    }
                }
            });
        })
    };

    // Connect to signaling handler
    let on_connect = {
        let room_id = room_id.clone();
        let player_id = player_id.clone();
        let socket_ref = socket_ref.clone();
        let gossip_ref = gossip_ref.clone();
        let topology = topology.clone();
        let is_connected = is_connected.clone();
        let peers = peers.clone();
        let messages = messages.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let current_room_id = (*room_id).clone();
            if current_room_id.is_empty() {
                add_log.emit(("ERROR".to_string(), "No room ID set".to_string()));
                return;
            }

            let signaling_url = format!("ws://localhost:3000/signaling/{}", current_room_id);
            add_log.emit((
                "INFO".to_string(),
                format!("Connecting to: {signaling_url}"),
            ));

            let (socket, loop_fut) = WebRtcSocket::new_reliable(&signaling_url);

            // Store socket
            let socket = Rc::new(RefCell::new(socket));
            socket_ref.set(Some(socket.clone()));

            // Initialize handlers
            let topo = (*topology).clone().unwrap_or(PeerTopology {
                mesh_group: 0,
                is_bridge: false,
                connect_to: vec![],
                bridge_peers: vec![],
            });

            let gossip = Rc::new(RefCell::new(GossipHandler::new(topo.mesh_group, topo.is_bridge)));
            gossip_ref.set(Some(gossip.clone()));

            is_connected.set(true);

            // Spawn the signaling loop (IMPORTANT: must be awaited for WebRTC to work!)
            let add_log_for_signaling = add_log.clone();
            spawn_local(async move {
                add_log_for_signaling.emit(("INFO".to_string(), "Signaling loop started".to_string()));
                if let Err(e) = loop_fut.await {
                    add_log_for_signaling.emit(("ERROR".to_string(), format!("Signaling error: {:?}", e)));
                }
                add_log_for_signaling.emit(("INFO".to_string(), "Signaling loop ended".to_string()));
            });

            // Spawn the message handling loop
            let socket_clone = socket.clone();
            let peers_clone = peers.clone();
            let messages_clone = messages.clone();
            let add_log_clone = add_log.clone();
            let gossip_clone = gossip.clone();
            let player_id_clone = player_id.clone();

            spawn_local(async move {
                loop {
                    // Check for new peers
                    {
                        let mut socket = socket_clone.borrow_mut();
                        for (peer_id, state) in socket.update_peers() {
                            let state_str = match state {
                                PeerState::Connected => "Connected",
                                PeerState::Disconnected => "Disconnected",
                            };
                            add_log_clone.emit((
                                "PEER".to_string(),
                                format!("Peer {:?}: {state_str}", peer_id),
                            ));

                            let mut current_peers = (*peers_clone).clone();
                            match state {
                                PeerState::Connected => {
                                    current_peers.push(PeerInfo {
                                        peer_id,
                                        state: "Connected".to_string(),
                                    });
                                    // Update gossip handler with new peer
                                    let peer_ids: Vec<PeerId> = current_peers.iter().map(|p| p.peer_id).collect();
                                    gossip_clone.borrow_mut().set_peers(peer_ids, vec![]);
                                }
                                PeerState::Disconnected => {
                                    current_peers.retain(|p| p.peer_id != peer_id);
                                    // Update gossip handler
                                    let peer_ids: Vec<PeerId> = current_peers.iter().map(|p| p.peer_id).collect();
                                    gossip_clone.borrow_mut().set_peers(peer_ids, vec![]);
                                }
                            }
                            peers_clone.set(current_peers);
                        }
                    }

                    // Receive messages
                    {
                        let mut socket = socket_clone.borrow_mut();
                        let received = socket.channel_mut(0).receive();
                        drop(socket);

                        for (peer_id, data) in received {
                            if let Ok(msg) = P2pMessage::decode(&*data) {
                                let mut gossip = gossip_clone.borrow_mut();
                                let (should_process, relay_targets) =
                                    gossip.handle_incoming(&msg, peer_id);

                                if should_process {
                                    // Process message
                                    if let Some(payload) = &msg.payload {
                                        match payload {
                                            Payload::ChatMessage(chat) => {
                                                let mut msgs = (*messages_clone).clone();
                                                msgs.push((
                                                    chat.player_id.clone(),
                                                    chat.content.clone(),
                                                ));
                                                if msgs.len() > 50 {
                                                    msgs.remove(0);
                                                }
                                                messages_clone.set(msgs);
                                            }
                                            Payload::Ping(ping) => {
                                                // Reply with pong
                                                let pong = gossip.create_message(
                                                    &player_id_clone,
                                                    1,
                                                    Payload::Pong(Pong {
                                                        timestamp: ping.timestamp,
                                                    }),
                                                );
                                                let data = pong.encode_to_vec();
                                                drop(gossip);
                                                let mut socket_inner = socket_clone.borrow_mut();
                                                socket_inner.channel_mut(0).send(data.into_boxed_slice(), peer_id);
                                            }
                                            Payload::Pong(pong) => {
                                                let now = js_sys::Date::now();
                                                let rtt = (now - pong.timestamp) as u32;
                                                add_log_clone.emit((
                                                    "RTT".to_string(),
                                                    format!("RTT from {:?}: {}ms", peer_id, rtt),
                                                ));
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                // Relay if needed
                                if !relay_targets.is_empty() {
                                    let relay_msg = gossip_clone.borrow().prepare_for_relay(&msg);
                                    let data = relay_msg.encode_to_vec();
                                    let mut socket_inner = socket_clone.borrow_mut();
                                    for target in relay_targets {
                                        socket_inner.channel_mut(0).send(data.clone().into_boxed_slice(), target);
                                    }
                                }
                            }
                        }
                    }

                    // Yield to other tasks
                    gloo::timers::future::TimeoutFuture::new(16).await;
                }
            });
        })
    };

    // Send chat message
    let on_send_chat = {
        let chat_input = chat_input.clone();
        let socket_ref = socket_ref.clone();
        let gossip_ref = gossip_ref.clone();
        let player_id = player_id.clone();
        let messages = messages.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let content = (*chat_input).clone();
            if content.is_empty() {
                return;
            }

            if let (Some(socket), Some(gossip)) = ((*socket_ref).clone(), (*gossip_ref).clone()) {
                let msg = {
                    let mut gossip = gossip.borrow_mut();
                    gossip.create_message(
                        &player_id,
                        10, // TTL
                        Payload::ChatMessage(ChatMessage {
                            player_id: player_id.clone(),
                            content: content.clone(),
                            timestamp_ms: js_sys::Date::now() as u64,
                        }),
                    )
                };

                let data = msg.encode_to_vec();
                let peers_to_send = gossip.borrow().get_all_peers();

                let mut socket = socket.borrow_mut();
                for peer in peers_to_send {
                    socket.channel_mut(0).send(data.clone().into_boxed_slice(), peer);
                }

                // Add to local messages
                let mut msgs = (*messages).clone();
                msgs.push((player_id.clone(), content));
                messages.set(msgs);

                chat_input.set(String::new());
                add_log.emit(("CHAT".to_string(), "Message sent".to_string()));
            }
        })
    };

    // Send ping to all peers
    let on_send_ping = {
        let socket_ref = socket_ref.clone();
        let gossip_ref = gossip_ref.clone();
        let player_id = player_id.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            if let (Some(socket), Some(gossip)) = ((*socket_ref).clone(), (*gossip_ref).clone()) {
                let msg = {
                    let mut gossip = gossip.borrow_mut();
                    gossip.create_message(
                        &player_id,
                        1, // TTL 1 for ping (direct only)
                        Payload::Ping(Ping {
                            timestamp: js_sys::Date::now(),
                        }),
                    )
                };

                let data = msg.encode_to_vec();
                let peers_to_send = gossip.borrow().get_all_peers();

                let mut socket = socket.borrow_mut();
                for peer in &peers_to_send {
                    socket.channel_mut(0).send(data.clone().into_boxed_slice(), *peer);
                }

                add_log.emit((
                    "PING".to_string(),
                    format!("Sent ping to {} peers", peers_to_send.len()),
                ));
            }
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

    let on_max_players_change = {
        let max_players_input = max_players_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            max_players_input.set(input.value());
        })
    };

    let on_chat_change = {
        let chat_input = chat_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            chat_input.set(input.value());
        })
    };

    html! {
        <main class="page debug-p2p-page" style="padding: 20px; font-family: monospace; color: #e0e0e0;">
            <h1 style="color: #fff; margin-bottom: 20px;">{ "P2P Debug" }</h1>

            // Player Info
            <section style="margin-bottom: 20px; padding: 15px; background: #1a1a2e; border: 1px solid #333; border-radius: 8px;">
                <h3 style="color: #667eea; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #333; padding-bottom: 8px;">{ "Player Info" }</h3>
                <p style="margin: 5px 0; color: #ccc;">{ format!("ID: {}", player_id) }</p>
                <p style="margin: 5px 0; color: #ccc;">{ format!("Room: {}", if (*room_id).is_empty() { "Not in room" } else { &*room_id }) }</p>
                <p style="margin: 5px 0; color: #ccc;">{ format!("Connected: {}", *is_connected) }</p>
                <p style="margin: 5px 0; color: #ccc;">{ format!("Is Host: {}", *is_host) }</p>
            </section>

            // Room Controls
            <section style="margin-bottom: 20px; padding: 15px; background: #1a2e1a; border: 1px solid #2a4a2a; border-radius: 8px;">
                <h3 style="color: #4caf50; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #2a4a2a; padding-bottom: 8px;">{ "Room Controls" }</h3>
                <div style="margin-bottom: 10px;">
                    <label style="color: #aaa;">{ "Max Players: " }</label>
                    <input
                        type="number"
                        value={(*max_players_input).clone()}
                        oninput={on_max_players_change}
                        style="width: 60px; margin-right: 10px; padding: 6px; background: #2a2a3e; border: 1px solid #444; border-radius: 4px; color: #fff;"
                    />
                    <button onclick={on_create_room} style="padding: 6px 12px; background: #4caf50; border: none; border-radius: 4px; color: #fff; cursor: pointer;">{ "Create Room" }</button>
                </div>
                <div style="margin-bottom: 10px;">
                    <label style="color: #aaa;">{ "Room ID: " }</label>
                    <input
                        type="text"
                        value={(*room_id_input).clone()}
                        oninput={on_room_id_change}
                        placeholder="Enter room ID"
                        style="width: 300px; margin-right: 10px; padding: 6px; background: #2a2a3e; border: 1px solid #444; border-radius: 4px; color: #fff;"
                    />
                    <button onclick={on_join_room} style="padding: 6px 12px; background: #667eea; border: none; border-radius: 4px; color: #fff; cursor: pointer;">{ "Join Room" }</button>
                </div>
                <div>
                    <button onclick={on_connect} disabled={(*room_id).is_empty() || *is_connected} style="padding: 6px 12px; background: #ff9800; border: none; border-radius: 4px; color: #fff; cursor: pointer;">
                        { "Connect to Signaling" }
                    </button>
                </div>
            </section>

            // Topology Info
            if let Some(topo) = (*topology).clone() {
                <section style="margin-bottom: 20px; padding: 15px; background: #1a1a2e; border: 1px solid #333; border-radius: 8px;">
                    <h3 style="color: #9c27b0; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #333; padding-bottom: 8px;">{ "Topology" }</h3>
                    <p style="margin: 5px 0; color: #ccc;">{ format!("Mesh Group: {}", topo.mesh_group) }</p>
                    <p style="margin: 5px 0; color: #ccc;">{ format!("Is Bridge: {}", topo.is_bridge) }</p>
                    <p style="margin: 5px 0; color: #ccc;">{ format!("Connect To: {} peers", topo.connect_to.len()) }</p>
                    <ul style="margin: 5px 0; padding-left: 20px; color: #aaa;">
                        { for topo.connect_to.iter().map(|p| html! {
                            <li>{ format!("{} ({})", p.player_id, p.peer_id) }</li>
                        }) }
                    </ul>
                    if topo.is_bridge {
                        <p style="margin: 5px 0; color: #ccc;">{ format!("Bridge Peers: {} peers", topo.bridge_peers.len()) }</p>
                        <ul style="margin: 5px 0; padding-left: 20px; color: #aaa;">
                            { for topo.bridge_peers.iter().map(|p| html! {
                                <li>{ format!("{} ({})", p.player_id, p.peer_id) }</li>
                            }) }
                        </ul>
                    }
                </section>
            }

            // Connected Peers
            <section style="margin-bottom: 20px; padding: 15px; background: #2e1a1a; border: 1px solid #4a2a2a; border-radius: 8px;">
                <h3 style="color: #f44336; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #4a2a2a; padding-bottom: 8px;">{ format!("Connected Peers ({})", (*peers).len()) }</h3>
                <button onclick={on_send_ping} disabled={!*is_connected} style="padding: 6px 12px; background: #f44336; border: none; border-radius: 4px; color: #fff; cursor: pointer; margin-bottom: 10px;">{ "Send Ping" }</button>
                <ul style="margin: 5px 0; padding-left: 20px; color: #ccc;">
                    { for (*peers).iter().map(|p| html! {
                        <li>{ format!("{:?} - {}", p.peer_id, p.state) }</li>
                    }) }
                </ul>
            </section>

            // Chat
            <section style="margin-bottom: 20px; padding: 15px; background: #2e2e1a; border: 1px solid #4a4a2a; border-radius: 8px;">
                <h3 style="color: #ffeb3b; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #4a4a2a; padding-bottom: 8px;">{ "Chat (Gossip Test)" }</h3>
                <div style="height: 150px; overflow-y: auto; background: #1a1a2e; padding: 10px; margin-bottom: 10px; border: 1px solid #333; border-radius: 4px;">
                    { for (*messages).iter().map(|(pid, content)| html! {
                        <div style="margin-bottom: 5px; color: #e0e0e0;">
                            <strong style="color: #667eea;">{ format!("[{}]", pid) }</strong>
                            { format!(": {}", content) }
                        </div>
                    }) }
                </div>
                <div>
                    <input
                        type="text"
                        value={(*chat_input).clone()}
                        oninput={on_chat_change}
                        placeholder="Type a message..."
                        style="width: 300px; margin-right: 10px; padding: 6px; background: #2a2a3e; border: 1px solid #444; border-radius: 4px; color: #fff;"
                    />
                    <button onclick={on_send_chat} disabled={!*is_connected} style="padding: 6px 12px; background: #667eea; border: none; border-radius: 4px; color: #fff; cursor: pointer;">{ "Send" }</button>
                </div>
            </section>

            // Logs
            <section style="padding: 15px; background: #0d0d1a; border: 1px solid #333; border-radius: 8px;">
                <h3 style="color: #4caf50; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #333; padding-bottom: 8px;">{ "Logs" }</h3>
                <div style="height: 200px; overflow-y: auto; font-size: 12px; background: #0a0a14; padding: 10px; border-radius: 4px;">
                    { for (*logs).iter().rev().map(|log| {
                        let color = match log.level.as_str() {
                            "ERROR" => "#ff6b6b",
                            "SUCCESS" => "#69db7c",
                            "PEER" => "#74c0fc",
                            "CHAT" => "#ffd43b",
                            "PING" | "RTT" => "#da77f2",
                            _ => "#69db7c",
                        };
                        html! {
                            <div style={format!("color: {}; margin-bottom: 2px;", color)}>
                                { format!("[{:.0}] [{}] {}", log.timestamp % 100000.0, log.level, log.message) }
                            </div>
                        }
                    }) }
                </div>
            </section>
        </main>
    }
}
