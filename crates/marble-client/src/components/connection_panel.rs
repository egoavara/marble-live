//! Connection panel component for creating/joining rooms.

use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Connection panel for creating or joining a room.
#[function_component(ConnectionPanel)]
pub fn connection_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");
    let join_room_id = use_state(String::new);
    let player_name = use_state(|| "Player".to_string());

    let on_name_input = {
        let player_name = player_name.clone();
        let state = state.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let name = input.value();
            player_name.set(name.clone());
            state.dispatch(P2PAction::SetMyName(name));
        })
    };

    let on_room_id_input = {
        let join_room_id = join_room_id.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            join_room_id.set(input.value());
        })
    };

    let on_create_room = {
        let state = state.clone();
        let player_name = player_name.clone();

        Callback::from(move |_| {
            let state = state.clone();
            let name = (*player_name).clone();

            state.dispatch(P2PAction::SetMyName(name.clone()));
            state.dispatch(P2PAction::SetConnecting);

            let network = state.network.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match network.borrow_mut().create_and_join_room("P2P Game", &name).await {
                    Ok(room_id) => {
                        state.dispatch(P2PAction::SetConnected { room_id });
                    }
                    Err(e) => {
                        state.dispatch(P2PAction::SetError(e.clone()));
                        state.dispatch(P2PAction::SetDisconnected);
                    }
                }
            });
        })
    };

    let on_join_room = {
        let state = state.clone();
        let join_room_id = join_room_id.clone();
        let player_name = player_name.clone();

        Callback::from(move |_| {
            let state = state.clone();
            let room_id = (*join_room_id).clone();
            let name = (*player_name).clone();

            if room_id.is_empty() {
                state.dispatch(P2PAction::AddLog("Room ID is required".to_string()));
                return;
            }

            state.dispatch(P2PAction::SetMyName(name.clone()));
            state.dispatch(P2PAction::SetConnecting);

            let network = state.network.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match network.borrow_mut().join_room(&room_id, &name).await {
                    Ok(()) => {
                        state.dispatch(P2PAction::SetConnected { room_id });
                    }
                    Err(e) => {
                        state.dispatch(P2PAction::SetError(e.clone()));
                        state.dispatch(P2PAction::SetDisconnected);
                    }
                }
            });
        })
    };

    let is_connecting = state.phase == P2PPhase::Connecting;

    html! {
        <div class="connection-panel" style="background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);">
            <h2 style="margin: 0 0 20px 0; color: #333;">{"P2P Game"}</h2>

            // Player name input
            <div style="margin-bottom: 20px;">
                <label style="display: block; margin-bottom: 5px; color: #333; font-weight: bold;">
                    {"Your Name"}
                </label>
                <input
                    type="text"
                    value={(*player_name).clone()}
                    oninput={on_name_input}
                    disabled={is_connecting}
                    style="width: 100%; padding: 10px; font-size: 14px; border: 1px solid #ccc; border-radius: 4px; box-sizing: border-box;"
                    placeholder="Enter your name"
                />
            </div>

            // Create room section
            <div style="margin-bottom: 20px; padding-bottom: 20px; border-bottom: 1px solid #eee;">
                <h3 style="margin: 0 0 10px 0; color: #333;">{"Create a new room"}</h3>
                <button
                    onclick={on_create_room}
                    disabled={is_connecting}
                    style="width: 100%; padding: 12px 20px; font-size: 16px; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px; opacity: if is_connecting { 0.6 } else { 1.0 };"
                >
                    {if is_connecting { "Creating..." } else { "Create Room" }}
                </button>
            </div>

            // Join room section
            <div>
                <h3 style="margin: 0 0 10px 0; color: #333;">{"Or join an existing room"}</h3>
                <div style="display: flex; gap: 10px;">
                    <input
                        type="text"
                        placeholder="Enter Room ID"
                        value={(*join_room_id).clone()}
                        oninput={on_room_id_input}
                        disabled={is_connecting}
                        style="flex: 1; padding: 10px; font-size: 14px; border: 1px solid #ccc; border-radius: 4px;"
                    />
                    <button
                        onclick={on_join_room}
                        disabled={is_connecting}
                        style="padding: 10px 20px; font-size: 16px; cursor: pointer; background: #2196F3; color: white; border: none; border-radius: 4px;"
                    >
                        {if is_connecting { "Joining..." } else { "Join" }}
                    </button>
                </div>
            </div>
        </div>
    }
}
