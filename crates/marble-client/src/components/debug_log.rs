//! Debug log toggle component with game status.

use crate::p2p::state::{P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Debug log toggle button and popup with game status.
#[function_component(DebugLogToggle)]
pub fn debug_log_toggle() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");
    let is_open = use_state(|| false);

    let on_toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| {
            is_open.set(!*is_open);
        })
    };

    let on_close = {
        let is_open = is_open.clone();
        Callback::from(move |_| {
            is_open.set(false);
        })
    };

    let frame = state.game_state.current_frame();
    let hash = state.game_state.compute_hash();

    let phase_str = match &state.phase {
        P2PPhase::Disconnected => "Disconnected",
        P2PPhase::Connecting => "Connecting",
        P2PPhase::WaitingForPeers => "Waiting for Peers",
        P2PPhase::Lobby => "Lobby",
        P2PPhase::Countdown { remaining_frames } => {
            let seconds = (remaining_frames / 60 + 1) as u64;
            return render_with_countdown(
                &state,
                &is_open,
                on_toggle,
                on_close,
                frame,
                hash,
                seconds,
            );
        }
        P2PPhase::Starting => "Starting",
        P2PPhase::Running => "Running",
        P2PPhase::Resyncing => "Resyncing",
        P2PPhase::Reconnecting => "Reconnecting",
        P2PPhase::Finished => "Finished",
    };

    render_debug_popup(&state, &is_open, on_toggle, on_close, frame, hash, phase_str, None)
}

fn render_with_countdown(
    state: &P2PStateContext,
    is_open: &UseStateHandle<bool>,
    on_toggle: Callback<MouseEvent>,
    on_close: Callback<MouseEvent>,
    frame: u64,
    hash: u64,
    countdown_seconds: u64,
) -> Html {
    let phase_str = format!("Countdown ({})", countdown_seconds);
    render_debug_popup(
        state,
        is_open,
        on_toggle,
        on_close,
        frame,
        hash,
        &phase_str,
        Some(countdown_seconds),
    )
}

fn render_debug_popup(
    state: &P2PStateContext,
    is_open: &UseStateHandle<bool>,
    on_toggle: Callback<MouseEvent>,
    on_close: Callback<MouseEvent>,
    frame: u64,
    hash: u64,
    phase_str: &str,
    _countdown: Option<u64>,
) -> Html {
    html! {
        <>
            // Toggle button
            <button class="debug-log-toggle-btn" onclick={on_toggle}>
                { "Debug" }
            </button>

            // Debug popup
            { if **is_open {
                html! {
                    <div class="debug-log-popup">
                        <div class="debug-log-header">
                            <span>{ "Debug Info" }</span>
                            <button class="debug-log-close-btn" onclick={on_close}>
                                { "X" }
                            </button>
                        </div>

                        // Game Status Section
                        <div class="debug-status-section">
                            <div class="debug-status-title">{ "Game Status" }</div>
                            <div class="debug-status-row">
                                <span class="debug-label">{ "Phase:" }</span>
                                <span class="debug-value">{ phase_str }</span>
                            </div>
                            <div class="debug-status-row">
                                <span class="debug-label">{ "Frame:" }</span>
                                <span class="debug-value">{ frame }</span>
                            </div>
                            <div class="debug-status-row">
                                <span class="debug-label">{ "Hash:" }</span>
                                <span class="debug-value hash">{ format!("{:016X}", hash) }</span>
                            </div>
                        </div>

                        // Event Log Section
                        <div class="debug-log-section">
                            <div class="debug-status-title">{ "Event Log" }</div>
                            <div class="debug-log-content">
                                { if state.logs.is_empty() {
                                    html! { <div class="debug-log-empty">{ "No events yet..." }</div> }
                                } else {
                                    html! {
                                        { for state.logs.iter().rev().map(|log| html! {
                                            <div class="debug-log-entry">{ log }</div>
                                        })}
                                    }
                                }}
                            </div>
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </>
    }
}
