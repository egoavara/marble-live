//! Canvas overlay controls for game actions.

use crate::p2p::protocol::P2PMessage;
use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Canvas overlay controls with Start button (host only).
/// Shows in lobby phases only. Game finish is handled by WinnerModal.
#[function_component(CanvasControls)]
pub fn canvas_controls() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let is_lobby = matches!(state.phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby);

    // Only show in lobby phases
    if !is_lobby {
        return html! {};
    }

    let on_start_game = {
        let state = state.clone();
        Callback::from(move |_| {
            if !state.is_host {
                state.dispatch(P2PAction::AddLog("Only the host can start the game".to_string()));
                return;
            }
            if state.player_count() < 2 {
                state.dispatch(P2PAction::AddLog("Need at least 2 players".to_string()));
                return;
            }

            let network = state.network.clone();
            let room_id = state.room_id.clone();
            let player_id = state.my_player_id.clone();
            let state_clone = state.clone();

            // Use the server-provided seed for deterministic game initialization
            let seed = state.game_seed;

            // Get player order from server-authoritative data (sorted by join_order)
            let player_order: Vec<String> = state_clone.server_players_by_order()
                .iter()
                .map(|p| p.player_id.clone())
                .collect();

            // Call server API to start game
            wasm_bindgen_futures::spawn_local(async move {
                match network.borrow().start_game_on_server(&room_id, &player_id).await {
                    Ok(()) => {
                        // Server confirmed game start - broadcast GameStartOrder to peers
                        let msg = P2PMessage::GameStartOrder {
                            seed,
                            player_order: player_order.clone(),
                        };
                        network.borrow_mut().broadcast(&msg.encode());

                        // Start game locally using the same player order
                        state_clone.dispatch(P2PAction::StartGameFromServer { seed, player_order });
                        state_clone.dispatch(P2PAction::StartCountdown);
                    }
                    Err(e) => {
                        state_clone.dispatch(P2PAction::AddLog(format!("Failed to start game: {}", e)));
                    }
                }
            });
        })
    };

    let can_start = state.is_host && state.player_count() >= 2;

    let status_message = if state.player_count() < 2 {
        "Need at least 2 players to start"
    } else if !state.is_host {
        "Waiting for host to start"
    } else {
        "Ready to start!"
    };

    html! {
        <div class="canvas-controls-overlay">
            <div class="canvas-controls-content">
                <div class="player-count">
                    { format!("{} Player{}", state.player_count(), if state.player_count() == 1 { "" } else { "s" }) }
                </div>
                <div class="status-message">{ status_message }</div>

                <div class="control-buttons">
                    { if state.is_host {
                        html! {
                            <button
                                class={classes!("control-btn", "start-btn", (!can_start).then_some("disabled"))}
                                onclick={on_start_game}
                                disabled={!can_start}
                            >
                                { "Start Game" }
                            </button>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}
