//! Lobby panel component for ready state and game start.

use crate::p2p::state::{P2PAction, P2PPhase, P2PStateContext};
use crate::p2p::session::get_player_list;
use crate::p2p::protocol::{P2PMessage, PlayerStartInfo};
use yew::prelude::*;

/// Lobby panel with ready button and game start.
#[function_component(LobbyPanel)]
pub fn lobby_panel() -> Html {
    let state = use_context::<P2PStateContext>().expect("P2PStateContext not found");

    let on_toggle_ready = {
        let state = state.clone();

        Callback::from(move |_| {
            let new_ready = !state.my_ready;
            state.dispatch(P2PAction::SetMyReady(new_ready));

            // Broadcast ready status to peers
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

            // Generate seed and player list
            let seed = js_sys::Date::now() as u64;
            let players = get_player_list(&state);

            // Create player start info for network message
            let player_infos: Vec<PlayerStartInfo> = players
                .iter()
                .map(|(peer_id, name, color)| PlayerStartInfo::new(*peer_id, name.clone(), *color))
                .collect();

            // Broadcast game start to all peers
            let msg = P2PMessage::GameStart {
                seed,
                players: player_infos,
            };
            state.network.borrow_mut().broadcast(&msg.encode());

            // Start game locally
            state.dispatch(P2PAction::StartGame {
                seed,
                players: players.clone(),
            });
            state.dispatch(P2PAction::StartCountdown);
        })
    };

    let on_disconnect = {
        let state = state.clone();

        Callback::from(move |_| {
            state.network.borrow_mut().disconnect();
            state.dispatch(P2PAction::SetDisconnected);
        })
    };

    let all_ready = state.all_peers_ready();
    let can_start = state.is_host && all_ready && state.player_count() >= 2;

    html! {
        <div class="lobby-panel" style="background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);">
            // Room info
            {if !state.room_id.is_empty() {
                html! {
                    <div style="margin-bottom: 20px; padding: 15px; background: #e8f5e9; border-radius: 8px; border: 1px solid #c8e6c9;">
                        <strong style="color: #333;">{"Room ID (share this): "}</strong>
                        <code style="background: #fff; padding: 4px 8px; border-radius: 4px; font-size: 14px; user-select: all; color: #333;">
                            {&state.room_id}
                        </code>
                    </div>
                }
            } else {
                html! {}
            }}

            // Player count
            <div style="margin-bottom: 20px; text-align: center;">
                <span style="font-size: 24px; font-weight: bold; color: #333;">
                    {format!("{} Player{}", state.player_count(), if state.player_count() == 1 { "" } else { "s" })}
                </span>
                <div style="color: #666; font-size: 14px;">
                    {if state.player_count() < 2 {
                        "Need at least 2 players to start"
                    } else if !all_ready {
                        "Waiting for all players to be ready"
                    } else if !state.is_host {
                        "Waiting for host to start the game"
                    } else {
                        "Ready to start!"
                    }}
                </div>
            </div>

            // Ready button
            <div style="margin-bottom: 15px;">
                <button
                    onclick={on_toggle_ready}
                    style={format!(
                        "width: 100%; padding: 15px 30px; font-size: 18px; cursor: pointer; border: none; border-radius: 4px; color: white; background: {};",
                        if state.my_ready { "#f44336" } else { "#4CAF50" }
                    )}
                >
                    {if state.my_ready { "Cancel Ready" } else { "Ready!" }}
                </button>
            </div>

            // Start game button (host only)
            {if state.is_host {
                html! {
                    <div style="margin-bottom: 15px;">
                        <button
                            onclick={on_start_game}
                            disabled={!can_start}
                            style={format!(
                                "width: 100%; padding: 15px 30px; font-size: 18px; cursor: {}; border: none; border-radius: 4px; color: white; background: {}; opacity: {};",
                                if can_start { "pointer" } else { "not-allowed" },
                                "#ff9800",
                                if can_start { "1.0" } else { "0.5" }
                            )}
                        >
                            {"Start Game"}
                        </button>
                    </div>
                }
            } else {
                html! {}
            }}

            // Disconnect button
            <div>
                <button
                    onclick={on_disconnect}
                    style="width: 100%; padding: 10px 20px; font-size: 14px; cursor: pointer; background: #9e9e9e; color: white; border: none; border-radius: 4px;"
                >
                    {"Leave Room"}
                </button>
            </div>

            // Phase indicator
            <div style="margin-top: 20px; text-align: center; color: #666; font-size: 12px;">
                {format!("Phase: {:?}", state.phase)}
            </div>
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
        <div class="game-status-panel" style="background: white; padding: 15px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);">
            <h3 style="margin: 0 0 15px 0; color: #333;">{"Game Status"}</h3>

            // Phase
            <div style="margin-bottom: 10px;">
                <strong style="color: #333;">{"Phase: "}</strong>
                <span style={match state.phase {
                    P2PPhase::Running => "color: green;",
                    P2PPhase::Countdown { .. } => "color: orange;",
                    P2PPhase::Finished => "color: blue;",
                    P2PPhase::Resyncing => "color: red;",
                    _ => "color: #333;",
                }}>
                    {format!("{:?}", state.phase)}
                </span>
            </div>

            // Frame and hash
            <div style="margin-bottom: 10px; font-family: monospace; font-size: 12px; color: #666;">
                <div>{format!("Frame: {}", frame)}</div>
                <div>{format!("Hash: {:016X}", hash)}</div>
            </div>

            // Countdown display
            {if let P2PPhase::Countdown { remaining_frames } = state.phase {
                let seconds = remaining_frames / 60;
                html! {
                    <div style="text-align: center; font-size: 48px; font-weight: bold; color: #ff9800; margin: 20px 0;">
                        {seconds + 1}
                    </div>
                }
            } else {
                html! {}
            }}

            // Back to lobby button (when finished)
            {if state.phase == P2PPhase::Finished {
                html! {
                    <button
                        onclick={on_back_to_lobby}
                        style="width: 100%; margin-top: 15px; padding: 12px 20px; font-size: 16px; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px;"
                    >
                        {"Back to Lobby"}
                    </button>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
