//! Custom Rapier2D physics plugin for Bevy.
//!
//! Replaces `bevy_rapier2d` with direct Rapier2D integration via `PhysicsWorld`.
//! This gives full control over the physics state, enabling:
//! - Complete state serialization/deserialization for P2P sync
//! - Deterministic entity-handle mapping via `user_data`
//! - No hidden internal state that could cause desync

use bevy::prelude::*;
use rapier2d::prelude::*;

use crate::physics::PhysicsWorld;

// ============================================================================
// Resources
// ============================================================================

/// Bevy Resource wrapping `PhysicsWorld` for direct Rapier access.
#[derive(Resource)]
pub struct PhysicsWorldRes {
    pub world: PhysicsWorld,
    /// Collision events collected during the last physics step.
    collision_events: Vec<PhysicsCollisionEvent>,
}

impl PhysicsWorldRes {
    pub fn new() -> Self {
        Self {
            world: PhysicsWorld::new(),
            collision_events: Vec::new(),
        }
    }

    /// Returns the collision events from the last step and clears the buffer.
    pub fn drain_collision_events(&mut self) -> Vec<PhysicsCollisionEvent> {
        std::mem::take(&mut self.collision_events)
    }
}

impl Default for PhysicsWorldRes {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Components
// ============================================================================

/// Entity ↔ RigidBody mapping component.
#[derive(Component, Debug, Clone, Copy)]
pub struct PhysicsBody(pub RigidBodyHandle);

/// Entity ↔ Collider mapping (for sensor/trigger colliders without a body).
#[derive(Component, Debug, Clone, Copy)]
pub struct PhysicsCollider(pub ColliderHandle);

/// External force accumulator component (replaces `bevy_rapier2d::ExternalForce`).
#[derive(Component, Default, Debug, Clone)]
pub struct PhysicsExternalForce {
    pub force: Vec2,
    pub torque: f32,
}

/// Marker component for sensor colliders (replaces `bevy_rapier2d::Sensor`).
#[derive(Component, Default, Debug, Clone)]
pub struct Sensor;

// ============================================================================
// System Sets
// ============================================================================

/// Custom PhysicsSet (replaces `bevy_rapier2d::PhysicsSet`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhysicsSet {
    /// Sync Bevy state → Rapier (replaces `PhysicsSet::SyncBackend`).
    SyncToRapier,
    /// Run physics simulation step.
    Step,
    /// Sync Rapier state → Bevy (replaces `PhysicsSet::Writeback`).
    SyncFromRapier,
}

// ============================================================================
// Collision Events
// ============================================================================

/// Collision event from the physics simulation.
#[derive(Debug, Clone)]
pub enum PhysicsCollisionEvent {
    Started(Entity, Entity),
    Stopped(Entity, Entity),
}

/// Bevy Message for collision events (for system communication).
#[derive(Message, Debug, Clone)]
pub enum CollisionEvent {
    Started(Entity, Entity, CollisionEventFlags),
    Stopped(Entity, Entity, CollisionEventFlags),
}

/// Flags for collision events (compatibility with bevy_rapier2d interface).
#[derive(Debug, Clone, Copy, Default)]
pub struct CollisionEventFlags;

// ============================================================================
// user_data encoding for P2P-stable entity identification
// ============================================================================

/// Type tags for user_data encoding.
pub const USER_DATA_MARBLE: u64 = 1;
pub const USER_DATA_MAP_OBJECT: u64 = 2;
pub const USER_DATA_TRIGGER: u64 = 3;

/// Encodes a type tag and ID into u128 user_data.
pub fn encode_user_data(type_tag: u64, id: u64) -> u128 {
    ((type_tag as u128) << 64) | (id as u128)
}

/// Decodes u128 user_data into (type_tag, id).
pub fn decode_user_data(user_data: u128) -> (u64, u64) {
    let type_tag = (user_data >> 64) as u64;
    let id = user_data as u64;
    (type_tag, id)
}

// (CollisionCollector removed — event collection is handled inside PhysicsWorld::step_with_events)

// ============================================================================
// Physics Systems
// ============================================================================

/// Syncs Bevy component state into Rapier bodies.
///
/// - Animated obstacles: `Transform` → kinematic body `next_position`/`next_rotation`
/// - External forces: `PhysicsExternalForce` → rapier body `add_force`
pub fn sync_to_rapier(
    mut physics: ResMut<PhysicsWorldRes>,
    animated_bodies: Query<
        (&PhysicsBody, &Transform, &crate::bevy::AnimatedObject),
        Without<crate::bevy::Marble>,
    >,
    force_bodies: Query<(&PhysicsBody, &PhysicsExternalForce)>,
) {
    // 1. Sync animated (kinematic) obstacle transforms
    for (body_comp, transform, _animated) in animated_bodies.iter() {
        if let Some(body) = physics.world.rigid_body_set.get_mut(body_comp.0) {
            if body.is_kinematic() {
                let pos = transform.translation.truncate();
                let rot = transform.rotation.to_euler(EulerRot::ZYX).0;
                body.set_next_kinematic_translation(Vector::new(pos.x, pos.y));
                body.set_next_kinematic_rotation(Rotation::from_angle(rot));
            }
        }
    }

    // 2. Reset Rapier body forces then apply this frame's external forces.
    //    Without reset, add_force() accumulates across frames.
    for (body_comp, ext_force) in force_bodies.iter() {
        if let Some(body) = physics.world.rigid_body_set.get_mut(body_comp.0) {
            body.reset_forces(false);
            body.reset_torques(false);

            if ext_force.force.length_squared() < f32::EPSILON
                && ext_force.torque.abs() < f32::EPSILON
            {
                continue;
            }
            body.add_force(Vector::new(ext_force.force.x, ext_force.force.y), true);
            body.add_torque(ext_force.torque, true);
        }
    }
}

