//! ECS-based sync snapshot for P2P state restoration.
//!
//! Unlike the legacy `SyncSnapshot` which depends on `PhysicsWorld` and
//! `MarbleManager`, this snapshot captures marble state from Bevy ECS
//! components (Transform, Velocity, Marble, MarbleVisual).

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::bevy::resources::ActivatedKeyframes;
use crate::game::Player;
use crate::keyframe::KeyframeExecutor;
use crate::marble::{Color, PlayerId};

/// Snapshot of a single marble's state.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarbleSnapshot {
    /// Owner player ID.
    pub owner_id: PlayerId,
    /// Whether this marble has been eliminated.
    pub eliminated: bool,
    /// Marble color.
    pub color: Color,
    /// Marble radius.
    pub radius: f32,
    /// World position [x, y].
    pub position: [f32; 2],
    /// Rotation in radians.
    pub rotation: f32,
    /// Linear velocity [x, y].
    pub linear_velocity: [f32; 2],
    /// Angular velocity (radians/sec).
    pub angular_velocity: f32,
}

/// Snapshot of a keyframe-animated map object's transform.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapObjectTransformSnapshot {
    /// Object ID (matches KeyframeTarget.object_id).
    pub object_id: String,
    /// World position [x, y].
    pub position: [f32; 2],
    /// Rotation in radians.
    pub rotation: f32,
}

/// Complete ECS-based game state snapshot for P2P synchronization.
///
/// Contains all information needed to reconstruct the game state on a peer:
/// - Game metadata (frame, seed, gamerule)
/// - Player list and arrival order
/// - Per-marble physics state
/// - Keyframe executor state (animation progress)
/// - Map object transforms (current animated positions)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BevySyncSnapshot {
    /// Current simulation frame.
    pub frame: u64,
    /// RNG seed for deterministic replay.
    pub rng_seed: u64,
    /// DeterministicRng full state (seed + stream + word_pos).
    #[serde(default)]
    pub det_rng: Option<ChaCha8Rng>,
    /// GameContext RNG full state.
    #[serde(default)]
    pub game_ctx_rng: Option<ChaCha8Rng>,
    /// GameContext time value.
    #[serde(default)]
    pub game_ctx_time: f32,
    /// Player list.
    pub players: Vec<Player>,
    /// Order in which marbles arrived at triggers.
    pub arrival_order: Vec<PlayerId>,
    /// Selected game rule.
    pub selected_gamerule: String,
    /// Per-marble state snapshots.
    pub marbles: Vec<MarbleSnapshot>,
    /// Keyframe executor states (animation progress).
    #[serde(default)]
    pub keyframe_executors: Vec<KeyframeExecutor>,
    /// Which keyframes are currently activated.
    #[serde(default)]
    pub activated_keyframes: ActivatedKeyframes,
    /// Current transforms of keyframe-animated map objects.
    #[serde(default)]
    pub map_object_transforms: Vec<MapObjectTransformSnapshot>,
    /// Serialized PhysicsWorld bytes for complete Rapier state restoration.
    /// When present, this takes priority over marble-level position/velocity sync.
    #[serde(default)]
    pub physics_world_bytes: Vec<u8>,
}

impl BevySyncSnapshot {
    /// Serialize to bytes using postcard.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        postcard::to_allocvec(self).map_err(|e| e.to_string())
    }

    /// Deserialize from bytes using postcard.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(data).map_err(|e| e.to_string())
    }
}
