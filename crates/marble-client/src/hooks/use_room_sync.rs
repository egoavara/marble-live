
//! Hook for room state synchronization via GetRoom polling.

use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext, ServerPlayerInfo};
use gloo::timers::callback::Interval;
use marble_core::Color;
use yew::prelude::*;

/// Polling interval for GetRoom in milliseconds.
const ROOM_SYNC_INTERVAL_MS: u32 = 2000;

/// Poll GetRoom to synchronize player list during lobby phase.
///
/// This hook handles:
/// - Periodic GetRoom polling in lobby state
/// - Updating server_players when changes are detected
/// - Detecting host changes
#[hook]
pub fn use_room_sync(state: &P2PStateContext) {
    let state = state.clone();
    let phase = state.phase.clone();

    use_effect_with(phase.clone(), move |phase| {
        let is_in_lobby = matches!(phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby);

        let interval: Option<Interval> = if !is_in_lobby {
            None
        } else {
            let state_inner = state.clone();
            Some(Interval::new(ROOM_SYNC_INTERVAL_MS, move || {
                let network = state_inner.network.clone();
                let state_clone = state_inner.clone();
                let room_id = state_inner.room_id.clone();

                if room_id.is_empty() {
                    return;
                }

                wasm_bindgen_futures::spawn_local(async move {
                    let result = network.borrow().room_client().get_room(&room_id).await;

                    if let Ok(resp) = result {
                        if let Some(room) = resp.room {
                            // Convert proto PlayerInfo to ServerPlayerInfo
                            let server_players: Vec<ServerPlayerInfo> = room.players
                                .into_iter()
                                .map(|p| ServerPlayerInfo {
                                    player_id: p.id,
                                    name: p.name,
                                    color: Color::rgb(p.color_r as u8, p.color_g as u8, p.color_b as u8),
                                    is_connected: p.is_connected,
                                    join_order: p.join_order,
                                })
                                .collect();

                            state_clone.dispatch(P2PAction::UpdateServerPlayers {
                                players: server_players,
                                host_player_id: room.host_player_id,
                            });
                        }
                    }
                });
            }))
        };

        move || drop(interval)
    });
}
