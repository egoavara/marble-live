//! Debug panel component for displaying simulation state.

use crate::state::AppStateContext;
use marble_core::GamePhase;
use yew::prelude::*;

/// Properties for the DebugPanel component.
#[derive(Properties, PartialEq)]
pub struct DebugPanelProps {}

/// Debug panel showing FPS, frame number, and physics state.
#[function_component(DebugPanel)]
pub fn debug_panel(_props: &DebugPanelProps) -> Html {
    let app_state = use_context::<AppStateContext>().expect("AppStateContext not found");

    let phase_str = match app_state.phase() {
        GamePhase::Lobby => "Lobby".to_string(),
        GamePhase::Countdown { remaining_frames } => {
            let seconds = *remaining_frames as f32 / 60.0;
            format!("Countdown: {seconds:.1}s")
        }
        GamePhase::Running => "Running".to_string(),
        GamePhase::Finished { winner } => match winner {
            Some(id) => format!("Finished - Winner: Player {}", id + 1),
            None => "Finished - No Winner".to_string(),
        },
    };

    let frame = app_state.frame();
    let marble_count = app_state.game_state.marble_manager.marbles().len();
    let active_count = app_state.game_state.marble_manager.active_count();
    let hash = app_state.game_state.compute_hash();

    html! {
        <div class="debug-panel">
            <h3>{ "Debug Info" }</h3>
            <div class="debug-row">
                <span class="debug-label">{ "Phase:" }</span>
                <span class="debug-value">{ phase_str }</span>
            </div>
            <div class="debug-row">
                <span class="debug-label">{ "Frame:" }</span>
                <span class="debug-value">{ frame }</span>
            </div>
            <div class="debug-row">
                <span class="debug-label">{ "Marbles:" }</span>
                <span class="debug-value">{ format!("{active_count}/{marble_count}") }</span>
            </div>
            <div class="debug-row">
                <span class="debug-label">{ "Hash:" }</span>
                <span class="debug-value hash">{ format!("{hash:016x}") }</span>
            </div>
            <div class="debug-row">
                <span class="debug-label">{ "Running:" }</span>
                <span class="debug-value">{ if app_state.is_running { "Yes" } else { "No" } }</span>
            </div>
        </div>
    }
}
