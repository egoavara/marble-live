//! P2P Room connection hooks for Yew components.
//!
//! This module provides reusable custom hooks for P2P communication.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::services::p2p::{
    P2pConnectionState, P2pRoomConfig, P2pRoomHandle, P2pRoomState,
};

// Re-export types for convenience
pub use crate::services::p2p::{P2pPeerInfo, ReceivedMessage};

/// P2P room connection hook
///
/// # Arguments
/// * `room_id` - Room ID to connect to (empty string means no connection)
///
/// # Returns
/// P2pRoomHandle for connection control and state queries
///
/// # Example
/// ```ignore
/// #[function_component(ChatRoom)]
/// pub fn chat_room(props: &Props) -> Html {
///     let p2p = use_p2p_room(&props.room_id);
///
///     let messages = p2p.messages();
///     let peers = p2p.peers();
///
///     html! {
///         <div>
///             { for messages.iter().map(|m| html! { <p>{ &m.from_player }: { format!("{:?}", m.payload) }</p> }) }
///         </div>
///     }
/// }
/// ```
#[hook]
pub fn use_p2p_room(room_id: &str) -> P2pRoomHandle {
    use_p2p_room_with_config(room_id, P2pRoomConfig::default())
}

/// P2P room connection hook with custom configuration
///
/// # Arguments
/// * `room_id` - Room ID to connect to (empty string means no connection)
/// * `config` - Configuration options
///
/// # Returns
/// P2pRoomHandle for connection control and state queries
#[hook]
pub fn use_p2p_room_with_config(room_id: &str, config: P2pRoomConfig) -> P2pRoomHandle {
    // Generate player ID from hook (stable across renders)
    let player_id = use_memo((), |_| uuid::Uuid::new_v4().to_string());

    use_p2p_room_internal(room_id, &player_id, config)
}

/// P2P room hook with player ID override
///
/// Use this when you have a known player ID from authentication
#[hook]
pub fn use_p2p_room_with_player_id(
    room_id: &str,
    player_id: &str,
    config: P2pRoomConfig,
) -> P2pRoomHandle {
    use_p2p_room_internal(room_id, player_id, config)
}

/// P2P room hook with player credentials (ID and secret)
///
/// Use this when you need to register peer_id with the server after P2P connection.
/// The player_secret will be used for authentication when calling RegisterPeerId.
#[hook]
pub fn use_p2p_room_with_credentials(
    room_id: &str,
    player_id: &str,
    player_secret: &str,
    mut config: P2pRoomConfig,
) -> P2pRoomHandle {
    config.player_secret = Some(player_secret.to_string());
    use_p2p_room_internal(room_id, player_id, config)
}

/// Internal hook implementation shared by all variants
#[hook]
fn use_p2p_room_internal(
    room_id: &str,
    player_id: &str,
    config: P2pRoomConfig,
) -> P2pRoomHandle {
    let player_id = player_id.to_string();

    // Yew state handles for reactive updates
    let state_handle = use_state(P2pConnectionState::default);
    let peers_version = use_state(|| 0u32);
    let messages_version = use_state(|| 0u32);

    let room_id_owned = room_id.to_string();
    let config_clone = config.clone();

    // Create inner state
    let inner = use_memo(
        (room_id_owned.clone(), player_id.clone()),
        |(rid, pid)| {
            Rc::new(RefCell::new(P2pRoomState::new(
                rid.clone(),
                pid.clone(),
                config_clone.clone(),
            )))
        },
    );

    // Handle room_id changes
    {
        let inner = (*inner).clone();
        let room_id = room_id.to_string();
        let state_handle = state_handle.clone();
        let peers_version = peers_version.clone();
        let messages_version = messages_version.clone();
        let auto_connect = config.auto_connect;

        use_effect_with(room_id.clone(), move |new_room_id| {
            // Update room_id in inner state
            {
                let mut inner_mut = inner.borrow_mut();
                if inner_mut.room_id != *new_room_id {
                    // Disconnect if connected
                    inner_mut.reset_connection();
                    inner_mut.room_id = new_room_id.clone();
                }
            }

            // Reset state
            state_handle.set(P2pConnectionState::Disconnected);
            peers_version.set(*peers_version + 1);

            // Auto connect if configured and room_id is not empty
            if auto_connect && !new_room_id.is_empty() {
                let inner = inner.clone();
                let state_handle = state_handle.clone();
                let peers_version = peers_version.clone();
                let messages_version = messages_version.clone();

                spawn_local(async move {
                    P2pRoomHandle::do_connect(inner, state_handle, peers_version, messages_version)
                        .await;
                });
            }

            || ()
        });
    }

    // Cleanup on unmount
    {
        let inner = (*inner).clone();
        use_effect_with((), move |_| {
            let inner = inner.clone();
            move || {
                // Stop polling and disconnect Bevy P2P socket
                inner.borrow_mut().is_running = false;
                marble_core::bevy::wasm_entry::disconnect_p2p();
            }
        });
    }

    P2pRoomHandle {
        inner: (*inner).clone(),
        state_handle,
        peers_version,
        messages_version,
    }
}
