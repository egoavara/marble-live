//! Bevy-based game engine for marble-live.
//!
//! This module provides a complete Bevy integration for the marble roulette game,
//! including physics simulation via direct Rapier2D integration, ECS components,
//! resources, and systems for both game play and editor modes.

pub mod components;
pub mod events;
pub mod gossip;
pub mod plugin;
pub mod rapier_plugin;
pub mod resources;
pub mod state_store;
pub mod sync_snapshot;
pub mod systems;

#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(target_arch = "wasm32")]
pub mod p2p_socket;
#[cfg(target_arch = "wasm32")]
pub mod wasm_entry;

#[cfg(target_arch = "wasm32")]
pub use wasm_entry::*;

pub use components::*;
pub use events::*;
pub use plugin::{AppMode, EditorState, MarbleHeadlessPlugin, MarbleUnifiedPlugin};
pub use rapier_plugin::{
    CollisionEvent, CollisionEventFlags, MarblePhysicsPlugin, PhysicsBody, PhysicsCollider,
    PhysicsExternalForce, PhysicsSet, PhysicsWorldRes, Sensor,
};
pub use resources::*;
pub use state_store::{
    ChatMessage, ChatStore, ConnectionState, ConnectionStore, EditorStateSummary, EditorStore,
    GameStateStore, GameStateSummary, PeerInfo, PeerStore, PlayerInfo, PlayerStore, Reaction,
    ReactionStore, SnapConfigStore, SnapConfigSummary, StateStores,
};
pub use systems::camera::{
    apply_camera_smoothing, handle_editor_camera_input, update_follow_leader, update_follow_target,
    update_overview_camera,
};
pub use systems::editor::{
    EditorStateRes, EditorStateStore, GizmoHandle, SelectObjectEvent, UpdateObjectEvent,
};
