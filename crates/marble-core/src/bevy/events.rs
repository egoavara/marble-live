//! ECS Events (Messages) for the marble game.
//!
//! These events enable communication between systems.
//! Note: In Bevy 0.18+, buffered events use Message trait instead of Event.

use bevy::prelude::*;

use crate::marble::PlayerId;

/// Message fired when a marble arrives at a trigger.
#[derive(Message, Debug, Clone)]
pub struct MarbleArrivedEvent {
    /// The player whose marble arrived.
    pub player_id: PlayerId,
    /// The entity of the marble that arrived.
    pub marble_entity: Entity,
    /// The trigger action (e.g., "gamerule").
    pub trigger_action: String,
    /// Index of the trigger.
    pub trigger_index: usize,
}

/// Message to request spawning marbles for all players.
#[derive(Message, Debug, Clone, Default)]
pub struct SpawnMarblesEvent;

/// Message to request spawning marbles at specific positions (peer: host-provided coordinates).
#[derive(Message, Debug, Clone)]
pub struct SpawnMarblesAtEvent {
    pub positions: Vec<[f32; 2]>,
}

/// Message fired when a map has been loaded.
#[derive(Message, Debug, Clone)]
pub struct MapLoadedEvent {
    /// Name of the loaded map.
    pub map_name: String,
}

/// Message fired when a P2P sync snapshot is received.
#[derive(Message, Debug, Clone)]
pub struct SyncSnapshotReceivedEvent {
    /// Serialized snapshot data.
    pub data: Vec<u8>,
    /// Frame number of the snapshot.
    pub frame: u64,
}

/// Message to request loading a map.
#[derive(Message, Debug, Clone)]
pub struct LoadMapEvent {
    /// The map configuration to load.
    pub config: crate::map::RouletteConfig,
}

/// Message to request clearing all marbles.
#[derive(Message, Debug, Clone, Default)]
pub struct ClearMarblesEvent;

/// Message to request adding a player.
#[derive(Message, Debug, Clone)]
pub struct AddPlayerEvent {
    /// Player name.
    pub name: String,
    /// Player color.
    pub color: crate::marble::Color,
}

/// Message fired when a player is added.
#[derive(Message, Debug, Clone)]
pub struct PlayerAddedEvent {
    /// The new player's ID.
    pub player_id: PlayerId,
}

/// Message to request removing a player.
#[derive(Message, Debug, Clone)]
pub struct RemovePlayerEvent {
    /// The player ID to remove.
    pub player_id: PlayerId,
}

/// Message fired when all marbles have arrived (game over).
#[derive(Message, Debug, Clone, Default)]
pub struct GameOverEvent;

// ========== Editor Events ==========

/// Message to start simulation in editor.
#[derive(Message, Debug, Clone, Default)]
pub struct StartSimulationEvent;

/// Message to stop simulation in editor.
#[derive(Message, Debug, Clone, Default)]
pub struct StopSimulationEvent;

/// Message to reset simulation in editor.
#[derive(Message, Debug, Clone, Default)]
pub struct ResetSimulationEvent;

/// Message to toggle keyframe preview in editor.
#[derive(Message, Debug, Clone)]
pub struct PreviewSequenceEvent {
    /// Whether to start or stop preview.
    pub start: bool,
}

/// Message sent when a keyframe is updated via gizmo drag.
#[derive(Message, Debug, Clone)]
pub struct UpdateKeyframeEvent {
    /// The sequence index containing the keyframe.
    pub sequence_index: usize,
    /// The keyframe index within the sequence.
    pub keyframe_index: usize,
    /// The updated keyframe.
    pub keyframe: crate::map::Keyframe,
}

/// Message to add a new map object.
#[derive(Message, Debug, Clone)]
pub struct AddObjectEvent {
    /// The object to add.
    pub object: crate::map::MapObject,
    /// The index where the object was added.
    pub index: usize,
}

/// Message to delete a map object.
#[derive(Message, Debug, Clone)]
pub struct DeleteObjectEvent {
    /// The index of the object to delete.
    pub index: usize,
}

// ========== P2P Sync Events ==========

/// Message fired when the host should broadcast a GameStart to all peers.
#[derive(Message, Debug, Clone, Default)]
pub struct BroadcastGameStartEvent;

/// Message fired when a peer requests a sync snapshot from the host.
#[derive(Message, Debug, Clone)]
pub struct SyncSnapshotRequestEvent {
    /// The requesting peer's ID as bytes (PeerId is !Send, so stored as bytes).
    pub peer_id_bytes: Vec<u8>,
    /// The frame from which the peer wants to resync.
    pub from_frame: u64,
}
