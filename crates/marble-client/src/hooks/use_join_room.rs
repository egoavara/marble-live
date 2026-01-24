//! Hook for automatically joining a room via gRPC.

use marble_proto::room::{GetRoomPlayerRequest, JoinRoomRequest, PlayerAuth};
use yew::prelude::*;

use crate::hooks::use_config_username;

use super::{use_config_secret, use_grpc_room_service};

/// State for room join operation.
#[derive(Clone, PartialEq)]
pub enum JoinRoomState {
    Idle,
    Joining,
    /// Successfully joined the room. Server is idempotent, so this works even if already in room.
    Joined { signaling_url: String, is_host: bool },
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

                let join_request = JoinRoomRequest {
                    room_id: room_id.clone(),
                    player: Some(PlayerAuth {
                        id: player_id.clone(),
                        secret: player_secret.clone(),
                    }),
                };

                // First request: join room
                let join_result = client.borrow_mut().join_room(join_request).await;

                match join_result {
                    Ok(response) => {
                        let signaling_url = response.into_inner().signaling_url;

                        // Second request: get player info to check if we're the host
                        let player_request = GetRoomPlayerRequest {
                            room_id: room_id.clone(),
                        };

                        let player_result = client.borrow_mut().get_room_player(player_request).await;

                        let is_host = match player_result {
                            Ok(player_response) => {
                                player_response
                                    .into_inner()
                                    .players
                                    .iter()
                                    .find(|p| p.id == player_id)
                                    .map(|p| p.is_host)
                                    .unwrap_or(false)
                            }
                            Err(_) => false,
                        };

                        state.set(JoinRoomState::Joined { signaling_url, is_host });
                    }
                    Err(e) => {
                        state.set(JoinRoomState::Error(e.message().to_string()));
                    }
                }
            });

            || ()
        });
    }

    state
}
