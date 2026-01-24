//! Debug page for testing gRPC RoomService calls.

use crate::components::Layout;
use crate::hooks::use_grpc_room_service;
use marble_proto::room::{
    CreateRoomRequest, GetRoomPlayerRequest, GetRoomRequest, JoinRoomRequest, KickRoomRequest,
    PlayerAuth, StartRoomRequest,
};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

/// Debug page for testing gRPC calls.
#[function_component(DebugGrpcPage)]
pub fn debug_grpc_page() -> Html {
    let client = use_grpc_room_service();

    // Input states
    let room_id = use_state(|| "".to_string());
    let player_id = use_state(|| "".to_string());
    let player_secret = use_state(|| "".to_string());
    let max_players = use_state(|| "4".to_string());
    let target_player = use_state(|| "".to_string());

    // Response state
    let response = use_state(|| "No response yet".to_string());
    let loading = use_state(|| false);

    // Input handlers
    let on_room_id_change = {
        let room_id = room_id.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            room_id.set(input.value());
        })
    };

    let on_player_id_change = {
        let player_id = player_id.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            player_id.set(input.value());
        })
    };

    let on_player_secret_change = {
        let player_secret = player_secret.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            player_secret.set(input.value());
        })
    };

    let on_max_players_change = {
        let max_players = max_players.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            max_players.set(input.value());
        })
    };

    let on_target_player_change = {
        let target_player = target_player.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            target_player.set(input.value());
        })
    };

    // CreateRoom
    let on_create_room = {
        let client = client.clone();
        let player_id = player_id.clone();
        let player_secret = player_secret.clone();
        let max_players = max_players.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let player_id = (*player_id).clone();
            let player_secret = (*player_secret).clone();
            let max_players_val: u32 = max_players.parse().unwrap_or(4);
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = CreateRoomRequest {
                    host: Some(PlayerAuth {
                        id: player_id,
                        secret: player_secret,
                    }),
                    max_players: max_players_val,
                };
                let result = client.borrow_mut().create_room(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("CreateRoom Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("CreateRoom Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    // JoinRoom
    let on_join_room = {
        let client = client.clone();
        let room_id = room_id.clone();
        let player_id = player_id.clone();
        let player_secret = player_secret.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let room_id = (*room_id).clone();
            let player_id = (*player_id).clone();
            let player_secret = (*player_secret).clone();
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = JoinRoomRequest {
                    room_id,
                    player: Some(PlayerAuth {
                        id: player_id,
                        secret: player_secret,
                    }),
                };
                let result = client.borrow_mut().join_room(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("JoinRoom Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("JoinRoom Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    // StartRoom
    let on_start_room = {
        let client = client.clone();
        let room_id = room_id.clone();
        let player_id = player_id.clone();
        let player_secret = player_secret.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let room_id = (*room_id).clone();
            let player_id = (*player_id).clone();
            let player_secret = (*player_secret).clone();
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = StartRoomRequest {
                    room_id,
                    player: Some(PlayerAuth {
                        id: player_id,
                        secret: player_secret,
                    }),
                };
                let result = client.borrow_mut().start_room(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("StartRoom Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("StartRoom Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    // KickRoom
    let on_kick_room = {
        let client = client.clone();
        let room_id = room_id.clone();
        let player_id = player_id.clone();
        let player_secret = player_secret.clone();
        let target_player = target_player.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let room_id = (*room_id).clone();
            let player_id = (*player_id).clone();
            let player_secret = (*player_secret).clone();
            let target_player = (*target_player).clone();
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = KickRoomRequest {
                    room_id,
                    player: Some(PlayerAuth {
                        id: player_id,
                        secret: player_secret,
                    }),
                    target_player,
                };
                let result = client.borrow_mut().kick_room(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("KickRoom Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("KickRoom Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    // GetRoom
    let on_get_room = {
        let client = client.clone();
        let room_id = room_id.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let room_id = (*room_id).clone();
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = GetRoomRequest { room_id };
                let result = client.borrow_mut().get_room(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("GetRoom Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("GetRoom Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    // GetRoomPlayer
    let on_get_room_player = {
        let client = client.clone();
        let room_id = room_id.clone();
        let response = response.clone();
        let loading = loading.clone();
        Callback::from(move |_| {
            let client = client.clone();
            let room_id = (*room_id).clone();
            let response = response.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                let req = GetRoomPlayerRequest { room_id };
                let result = client.borrow_mut().get_room_player(req).await;
                match result {
                    Ok(res) => {
                        response.set(format!("GetRoomPlayer Response:\n{:#?}", res.into_inner()));
                    }
                    Err(e) => {
                        response.set(format!("GetRoomPlayer Error:\n{}", e));
                    }
                }
                loading.set(false);
            });
        })
    };

    html! {
        <Layout>
            <div class="debug-grpc-page" style="padding: 20px; max-width: 800px; margin: 0 auto;">
                <h1 style="margin-bottom: 20px;">{ "gRPC RoomService Debug" }</h1>

                // Input fields
                <div style="background: #1a1a2e; padding: 20px; border-radius: 8px; margin-bottom: 20px;">
                    <h3 style="margin-bottom: 15px;">{ "Parameters" }</h3>

                    <div style="display: grid; gap: 10px;">
                        <div>
                            <label style="display: block; margin-bottom: 5px;">{ "Room ID" }</label>
                            <input
                                type="text"
                                value={(*room_id).clone()}
                                oninput={on_room_id_change}
                                style="width: 100%; padding: 8px; border-radius: 4px; border: 1px solid #333; background: #0f0f1a; color: white;"
                                placeholder="Room ID (UUID)"
                            />
                        </div>

                        <div>
                            <label style="display: block; margin-bottom: 5px;">{ "Player ID" }</label>
                            <input
                                type="text"
                                value={(*player_id).clone()}
                                oninput={on_player_id_change}
                                style="width: 100%; padding: 8px; border-radius: 4px; border: 1px solid #333; background: #0f0f1a; color: white;"
                                placeholder="Player ID"
                            />
                        </div>

                        <div>
                            <label style="display: block; margin-bottom: 5px;">{ "Player Secret" }</label>
                            <input
                                type="text"
                                value={(*player_secret).clone()}
                                oninput={on_player_secret_change}
                                style="width: 100%; padding: 8px; border-radius: 4px; border: 1px solid #333; background: #0f0f1a; color: white;"
                                placeholder="Player Secret"
                            />
                        </div>

                        <div>
                            <label style="display: block; margin-bottom: 5px;">{ "Max Players (CreateRoom)" }</label>
                            <input
                                type="number"
                                value={(*max_players).clone()}
                                oninput={on_max_players_change}
                                style="width: 100%; padding: 8px; border-radius: 4px; border: 1px solid #333; background: #0f0f1a; color: white;"
                                placeholder="4"
                            />
                        </div>

                        <div>
                            <label style="display: block; margin-bottom: 5px;">{ "Target Player (KickRoom)" }</label>
                            <input
                                type="text"
                                value={(*target_player).clone()}
                                oninput={on_target_player_change}
                                style="width: 100%; padding: 8px; border-radius: 4px; border: 1px solid #333; background: #0f0f1a; color: white;"
                                placeholder="Target Player ID to kick"
                            />
                        </div>
                    </div>
                </div>

                // Action buttons
                <div style="background: #1a1a2e; padding: 20px; border-radius: 8px; margin-bottom: 20px;">
                    <h3 style="margin-bottom: 15px;">{ "Actions" }</h3>

                    <div style="display: flex; flex-wrap: wrap; gap: 10px;">
                        <button
                            onclick={on_create_room}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #4CAF50; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "CreateRoom" }
                        </button>

                        <button
                            onclick={on_join_room}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #2196F3; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "JoinRoom" }
                        </button>

                        <button
                            onclick={on_start_room}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #FF9800; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "StartRoom" }
                        </button>

                        <button
                            onclick={on_kick_room}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #f44336; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "KickRoom" }
                        </button>

                        <button
                            onclick={on_get_room}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #9C27B0; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "GetRoom" }
                        </button>

                        <button
                            onclick={on_get_room_player}
                            disabled={*loading}
                            style="padding: 10px 20px; background: #00BCD4; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            { "GetRoomPlayer" }
                        </button>
                    </div>
                </div>

                // Response display
                <div style="background: #1a1a2e; padding: 20px; border-radius: 8px;">
                    <h3 style="margin-bottom: 15px;">
                        { "Response" }
                        { if *loading { html! { <span style="margin-left: 10px; color: #888;">{ " Loading..." }</span> } } else { html! {} } }
                    </h3>
                    <pre style="background: #0f0f1a; padding: 15px; border-radius: 4px; overflow-x: auto; white-space: pre-wrap; word-wrap: break-word;">
                        { &*response }
                    </pre>
                </div>
            </div>
        </Layout>
    }
}
