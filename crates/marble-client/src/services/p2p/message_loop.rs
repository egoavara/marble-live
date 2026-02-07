//! P2P peer resolution utilities.
//!
//! After refactor: The message loop has been replaced by Bevy's `poll_p2p_socket` system.
//! This module only provides peer_id → player_id resolution via gRPC.

use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{PlayerAuth, ResolvePeerIdsRequest};
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;

/// Resolve peer_ids to player_ids via server API and update Bevy's P2P socket state.
pub fn resolve_peer_ids_from_server(
    room_id: String,
    player_id: String,
    player_secret: String,
    peer_id_strings: Vec<String>,
) {
    if peer_id_strings.is_empty() {
        return;
    }

    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            tracing::warn!("No window object available for ResolvePeerIds");
            return;
        };
        let Ok(origin) = window.location().origin() else {
            tracing::warn!("Failed to get origin for ResolvePeerIds");
            return;
        };

        let client = Client::new(format!("{}/grpc", origin));
        let mut grpc = RoomServiceClient::new(client);

        let req = ResolvePeerIdsRequest {
            room_id,
            player: Some(PlayerAuth {
                id: player_id,
                secret: player_secret,
            }),
            peer_ids: peer_id_strings.clone(),
        };

        match grpc.resolve_peer_ids(req).await {
            Ok(resp) => {
                let resp = resp.into_inner();
                let resolved_count = resp.peer_to_player.len();

                for (peer_id_str, player_id) in resp.peer_to_player {
                    // Forward to Bevy so the P2P socket resource gets updated
                    marble_core::bevy::wasm_entry::update_peer_player_id(
                        &peer_id_str,
                        &player_id,
                    );
                }

                tracing::debug!(
                    resolved = resolved_count,
                    requested = peer_id_strings.len(),
                    "Resolved peer_ids from server"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to resolve peer_ids from server");
            }
        }
    });
}
