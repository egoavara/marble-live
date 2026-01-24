//! Hook for fetching room information via gRPC.

use marble_proto::room::{GetRoomRequest, RoomInfo};
use yew::prelude::*;

use super::use_grpc_room_service;

/// State for room information.
#[derive(Clone, PartialEq, Default)]
pub struct RoomInfoState {
    pub data: Option<RoomInfo>,
    pub loading: bool,
    pub error: Option<String>,
}

/// Hook to fetch room information via gRPC GetRoom call.
///
/// This hook fetches room info only once on mount.
/// The returned state includes loading/error states for UI feedback.
#[hook]
pub fn use_room_info(room_id: &str) -> UseStateHandle<RoomInfoState> {
    let client = use_grpc_room_service();
    let state = use_state(|| RoomInfoState {
        data: None,
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
                state.set(RoomInfoState {
                    data: None,
                    loading: true,
                    error: None,
                });

                let request = GetRoomRequest {
                    room_id: room_id.clone(),
                };

                match client.borrow_mut().get_room(request).await {
                    Ok(response) => {
                        let room = response.into_inner().room;
                        state.set(RoomInfoState {
                            data: room,
                            loading: false,
                            error: None,
                        });
                    }
                    Err(e) => {
                        state.set(RoomInfoState {
                            data: None,
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
