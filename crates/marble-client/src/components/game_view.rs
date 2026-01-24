//! GameView component - main game view with P2P integration.

use yew::prelude::*;

use crate::hooks::{use_config_secret, use_config_username, use_p2p_room_with_credentials, P2pRoomConfig};
use super::{ChatPanel, PeerList};

/// Props for the GameView component.
#[derive(Properties, PartialEq)]
pub struct GameViewProps {
    /// Room ID to connect to
    pub room_id: String,
    /// Signaling server URL
    pub signaling_url: String,
}

/// GameView component - P2P-enabled game view with overlays.
///
/// Contains:
/// - PeerList (left side, vertically centered)
/// - ChatPanel (bottom-right corner)
/// - Game canvas area (future implementation)
#[function_component(GameView)]
pub fn game_view(props: &GameViewProps) -> Html {
    let config_username = use_config_username();
    let config_secret = use_config_secret();

    let player_id = config_username
        .as_ref()
        .map(|x| x.to_string())
        .unwrap_or_default();
    let player_secret = config_secret.to_string();

    let config = P2pRoomConfig {
        signaling_url: Some(props.signaling_url.clone()),
        auto_connect: true,
        ..Default::default()
    };

    let p2p = use_p2p_room_with_credentials(&props.room_id, &player_id, &player_secret, config);

    let peers = p2p.peers();
    let connection_state = p2p.state();
    let messages = p2p.messages();
    let is_connected = matches!(connection_state, crate::services::p2p::P2pConnectionState::Connected);

    html! {
        <div class="game-view">
            // Left side: Peer list (vertically centered)
            <PeerList
                peers={peers}
                my_player_id={player_id.clone()}
                connection_state={connection_state}
            />

            // Center: Game canvas area (future)
            <div class="game-canvas-placeholder">
                <span class="placeholder-text">{"Game Area"}</span>
            </div>

            // Bottom-right: Chat panel
            <ChatPanel
                p2p={p2p}
                is_connected={is_connected}
                messages={messages}
                my_player_id={player_id}
            />
        </div>
    }
}
