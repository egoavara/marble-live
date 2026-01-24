//! Hook for room connection management.

use crate::fingerprint::get_browser_fingerprint;
use crate::p2p::state::{P2PAction, P2PStateContext};
use crate::storage::UserSettings;
use yew::prelude::*;

/// Manage room connection (join or create).
///
/// This hook handles:
/// - Auto-connecting to a room when the component mounts
/// - Falling back to creating a room if join fails
/// - Dispatching connection state to the reducer
#[hook]
pub fn use_room_connection(room_id: &str, state: &P2PStateContext) {
    let state = state.clone();
    let room_id = room_id.to_string();

    // Only run once on mount
    use_effect_with((), move |_| {
        let settings = UserSettings::load().unwrap_or_default();
        let name = if settings.name.is_empty() {
            "Player".to_string()
        } else {
            settings.display_name()
        };
        let fingerprint = get_browser_fingerprint();
        let color = settings.color;

        state.dispatch(P2PAction::SetConnecting);

        let network = state.network.clone();
        let state_clone = state.clone();
        let room_id_clone = room_id.clone();

        wasm_bindgen_futures::spawn_local(async move {
            // Try to join the existing room with color
            let result = network.borrow_mut().join_room(&room_id_clone, &name, &fingerprint, color).await;

            match result {
                Ok((seed, is_game_in_progress, player_id, is_host, server_players, host_player_id)) => {
                    state_clone.dispatch(P2PAction::SetConnected {
                        room_id: room_id_clone,
                        server_seed: seed,
                        is_game_in_progress,
                        player_id,
                        is_host,
                    });
                    // Update server players from response
                    state_clone.dispatch(P2PAction::UpdateServerPlayers { players: server_players, host_player_id });
                }
                Err(_) => {
                    // Fall back to creating a new room
                    match network
                        .borrow_mut()
                        .create_and_join_room("Marble Race", &name, &fingerprint, color)
                        .await
                    {
                        Ok((created_room_id, seed, is_game_in_progress, player_id, is_host, server_players, host_player_id)) => {
                            state_clone.dispatch(P2PAction::SetConnected {
                                room_id: created_room_id,
                                server_seed: seed,
                                is_game_in_progress,
                                player_id,
                                is_host,
                            });
                            // Update server players from response
                            state_clone.dispatch(P2PAction::UpdateServerPlayers { players: server_players, host_player_id });
                        }
                        Err(e) => {
                            state_clone.dispatch(P2PAction::SetError(e));
                            state_clone.dispatch(P2PAction::SetDisconnected);
                        }
                    }
                }
            }
        });

        || ()
    });
}
