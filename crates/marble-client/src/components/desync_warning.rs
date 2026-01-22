//! Desync warning component.

use crate::p2p::state::{P2PPhase, P2PStateContext};
use yew::prelude::*;

/// Desync warning overlay.
#[function_component(DesyncWarning)]
pub fn desync_warning() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    if !state.desync_detected && state.phase != P2PPhase::Resyncing {
        return html! {};
    }

    html! {
        <div
            class="desync-warning"
            style="
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                background: linear-gradient(135deg, #ff5722, #f44336);
                color: white;
                padding: 15px 20px;
                text-align: center;
                z-index: 1000;
                box-shadow: 0 2px 10px rgba(0,0,0,0.3);
                animation: pulse 1s ease-in-out infinite;
            "
        >
            <div style="display: flex; align-items: center; justify-content: center; gap: 15px;">
                <span style="font-size: 24px;">{"!!!"}</span>
                <div>
                    <div style="font-size: 18px; font-weight: bold;">
                        {if state.phase == P2PPhase::Resyncing {
                            "Resyncing..."
                        } else {
                            "Desync Detected!"
                        }}
                    </div>
                    <div style="font-size: 14px; opacity: 0.9;">
                        {if state.phase == P2PPhase::Resyncing {
                            "Please wait while the game state is being synchronized..."
                        } else {
                            "Your game state doesn't match other players. Requesting resync..."
                        }}
                    </div>
                </div>
                <span style="font-size: 24px;">{"!!!"}</span>
            </div>
        </div>
    }
}

/// Event log panel for debugging.
#[function_component(EventLogPanel)]
pub fn event_log_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    html! {
        <div class="event-log-panel" style="background: #1e1e1e; padding: 15px; border-radius: 8px;">
            <h3 style="margin: 0 0 10px 0; color: #fff;">{"Event Log"}</h3>
            <div style="
                color: #d4d4d4;
                height: 200px;
                overflow-y: auto;
                font-family: monospace;
                font-size: 12px;
                line-height: 1.6;
            ">
                {if state.logs.is_empty() {
                    html! { <div style="color: #666;">{"No events yet..."}</div> }
                } else {
                    html! {
                        {for state.logs.iter().rev().map(|log| html! {
                            <div style="border-bottom: 1px solid #333; padding: 4px 0;">{log}</div>
                        })}
                    }
                }}
            </div>
        </div>
    }
}
