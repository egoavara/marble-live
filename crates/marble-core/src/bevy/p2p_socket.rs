//! P2P socket resource for Bevy ECS.
//!
//! Wraps `matchbox_socket::WebRtcSocket` in a Bevy Resource.
//! The socket is `!Send` natively, but in WASM single-threaded
//! environment we can safely implement Send/Sync.

use std::collections::HashMap;

use bevy::prelude::*;
use matchbox_socket::{PeerId, WebRtcSocket};

/// Wrapper around `WebRtcSocket` that implements Send/Sync for WASM.
///
/// # Safety
/// WASM runs on a single thread, so there are no data races.
/// This wrapper must only be used in `target_arch = "wasm32"`.
pub struct P2pSocketWrapper(pub WebRtcSocket);

unsafe impl Send for P2pSocketWrapper {}
unsafe impl Sync for P2pSocketWrapper {}

/// P2P socket and related state as a Bevy Resource.
///
/// Inserted by `pickup_pending_p2p` system when `init_p2p_socket()` is called from JS.
/// Removed by `handle_p2p_disconnect` system when `disconnect_p2p()` is called.
#[derive(Resource)]
pub struct P2pSocketRes {
    /// The WebRTC socket wrapper.
    pub socket: P2pSocketWrapper,
    /// This player's ID string.
    pub player_id: String,
    /// Whether this client is the game host.
    pub is_host: bool,
    /// The host's peer ID (if known).
    pub host_peer_id: Option<PeerId>,
    /// Currently connected peer IDs.
    pub connected_peers: Vec<PeerId>,
    /// Mapping from peer_id to player_id (resolved via server).
    pub peer_player_map: HashMap<PeerId, String>,
}
