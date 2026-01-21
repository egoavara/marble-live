//! P2P connection test page
//!
//! Two players can share a simple counter state over WebRTC P2P connection.

use crate::network::{create_shared_network_manager, NetworkEvent};
use gloo::timers::callback::Interval;
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq)]
enum PageState {
    Initial,
    CreatingRoom,
    JoiningRoom,
    WaitingForPeer,
    Connected,
    Error(String),
}

// Simple message types for P2P communication
const MSG_COUNTER_UPDATE: u8 = 1;
const MSG_PING: u8 = 2;
const MSG_PONG: u8 = 3;

#[function_component(DebugConnTestPage)]
pub fn debug_conntest_page() -> Html {
    let page_state = use_state(|| PageState::Initial);
    let room_id = use_state(String::new);
    let join_room_id = use_state(String::new);
    let peer_count = use_state(|| 0usize);
    let shared_counter = use_state(|| 0u32);
    let ping_ms = use_state(|| None::<u32>);
    let logs = use_state(Vec::<String>::new);

    // Network manager
    let network = use_state(|| create_shared_network_manager("/grpc"));

    // Add log helper
    let add_log = {
        let logs = logs.clone();
        Callback::from(move |msg: String| {
            logs.set({
                let mut l = (*logs).clone();
                l.push(format!("[{}] {}", js_sys::Date::new_0().to_locale_time_string("en-US"), msg));
                if l.len() > 20 {
                    l.remove(0);
                }
                l
            });
        })
    };

    // Network polling effect
    {
        let network = network.clone();
        let peer_count = peer_count.clone();
        let page_state = page_state.clone();
        let shared_counter = shared_counter.clone();
        let ping_ms = ping_ms.clone();
        let add_log = add_log.clone();

        use_effect_with((*page_state).clone(), move |state| {
            let interval: Option<Interval> = if *state != PageState::WaitingForPeer && *state != PageState::Connected {
                None
            } else {
                Some(Interval::new(50, move || {
                let events = network.borrow_mut().poll();

                for event in events {
                    match event {
                        NetworkEvent::PeerJoined(peer_id) => {
                            let count = network.borrow().peers().len();
                            peer_count.set(count);
                            add_log.emit(format!("Peer joined: {peer_id}"));

                            if count >= 1 {
                                page_state.set(PageState::Connected);
                            }
                        }
                        NetworkEvent::PeerLeft(peer_id) => {
                            let count = network.borrow().peers().len();
                            peer_count.set(count);
                            add_log.emit(format!("Peer left: {peer_id}"));

                            if count == 0 {
                                page_state.set(PageState::WaitingForPeer);
                            }
                        }
                        NetworkEvent::Message { from, data } => {
                            if data.is_empty() {
                                continue;
                            }

                            match data[0] {
                                MSG_COUNTER_UPDATE if data.len() >= 5 => {
                                    let value = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
                                    shared_counter.set(value);
                                    add_log.emit(format!("Counter update from {from}: {value}"));
                                }
                                MSG_PING => {
                                    // Respond with pong
                                    let mut pong = vec![MSG_PONG];
                                    pong.extend_from_slice(&data[1..]);
                                    network.borrow_mut().send_to(from, &pong);
                                }
                                MSG_PONG if data.len() >= 9 => {
                                    let sent_time = f64::from_be_bytes([
                                        data[1], data[2], data[3], data[4],
                                        data[5], data[6], data[7], data[8],
                                    ]);
                                    let now = js_sys::Date::now();
                                    let latency = (now - sent_time) as u32;
                                    ping_ms.set(Some(latency));
                                    add_log.emit(format!("Ping: {latency}ms"));
                                }
                                _ => {}
                            }
                        }
                        NetworkEvent::StateChanged(state) => {
                            add_log.emit(format!("Connection state: {state:?}"));
                        }
                    }
                }
                }))
            };

            move || drop(interval)
        });
    }

    // Event handlers
    let on_create_room = {
        let network = network.clone();
        let page_state = page_state.clone();
        let room_id = room_id.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let network = network.clone();
            let page_state = page_state.clone();
            let room_id = room_id.clone();
            let add_log = add_log.clone();

            page_state.set(PageState::CreatingRoom);

            wasm_bindgen_futures::spawn_local(async move {
                match network
                    .borrow_mut()
                    .create_and_join_room("Test Room", "Player")
                    .await
                {
                    Ok(id) => {
                        add_log.emit(format!("Created room: {id}"));
                        room_id.set(id);
                        page_state.set(PageState::WaitingForPeer);
                    }
                    Err(e) => {
                        add_log.emit(format!("Error creating room: {e}"));
                        page_state.set(PageState::Error(e));
                    }
                }
            });
        })
    };

    let on_join_room = {
        let network = network.clone();
        let page_state = page_state.clone();
        let join_room_id = join_room_id.clone();
        let room_id = room_id.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let network = network.clone();
            let page_state = page_state.clone();
            let rid = (*join_room_id).clone();
            let room_id = room_id.clone();
            let add_log = add_log.clone();

            if rid.is_empty() {
                page_state.set(PageState::Error("Room ID is required".to_string()));
                return;
            }

            page_state.set(PageState::JoiningRoom);

            wasm_bindgen_futures::spawn_local(async move {
                match network.borrow_mut().join_room(&rid, "Player").await {
                    Ok(()) => {
                        add_log.emit(format!("Joined room: {rid}"));
                        room_id.set(rid);
                        page_state.set(PageState::WaitingForPeer);
                    }
                    Err(e) => {
                        add_log.emit(format!("Error joining room: {e}"));
                        page_state.set(PageState::Error(e));
                    }
                }
            });
        })
    };

    let on_room_id_input = {
        let join_room_id = join_room_id.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            join_room_id.set(input.value());
        })
    };

    let on_increment = {
        let network = network.clone();
        let shared_counter = shared_counter.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let new_value = *shared_counter + 1;
            shared_counter.set(new_value);

            // Broadcast to peers
            let mut msg = vec![MSG_COUNTER_UPDATE];
            msg.extend_from_slice(&new_value.to_be_bytes());
            network.borrow_mut().broadcast(&msg);

            add_log.emit(format!("Incremented counter to {new_value}"));
        })
    };

    let on_ping = {
        let network = network.clone();
        let add_log = add_log.clone();

        Callback::from(move |_| {
            let now = js_sys::Date::now();
            let mut msg = vec![MSG_PING];
            msg.extend_from_slice(&now.to_be_bytes());
            network.borrow_mut().broadcast(&msg);
            add_log.emit("Sent ping".to_string());
        })
    };

    let on_reset = {
        let page_state = page_state.clone();
        let room_id = room_id.clone();
        let network = network.clone();
        let peer_count = peer_count.clone();
        let shared_counter = shared_counter.clone();
        let logs = logs.clone();

        Callback::from(move |_| {
            network.borrow_mut().disconnect();
            page_state.set(PageState::Initial);
            room_id.set(String::new());
            peer_count.set(0);
            shared_counter.set(0);
            logs.set(Vec::new());
        })
    };

    html! {
        <div style="padding: 20px; font-family: monospace; max-width: 800px; margin: 0 auto; background: #fff; color: #333; min-height: 100vh;">
            <h1 style="color: #333;">{"P2P Connection Test"}</h1>

            // State display
            <div style="margin-bottom: 20px; padding: 15px; background: #f5f5f5; border-radius: 8px; border: 1px solid #ddd; color: #333;">
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 10px;">
                    <div>
                        <strong>{"State: "}</strong>
                        <span style={match *page_state {
                            PageState::Connected => "color: green",
                            PageState::Error(_) => "color: red",
                            _ => "color: orange"
                        }}>
                            {format!("{:?}", *page_state)}
                        </span>
                    </div>
                    <div>
                        <strong>{"Peers: "}</strong>{*peer_count}
                    </div>
                    <div>
                        <strong>{"Shared Counter: "}</strong>
                        <span style="font-size: 24px; font-weight: bold; color: #2196F3;">
                            {*shared_counter}
                        </span>
                    </div>
                    <div>
                        <strong>{"Ping: "}</strong>
                        {match *ping_ms {
                            Some(ms) => format!("{ms}ms"),
                            None => "-".to_string(),
                        }}
                    </div>
                </div>
            </div>

            // Room ID display (for sharing)
            if !room_id.is_empty() {
                <div style="margin-bottom: 20px; padding: 15px; background: #e8f5e9; border-radius: 8px; border: 1px solid #c8e6c9; color: #333;">
                    <strong>{"Room ID (share this): "}</strong>
                    <code style="background: #fff; padding: 4px 8px; border-radius: 4px; font-size: 14px; user-select: all;">
                        {&*room_id}
                    </code>
                </div>
            }

            // Controls based on state
            if *page_state == PageState::Initial || matches!(*page_state, PageState::Error(_)) {
                <div style="margin-bottom: 20px;">
                    <h3 style="color: #333;">{"Create a new room"}</h3>
                    <button
                        onclick={on_create_room}
                        style="padding: 10px 20px; font-size: 16px; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px;"
                    >
                        {"Create Room"}
                    </button>
                </div>

                <div style="margin-bottom: 20px;">
                    <h3 style="color: #333;">{"Or join an existing room"}</h3>
                    <input
                        type="text"
                        placeholder="Enter Room ID"
                        value={(*join_room_id).clone()}
                        oninput={on_room_id_input}
                        style="width: 300px; padding: 10px; margin-right: 10px; font-size: 14px; border: 1px solid #ccc; border-radius: 4px;"
                    />
                    <button
                        onclick={on_join_room}
                        style="padding: 10px 20px; font-size: 16px; cursor: pointer; background: #2196F3; color: white; border: none; border-radius: 4px;"
                    >
                        {"Join Room"}
                    </button>
                </div>
            }

            // Connected controls
            if *page_state == PageState::Connected {
                <div style="margin-bottom: 20px; display: flex; gap: 10px;">
                    <button
                        onclick={on_increment}
                        style="padding: 15px 30px; font-size: 18px; cursor: pointer; background: #FF9800; color: white; border: none; border-radius: 4px;"
                    >
                        {"+ Increment Counter"}
                    </button>
                    <button
                        onclick={on_ping}
                        style="padding: 15px 30px; font-size: 18px; cursor: pointer; background: #9C27B0; color: white; border: none; border-radius: 4px;"
                    >
                        {"Ping"}
                    </button>
                    <button
                        onclick={on_reset.clone()}
                        style="padding: 15px 30px; font-size: 18px; cursor: pointer; background: #f44336; color: white; border: none; border-radius: 4px;"
                    >
                        {"Disconnect"}
                    </button>
                </div>
                <p style="color: #666; font-size: 12px;">
                    {"Click 'Increment Counter' and the other player should see the same value!"}
                </p>
            }

            // Waiting message
            if *page_state == PageState::WaitingForPeer {
                <div style="padding: 20px; background: #fff3e0; border-radius: 8px; border: 1px solid #ffcc80; color: #333;">
                    <p style="margin: 0 0 10px 0; color: #333;">{"Waiting for another player to join..."}</p>
                    <p style="margin: 0; color: #555;">
                        {"Share the Room ID above with another browser tab/window."}
                    </p>
                </div>
            }

            // Loading states
            if *page_state == PageState::CreatingRoom || *page_state == PageState::JoiningRoom {
                <div style="padding: 20px; background: #e3f2fd; border-radius: 8px; border: 1px solid #90caf9; color: #333;">
                    {"Connecting..."}
                </div>
            }

            // Logs
            <div style="margin-top: 20px;">
                <h3 style="color: #333;">{"Event Log"}</h3>
                <div style="background: #1e1e1e; color: #d4d4d4; padding: 15px; border-radius: 8px; height: 250px; overflow-y: auto; font-size: 12px; line-height: 1.6;">
                    {if logs.is_empty() {
                        html! { <div style="color: #666;">{"No events yet..."}</div> }
                    } else {
                        html! {
                            {for logs.iter().rev().map(|log| html! {
                                <div>{log}</div>
                            })}
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
