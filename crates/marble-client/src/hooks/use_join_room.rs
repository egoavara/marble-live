//! Hook for automatically joining a room via gRPC.

use marble_proto::room::{JoinRoomRequest, PlayerAuth};
use yew::prelude::*;

use crate::hooks::use_config_username;

use super::{use_config_secret, use_grpc_room_service};

/// State for room join operation.
#[derive(Clone, PartialEq)]
pub enum JoinRoomState {
    Idle,
    Joining,
    /// Successfully joined (or already in) the room.
    /// signaling_url may be empty if player was already in the room.
    Joined { signaling_url: String },
    Error(String),
}

impl Default for JoinRoomState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Hook to automatically join a room via gRPC JoinRoom call.
///
/// This hook calls JoinRoom once on mount and returns the join state.
#[hook]
pub fn use_join_room(room_id: &str) -> UseStateHandle<JoinRoomState> {
    let client = use_grpc_room_service();
    let config_username = use_config_username();
    let config_secret = use_config_secret();
    let state = use_state(|| JoinRoomState::Idle);

    {
        let state = state.clone();
        let room_id = room_id.to_string();
        let player_id = config_username
            .as_ref()
            .map(|x| x.to_string())
            .clone()
            .expect("Username must be set to join a room");
        let player_secret = config_secret.to_string();

        use_effect_with(room_id.clone(), move |room_id| {
            let room_id = room_id.clone();
            let state = state.clone();
            let client = client.clone();

            wasm_bindgen_futures::spawn_local(async move {
                state.set(JoinRoomState::Joining);

                let request = JoinRoomRequest {
                    room_id: room_id.clone(),
                    player: Some(PlayerAuth {
                        id: player_id.clone(),
                        secret: player_secret.clone(),
                    }),
                };

                match client.borrow_mut().join_room(request).await {
                    Ok(response) => {
                        let signaling_url = response.into_inner().signaling_url;
                        state.set(JoinRoomState::Joined { signaling_url });
                    }
                    Err(e) => {
                        let error_msg = e.message().to_string();
                        // If player already exists in the room (e.g., host rejoining),
                        // treat it as a successful join
                        if error_msg.contains("already exists") {
                            state.set(JoinRoomState::Joined {
                                signaling_url: String::new(),
                            });
                        } else {
                            state.set(JoinRoomState::Error(error_msg));
                        }
                    }
                }
            });

            || ()
        });
    }

    state
}