/// Runs one physics simulation step and collects collision events.
pub fn run_physics_step(mut physics: ResMut<PhysicsWorldRes>) {
    // step_with_events() encapsulates the RefCell + EventHandler internally,
    // avoiding borrow-checker issues with multiple &mut fields.
    let raw = physics.world.step_with_events();

    // Convert raw rapier events → PhysicsCollisionEvent with entity mapping
    let mut bevy_events = Vec::with_capacity(raw.len());

    for event in raw {
        match event {
            rapier2d::prelude::CollisionEvent::Started(h1, h2, _flags) => {
                let e1 = collider_to_entity(&physics.world, h1);
                let e2 = collider_to_entity(&physics.world, h2);
                if let (Some(e1), Some(e2)) = (e1, e2) {
                    bevy_events.push(PhysicsCollisionEvent::Started(e1, e2));
                }
            }
            rapier2d::prelude::CollisionEvent::Stopped(h1, h2, _flags) => {
                let e1 = collider_to_entity(&physics.world, h1);
                let e2 = collider_to_entity(&physics.world, h2);
                if let (Some(e1), Some(e2)) = (e1, e2) {
                    bevy_events.push(PhysicsCollisionEvent::Stopped(e1, e2));
                }
            }
        }
    }

    physics.collision_events = bevy_events;
}

/// Syncs Rapier body state back to Bevy Transforms.
///
/// Also publishes collision events as Bevy Messages.
pub fn sync_from_rapier(
    physics: Res<PhysicsWorldRes>,
    mut bodies: Query<(&PhysicsBody, &mut Transform), Without<crate::bevy::AnimatedObject>>,
) {
    for (body_comp, mut transform) in bodies.iter_mut() {
        if let Some(body) = physics.world.rigid_body_set.get(body_comp.0) {
            if body.is_dynamic() {
                let pos = body.translation();
                let rot = body.rotation().angle();
                transform.translation.x = pos.x;
                transform.translation.y = pos.y;
                transform.rotation = Quat::from_rotation_z(rot);
            }
        }
    }
}

/// Publishes collision events as Bevy Messages.
pub fn publish_collision_events(
    mut physics: ResMut<PhysicsWorldRes>,
    mut writer: MessageWriter<CollisionEvent>,
) {
    for event in physics.drain_collision_events() {
        match event {
            PhysicsCollisionEvent::Started(e1, e2) => {
                writer.write(CollisionEvent::Started(e1, e2, CollisionEventFlags));
            }
            PhysicsCollisionEvent::Stopped(e1, e2) => {
                writer.write(CollisionEvent::Stopped(e1, e2, CollisionEventFlags));
            }
        }
    }
}

// ============================================================================
// Helper: ColliderHandle → Entity via user_data
// ============================================================================

/// Maps a Rapier ColliderHandle to a Bevy Entity via user_data stored in the
/// collider's parent body or the collider itself.
fn collider_to_entity(world: &PhysicsWorld, handle: ColliderHandle) -> Option<Entity> {
    let collider = world.collider_set.get(handle)?;
    let user_data = if let Some(parent) = collider.parent() {
        // Collider attached to a body: use body's user_data
        world.rigid_body_set.get(parent)?.user_data
    } else {
        // Static collider without body: use collider's own user_data
        collider.user_data
    };

    if user_data == 0 {
        return None;
    }

    // user_data stores the Entity bits
    Some(Entity::from_bits(user_data as u64))
}

// ============================================================================
// Plugin
// ============================================================================

/// Custom physics plugin replacing `bevy_rapier2d`.
pub struct MarblePhysicsPlugin;

impl Plugin for MarblePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorldRes::new());

        // Register collision event message
        app.add_message::<CollisionEvent>();

        // Configure system set ordering
        app.configure_sets(
            FixedUpdate,
            (
                PhysicsSet::SyncToRapier,
                PhysicsSet::Step,
                PhysicsSet::SyncFromRapier,
            )
                .chain(),
        );

        // Register physics systems
        app.add_systems(FixedUpdate, sync_to_rapier.in_set(PhysicsSet::SyncToRapier));
        app.add_systems(FixedUpdate, run_physics_step.in_set(PhysicsSet::Step));
        app.add_systems(
            FixedUpdate,
            (sync_from_rapier, publish_collision_events)
                .chain()
                .in_set(PhysicsSet::SyncFromRapier),
        );
    }
}
