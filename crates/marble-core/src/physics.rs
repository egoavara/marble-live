//! Physics simulation using `Rapier2D` with deterministic behavior.

use rapier2d::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Fixed timestep for physics simulation (60Hz).
pub const PHYSICS_DT: f32 = 1.0 / 60.0;

/// Default gravity vector (downward, in pixels/s²).
pub fn default_gravity() -> Vector {
    Vector::new(0.0, 981.0)
}

/// Physics world containing all `Rapier2D` components for deterministic simulation.
#[derive(Serialize, Deserialize)]
pub struct PhysicsWorld {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub integration_parameters: IntegrationParameters,
    #[serde(skip, default = "PhysicsPipeline::new")]
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub gravity: Vector,
    pub frame: u64,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PhysicsWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicsWorld")
            .field("frame", &self.frame)
            .field("rigid_body_count", &self.rigid_body_set.len())
            .field("collider_count", &self.collider_set.len())
            .field("gravity", &self.gravity)
            .finish_non_exhaustive()
    }
}

impl PhysicsWorld {
    /// Creates a new physics world with default settings.
    pub fn new() -> Self {
        Self::with_gravity(default_gravity())
    }

    /// Creates a new physics world with custom gravity.
    pub fn with_gravity(gravity: Vector) -> Self {
        let integration_parameters = IntegrationParameters {
            dt: PHYSICS_DT,
            ..Default::default()
        };

        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters,
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            gravity,
            frame: 0,
        }
    }

    /// Advances the physics simulation by one fixed timestep.
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &(),
        );
        self.frame += 1;
    }

    /// Advances the physics simulation by multiple steps.
    pub fn step_n(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Adds a rigid body to the world and returns its handle.
    pub fn add_rigid_body(&mut self, rigid_body: RigidBody) -> RigidBodyHandle {
        self.rigid_body_set.insert(rigid_body)
    }

    /// Adds a collider attached to a rigid body.
    pub fn add_collider(
        &mut self,
        collider: Collider,
        parent: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(collider, parent, &mut self.rigid_body_set)
    }

    /// Adds a collider without a parent (static collider).
    pub fn add_static_collider(&mut self, collider: Collider) -> ColliderHandle {
        self.collider_set.insert(collider)
    }

    /// Removes a rigid body and its attached colliders.
    pub fn remove_rigid_body(&mut self, handle: RigidBodyHandle) {
        self.rigid_body_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
    }

    /// Gets an immutable reference to a rigid body.
    pub fn get_rigid_body(&self, handle: RigidBodyHandle) -> Option<&RigidBody> {
        self.rigid_body_set.get(handle)
    }

    /// Gets a mutable reference to a rigid body.
    pub fn get_rigid_body_mut(&mut self, handle: RigidBodyHandle) -> Option<&mut RigidBody> {
        self.rigid_body_set.get_mut(handle)
    }

    /// Computes a deterministic hash of the current physics state.
    /// This hash can be used to verify simulation synchronization in P2P.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash frame number
        self.frame.hash(&mut hasher);

        // Hash all rigid body positions and velocities
        for (handle, body) in self.rigid_body_set.iter() {
            // Hash the handle's raw parts
            let (index, generation) = handle.into_raw_parts();
            index.hash(&mut hasher);
            generation.hash(&mut hasher);

            let pos = body.translation();
            hash_f32(pos.x, &mut hasher);
            hash_f32(pos.y, &mut hasher);

            let rot = body.rotation().angle();
            hash_f32(rot, &mut hasher);

            let linvel = body.linvel();
            hash_f32(linvel.x, &mut hasher);
            hash_f32(linvel.y, &mut hasher);

            let angvel = body.angvel();
            hash_f32(angvel, &mut hasher);
        }

        hasher.finish()
    }

    /// Returns the current simulation frame number.
    pub fn current_frame(&self) -> u64 {
        self.frame
    }

    /// Resets the physics world to its initial state.
    pub fn reset(&mut self) {
        *self = Self::with_gravity(self.gravity);
    }
}

/// Hashes a f32 value by converting to bits.
fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_world_creation() {
        let world = PhysicsWorld::new();
        assert_eq!(world.frame, 0);
        assert_eq!(world.integration_parameters.dt, PHYSICS_DT);
    }

    #[test]
    fn test_deterministic_simulation() {
        // Create two identical worlds
        let mut world1 = PhysicsWorld::new();
        let mut world2 = PhysicsWorld::new();

        // Add identical rigid bodies
        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(100.0, 100.0))
            .build();

        let collider = ColliderBuilder::ball(10.0)
            .restitution(0.7)
            .build();

        let handle1 = world1.add_rigid_body(body.clone());
        world1.add_collider(collider.clone(), handle1);

        let handle2 = world2.add_rigid_body(body);
        world2.add_collider(collider, handle2);

        // Run simulation for same number of steps
        for _ in 0..100 {
            world1.step();
            world2.step();
        }

        // Hashes should be identical
        assert_eq!(world1.compute_hash(), world2.compute_hash());

        // Positions should be identical
        let pos1 = world1.get_rigid_body(handle1).unwrap().translation();
        let pos2 = world2.get_rigid_body(handle2).unwrap().translation();
        assert_eq!(pos1.x, pos2.x);
        assert_eq!(pos1.y, pos2.y);
    }

    #[test]
    fn test_step_advances_frame() {
        let mut world = PhysicsWorld::new();
        assert_eq!(world.current_frame(), 0);

        world.step();
        assert_eq!(world.current_frame(), 1);

        world.step_n(10);
        assert_eq!(world.current_frame(), 11);
    }

    #[test]
    fn test_add_and_remove_body() {
        let mut world = PhysicsWorld::new();

        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(50.0, 50.0))
            .build();
        let handle = world.add_rigid_body(body);

        assert!(world.get_rigid_body(handle).is_some());

        world.remove_rigid_body(handle);
        assert!(world.get_rigid_body(handle).is_none());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut world = PhysicsWorld::new();

        // Add some bodies
        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(100.0, 200.0))
            .linvel(Vector::new(10.0, -5.0))
            .build();
        let handle = world.add_rigid_body(body);

        let collider = ColliderBuilder::ball(15.0).build();
        world.add_collider(collider, handle);

        // Run a few steps
        world.step_n(10);

        let hash_before = world.compute_hash();

        // Serialize and deserialize
        let serialized = serde_json::to_string(&world).expect("Failed to serialize");
        let mut deserialized: PhysicsWorld =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        // Hash should match after deserialization
        assert_eq!(hash_before, deserialized.compute_hash());

        // Running more steps should produce same results
        world.step_n(10);
        deserialized.step_n(10);

        assert_eq!(world.compute_hash(), deserialized.compute_hash());
    }
}
