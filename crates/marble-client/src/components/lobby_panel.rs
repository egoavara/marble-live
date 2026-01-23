//! Lobby panel component for ready state and game start.

use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Simplified lobby panel - only shows Leave Room button.
/// Ready/Start buttons are now in CanvasControls overlay.
#[function_component(LobbyPanel)]
pub fn lobby_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let on_disconnect = {
        let state = state.clone();
        Callback::from(move |_| {
            state.network.borrow_mut().disconnect();
            state.dispatch(P2PAction::SetDisconnected);
        })
    };

    html! {
        <div class="lobby-panel">
            <button class="leave-room-btn" onclick={on_disconnect}>
                { "Leave Room" }
            </button>
        </div>
    }
}

/// Game status panel shown during gameplay.
#[function_component(GameStatusPanel)]
pub fn game_status_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let on_back_to_lobby = {
        let state = state.clone();
        Callback::from(move |_| {
            state.dispatch(P2PAction::ResetToLobby);
        })
    };

    let frame = state.game_state.current_frame();
    let hash = state.game_state.compute_hash();

    html! {
        <div class="game-status-panel">
            <h3>{ "Game Status" }</h3>

            <div class="status-row">
                <span class="status-label">{ "Phase:" }</span>
                <span class={classes!("status-value", match state.phase {
                    P2PPhase::Running => "running",
                    P2PPhase::Countdown { .. } => "countdown",
                    P2PPhase::Finished => "finished",
                    P2PPhase::Resyncing => "resyncing",
                    _ => "",
                })}>
                    { format!("{:?}", state.phase) }
                </span>
            </div>

            <div class="status-info">
                <div>{ format!("Frame: {}", frame) }</div>
                <div>{ format!("Hash: {:016X}", hash) }</div>
            </div>

            { if let P2PPhase::Countdown { remaining_frames } = state.phase {
                let seconds = remaining_frames / 60;
                html! {
                    <div class="countdown-display">
                        { seconds + 1 }
                    </div>
                }
            } else {
                html! {}
            }}

            { if state.phase == P2PPhase::Finished {
                html! {
                    <button class="back-to-lobby-btn" onclick={on_back_to_lobby}>
                        { "Back to Lobby" }
                    </button>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
