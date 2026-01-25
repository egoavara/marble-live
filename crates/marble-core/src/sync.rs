//! Synchronization snapshot for P2P state restoration.

use serde::{Deserialize, Serialize};

use crate::game::{GamePhase, GameState, Player};
use crate::marble::MarbleManager;
use crate::physics::PhysicsWorld;

/// A snapshot of game state for P2P synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSnapshot {
    /// Current game phase.
    pub phase: GamePhase,
    /// Players in the game.
    pub players: Vec<Player>,
    /// Elimination order.
    pub eliminated_order: Vec<u32>,
    /// RNG seed.
    pub rng_seed: u64,
    /// Physics world state.
    pub physics_world: PhysicsWorld,
    /// Marble manager state.
    pub marble_manager: MarbleManager,
}

impl SyncSnapshot {
    /// Create a snapshot from the current game state.
    pub fn from_game_state(state: &GameState) -> Self {
        Self {
            phase: state.phase.clone(),
            players: state.players.clone(),
            eliminated_order: state.eliminated_order.clone(),
            rng_seed: state.rng_seed,
            physics_world: state.physics_world.clone(),
            marble_manager: state.marble_manager.clone(),
        }
    }

    /// Serialize the snapshot to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        postcard::to_allocvec(self).map_err(|e| e.to_string())
    }

    /// Deserialize a snapshot from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(data).map_err(|e| e.to_string())
    }

    /// Get the current frame number.
    pub fn frame(&self) -> u64 {
        self.physics_world.current_frame()
    }

    /// Compute the hash of this snapshot.
    pub fn compute_hash(&self) -> u64 {
        self.physics_world.compute_hash()
    }
}

impl GameState {
    /// Create a sync snapshot of the current state.
    pub fn create_snapshot(&self) -> SyncSnapshot {
        SyncSnapshot::from_game_state(self)
    }

    /// Restore state from a sync snapshot.
    pub fn restore_from_snapshot(&mut self, snapshot: SyncSnapshot) {
        self.phase = snapshot.phase;
        self.players = snapshot.players;
        self.eliminated_order = snapshot.eliminated_order;
        self.rng_seed = snapshot.rng_seed;
        self.physics_world = snapshot.physics_world;
        self.marble_manager = snapshot.marble_manager;

        // Reinitialize RNG after deserialization
        self.marble_manager.reinit_rng();

        // Reconstruct trigger_handles, spawners, blackholes, and kinematic_bodies from map_config
        // The physics_world already contains the colliders from the snapshot,
        // but we need to find their handles for collision detection and animation
        if let Some(ref config) = self.map_config {
            self.trigger_handles = config.find_trigger_handles(&self.physics_world);
            self.spawners = config.get_spawners();
            self.blackholes = config.get_blackholes();

            // Restore kinematic body handles
            let (kinematic_bodies, kinematic_initial_transforms) =
                config.find_kinematic_handles(&self.physics_world);
            self.kinematic_bodies = kinematic_bodies;
            self.kinematic_initial_transforms = kinematic_initial_transforms;

            // Note: keyframe_executors are not restored from snapshot
            // They will be reinitialized if needed based on the current phase
        }
    }
}
