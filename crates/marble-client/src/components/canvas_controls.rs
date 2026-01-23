//! Canvas overlay controls for game actions.

use crate::p2p::protocol::{P2PMessage, PlayerStartInfo};
use crate::p2p::session::get_player_list;
use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Canvas overlay controls with Ready/Start buttons.
/// Also shows "Back to Lobby" when game is finished.
#[function_component(CanvasControls)]
pub fn canvas_controls() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let is_lobby = matches!(state.phase, P2PPhase::WaitingForPeers | P2PPhase::Lobby);
    let is_finished = matches!(state.phase, P2PPhase::Finished);

    // Only show in lobby phases or when finished
    if !is_lobby && !is_finished {
        return html! {};
    }

    // Finished state: show winner and "Back to Lobby" button
    if is_finished {
        let winner_text = if let marble_core::GamePhase::Finished { winner } = state.game_state.current_phase() {
            match winner {
                Some(id) => {
                    let name = state.game_state
                        .get_player(*id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("Unknown");
                    format!("{} Wins!", name)
                }
                None => "No Winner!".to_string(),
            }
        } else {
            "Race Finished!".to_string()
        };

        let on_back_to_lobby = {
            let state = state.clone();
            Callback::from(move |_| {
                state.dispatch(P2PAction::ResetToLobby);
            })
        };

        return html! {
            <div class="canvas-controls-overlay finished">
                <div class="canvas-controls-content">
                    <div class="finished-message">{ winner_text }</div>
                    <div class="control-buttons">
                        <button
                            class="control-btn back-to-lobby-btn"
                            onclick={on_back_to_lobby}
                        >
                            { "Back to Lobby" }
                        </button>
                    </div>
                </div>
            </div>
        };
    }

    // Lobby state: show Ready/Start buttons
    let on_toggle_ready = {
        let state = state.clone();
        Callback::from(move |_| {
            let new_ready = !state.my_ready;
            state.dispatch(P2PAction::SetMyReady(new_ready));
            let msg = P2PMessage::PlayerReady { ready: new_ready };
            state.network.borrow_mut().broadcast(&msg.encode());
        })
    };

    let on_start_game = {
        let state = state.clone();
        Callback::from(move |_| {
            if !state.is_host {
                state.dispatch(P2PAction::AddLog("Only the host can start the game".to_string()));
                return;
            }
            if !state.all_peers_ready() {
                state.dispatch(P2PAction::AddLog("All players must be ready".to_string()));
                return;
            }

            let seed = js_sys::Date::now() as u64;
            let players = get_player_list(&state);
            let player_infos: Vec<PlayerStartInfo> = players
                .iter()
                .map(|(peer_id, name, color)| PlayerStartInfo::new(*peer_id, name.clone(), *color))
                .collect();

            let msg = P2PMessage::GameStart {
                seed,
                players: player_infos,
            };
            state.network.borrow_mut().broadcast(&msg.encode());

            state.dispatch(P2PAction::StartGame {
                seed,
                players: players.clone(),
            });
            state.dispatch(P2PAction::StartCountdown);
        })
    };

    let all_ready = state.all_peers_ready();
    let can_start = state.is_host && all_ready && state.player_count() >= 2;

    let status_message = if state.player_count() < 2 {
        "Need at least 2 players to start"
    } else if !all_ready {
        "Waiting for all players to be ready"
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
                    <button
                        class={classes!("control-btn", "ready-btn", state.my_ready.then_some("active"))}
                        onclick={on_toggle_ready}
                    >
                        { if state.my_ready { "Cancel Ready" } else { "Ready!" } }
                    </button>

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
