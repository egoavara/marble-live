//! Hook for fetching room player information via gRPC.

use marble_proto::room::{GetRoomUsersRequest, RoomUser};
use yew::prelude::*;

use super::use_grpc_room_service;

/// State for room player list.
#[derive(Clone, PartialEq, Default)]
pub struct RoomPlayerState {
    pub players: Vec<RoomUser>,
    pub loading: bool,
    pub error: Option<String>,
}

impl RoomPlayerState {
    /// Add a player to the list (for P2P enter signal integration).
    pub fn add_player(&mut self, player: RoomUser) {
        if !self.players.iter().any(|p| p.user_id == player.user_id) {
            self.players.push(player);
        }
    }

    /// Remove a player from the list by ID.
    pub fn remove_player(&mut self, user_id: &str) {
        self.players.retain(|p| p.user_id != user_id);
    }

    /// Update a player's info.
    pub fn update_player(&mut self, player: RoomUser) {
        if let Some(existing) = self.players.iter_mut().find(|p| p.user_id == player.user_id) {
            *existing = player;
        }
    }
}

/// Hook to fetch room players via gRPC GetRoomUsers call.
///
/// This hook fetches player list only once on mount.
/// The returned state includes loading/error states for UI feedback.
///
/// For P2P synchronization, use the utility methods on RoomPlayerState
/// to add/remove players as enter/leave signals are received.
#[hook]
pub fn use_room_player(room_id: &str) -> UseStateHandle<RoomPlayerState> {
    let client = use_grpc_room_service();
    let state = use_state(|| RoomPlayerState {
        players: vec![],
        loading: true,
        error: None,
    });

    {
        let state = state.clone();
        let room_id = room_id.to_string();
        use_effect_with(room_id.clone(), move |room_id| {
            let room_id = room_id.clone();
            let state = state.clone();
            let client = client.clone();

            wasm_bindgen_futures::spawn_local(async move {
                state.set(RoomPlayerState {
                    players: vec![],
                    loading: true,
                    error: None,
                });

                let request = GetRoomUsersRequest {
                    room_id: room_id.clone(),
                };

                match client.borrow_mut().get_room_users(request).await {
                    Ok(response) => {
                        let players = response.into_inner().users;
                        state.set(RoomPlayerState {
                            players,
                            loading: false,
                            error: None,
                        });
                    }
                    Err(e) => {
                        state.set(RoomPlayerState {
                            players: vec![],
                            loading: false,
                            error: Some(e.message().to_string()),
                        });
                    }
                }
            });

            || ()
        });
    }

    state
}
