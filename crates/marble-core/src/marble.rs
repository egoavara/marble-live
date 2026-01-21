//! Marble entity system with deterministic spawning.

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rapier2d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::map::SpawnArea;
use crate::physics::PhysicsWorld;

/// Unique identifier for a marble.
pub type MarbleId = u32;

/// Unique identifier for a player.
pub type PlayerId = u32;

/// RGBA color representation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Predefined colors for marbles.
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const PURPLE: Color = Color::rgb(128, 0, 128);
    pub const ORANGE: Color = Color::rgb(255, 165, 0);
    pub const CYAN: Color = Color::rgb(0, 255, 255);
    pub const PINK: Color = Color::rgb(255, 192, 203);

    /// Returns a list of default marble colors.
    pub fn palette() -> Vec<Color> {
        vec![
            Self::RED,
            Self::BLUE,
            Self::GREEN,
            Self::YELLOW,
            Self::PURPLE,
            Self::ORANGE,
            Self::CYAN,
            Self::PINK,
        ]
    }
}

/// Default marble radius in pixels.
pub const DEFAULT_MARBLE_RADIUS: f32 = 25.0;

/// Marble entity representing a player's marble in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marble {
    pub id: MarbleId,
    pub owner_id: PlayerId,
    pub body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    pub color: Color,
    pub eliminated: bool,
    pub radius: f32,
}

impl Marble {
    /// Creates a new marble entity.
    pub fn new(
        id: MarbleId,
        owner_id: PlayerId,
        body_handle: RigidBodyHandle,
        collider_handle: ColliderHandle,
        color: Color,
        radius: f32,
    ) -> Self {
        Self {
            id,
            owner_id,
            body_handle,
            collider_handle,
            color,
            eliminated: false,
            radius,
        }
    }

    /// Marks the marble as eliminated.
    pub fn eliminate(&mut self) {
        self.eliminated = true;
    }
}

/// Manages marble entities in the physics world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarbleManager {
    marbles: Vec<Marble>,
    next_id: MarbleId,
    #[serde(skip, default)]
    rng: Option<ChaCha8Rng>,
    seed: u64,
}

impl MarbleManager {
    /// Creates a new marble manager with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        Self {
            marbles: Vec::new(),
            next_id: 0,
            rng: Some(ChaCha8Rng::seed_from_u64(seed)),
            seed,
        }
    }

    /// Reinitializes the RNG (used after deserialization).
    pub fn reinit_rng(&mut self) {
        self.rng = Some(ChaCha8Rng::seed_from_u64(self.seed));
        // Fast-forward RNG to match the number of marbles created
        if let Some(rng) = &mut self.rng {
            // Each marble spawn uses 2 random numbers (x, y position)
            for _ in 0..(self.next_id * 2) {
                let _: f32 = rng.random();
            }
        }
    }

    /// Spawns a new marble at a random position within the spawn area.
    pub fn spawn_marble(
        &mut self,
        world: &mut PhysicsWorld,
        owner_id: PlayerId,
        color: Color,
        spawn_area: &SpawnArea,
    ) -> MarbleId {
        self.spawn_marble_with_radius(world, owner_id, color, spawn_area, DEFAULT_MARBLE_RADIUS)
    }

    /// Spawns a new marble with a custom radius.
    pub fn spawn_marble_with_radius(
        &mut self,
        world: &mut PhysicsWorld,
        owner_id: PlayerId,
        color: Color,
        spawn_area: &SpawnArea,
        radius: f32,
    ) -> MarbleId {
        let rng = self.rng.as_mut().expect("RNG not initialized");

        // Generate random position within spawn area
        let x = rng.random_range(spawn_area.x[0]..spawn_area.x[1]);
        let y = rng.random_range(spawn_area.y[0]..spawn_area.y[1]);

        self.spawn_marble_at(world, owner_id, color, x, y, radius)
    }

    /// Spawns a marble at a specific position.
    pub fn spawn_marble_at(
        &mut self,
        world: &mut PhysicsWorld,
        owner_id: PlayerId,
        color: Color,
        x: f32,
        y: f32,
        radius: f32,
    ) -> MarbleId {
        let id = self.next_id;
        self.next_id += 1;

        // Create rigid body
        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(x, y))
            .linear_damping(0.5)
            .angular_damping(0.5)
            .ccd_enabled(true)
            .build();

        let body_handle = world.add_rigid_body(rigid_body);

        // Create collider
        let collider = ColliderBuilder::ball(radius)
            .restitution(0.7)
            .friction(0.3)
            .density(1.0)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build();

        let collider_handle = world.add_collider(collider, body_handle);

        let marble = Marble::new(id, owner_id, body_handle, collider_handle, color, radius);
        self.marbles.push(marble);

        id
    }

    /// Removes a marble from the physics world.
    pub fn remove_marble(&mut self, world: &mut PhysicsWorld, marble_id: MarbleId) -> bool {
        if let Some(pos) = self.marbles.iter().position(|m| m.id == marble_id) {
            let marble = self.marbles.remove(pos);
            world.remove_rigid_body(marble.body_handle);
            true
        } else {
            false
        }
    }

    /// Gets a marble by ID.
    pub fn get_marble(&self, marble_id: MarbleId) -> Option<&Marble> {
        self.marbles.iter().find(|m| m.id == marble_id)
    }

    /// Gets a mutable reference to a marble by ID.
    pub fn get_marble_mut(&mut self, marble_id: MarbleId) -> Option<&mut Marble> {
        self.marbles.iter_mut().find(|m| m.id == marble_id)
    }

    /// Gets a marble by its collider handle.
    pub fn get_marble_by_collider(&self, handle: ColliderHandle) -> Option<&Marble> {
        self.marbles.iter().find(|m| m.collider_handle == handle)
    }

    /// Gets a mutable marble by its collider handle.
    pub fn get_marble_by_collider_mut(&mut self, handle: ColliderHandle) -> Option<&mut Marble> {
        self.marbles.iter_mut().find(|m| m.collider_handle == handle)
    }

    /// Returns all marbles.
    pub fn marbles(&self) -> &[Marble] {
        &self.marbles
    }

    /// Returns all active (non-eliminated) marbles.
    pub fn active_marbles(&self) -> Vec<&Marble> {
        self.marbles.iter().filter(|m| !m.eliminated).collect()
    }

    /// Returns all eliminated marbles.
    pub fn eliminated_marbles(&self) -> Vec<&Marble> {
        self.marbles.iter().filter(|m| m.eliminated).collect()
    }

    /// Returns the number of active marbles.
    pub fn active_count(&self) -> usize {
        self.marbles.iter().filter(|m| !m.eliminated).count()
    }

    /// Checks if a marble collides with any hole and marks it as eliminated.
    /// Returns the IDs of newly eliminated marbles.
    pub fn check_hole_collisions(
        &mut self,
        world: &PhysicsWorld,
        hole_handles: &[ColliderHandle],
    ) -> Vec<MarbleId> {
        let mut eliminated = Vec::new();

        for marble in &mut self.marbles {
            if marble.eliminated {
                continue;
            }

            // Check intersection with each hole
            for &hole_handle in hole_handles {
                let Some(hole_collider) = world.collider_set.get(hole_handle) else {
                    continue;
                };
                let Some(marble_body) = world.get_rigid_body(marble.body_handle) else {
                    continue;
                };

                let marble_pos = marble_body.translation();
                let hole_pos = hole_collider.translation();

                // Simple distance check (both are balls)
                let dx = marble_pos.x - hole_pos.x;
                let dy = marble_pos.y - hole_pos.y;
                let dist_sq = dx * dx + dy * dy;

                // Get hole radius from collider shape
                if let Some(ball) = hole_collider.shape().as_ball() {
                    // Marble center must be inside the hole
                    let threshold = ball.radius;
                    if dist_sq < threshold * threshold {
                        marble.eliminated = true;
                        eliminated.push(marble.id);
                        break;
                    }
                }
            }
        }

        eliminated
    }

    /// Gets the position of a marble.
    pub fn get_marble_position(&self, world: &PhysicsWorld, marble_id: MarbleId) -> Option<(f32, f32)> {
        self.get_marble(marble_id).and_then(|marble| {
            world.get_rigid_body(marble.body_handle).map(|body| {
                let pos = body.translation();
                (pos.x, pos.y)
            })
        })
    }

    /// Gets the velocity of a marble.
    pub fn get_marble_velocity(&self, world: &PhysicsWorld, marble_id: MarbleId) -> Option<(f32, f32)> {
        self.get_marble(marble_id).and_then(|marble| {
            world.get_rigid_body(marble.body_handle).map(|body| {
                let vel = body.linvel();
                (vel.x, vel.y)
            })
        })
    }

    /// Resets all marbles (removes them from the world).
    pub fn clear(&mut self, world: &mut PhysicsWorld) {
        for marble in self.marbles.drain(..) {
            world.remove_rigid_body(marble.body_handle);
        }
        self.next_id = 0;
        self.rng = Some(ChaCha8Rng::seed_from_u64(self.seed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::RouletteConfig;

    #[test]
    fn test_marble_creation() {
        let mut world = PhysicsWorld::new();
        let mut manager = MarbleManager::new(12345);
        let spawn_area = SpawnArea {
            x: [100.0, 200.0],
            y: [50.0, 100.0],
        };

        let id = manager.spawn_marble(&mut world, 1, Color::RED, &spawn_area);

        assert_eq!(id, 0);
        assert!(manager.get_marble(id).is_some());
        assert_eq!(manager.marbles().len(), 1);
    }

    #[test]
    fn test_deterministic_spawning() {
        let spawn_area = SpawnArea {
            x: [100.0, 700.0],
            y: [50.0, 150.0],
        };

        // Create two identical managers
        let mut world1 = PhysicsWorld::new();
        let mut manager1 = MarbleManager::new(42);

        let mut world2 = PhysicsWorld::new();
        let mut manager2 = MarbleManager::new(42);

        // Spawn marbles in both
        for i in 0..5 {
            let color = Color::palette()[i % Color::palette().len()];
            manager1.spawn_marble(&mut world1, i as u32, color, &spawn_area);
            manager2.spawn_marble(&mut world2, i as u32, color, &spawn_area);
        }

        // Positions should be identical
        for i in 0..5 {
            let pos1 = manager1.get_marble_position(&world1, i as u32);
            let pos2 = manager2.get_marble_position(&world2, i as u32);

            assert_eq!(pos1, pos2, "Position mismatch for marble {}", i);
        }
    }

    #[test]
    fn test_marble_elimination() {
        let mut world = PhysicsWorld::new();
        let config = RouletteConfig::default_classic();
        let hole_handles = config.apply_to_world(&mut world);

        let mut manager = MarbleManager::new(12345);

        // Spawn a marble directly in the hole
        let hole_center = config.holes[0].center;
        let id = manager.spawn_marble_at(
            &mut world,
            1,
            Color::BLUE,
            hole_center[0],
            hole_center[1],
            DEFAULT_MARBLE_RADIUS,
        );

        // Check collisions
        let eliminated = manager.check_hole_collisions(&world, &hole_handles);

        assert!(eliminated.contains(&id));
        assert!(manager.get_marble(id).unwrap().eliminated);
    }

    #[test]
    fn test_marble_removal() {
        let mut world = PhysicsWorld::new();
        let mut manager = MarbleManager::new(12345);
        let spawn_area = SpawnArea {
            x: [100.0, 200.0],
            y: [50.0, 100.0],
        };

        let id = manager.spawn_marble(&mut world, 1, Color::RED, &spawn_area);
        assert_eq!(manager.marbles().len(), 1);

        let removed = manager.remove_marble(&mut world, id);
        assert!(removed);
        assert_eq!(manager.marbles().len(), 0);
        assert!(manager.get_marble(id).is_none());
    }

    #[test]
    fn test_active_vs_eliminated() {
        let mut world = PhysicsWorld::new();
        let mut manager = MarbleManager::new(12345);
        let spawn_area = SpawnArea {
            x: [100.0, 200.0],
            y: [50.0, 100.0],
        };

        // Spawn 3 marbles
        for i in 0..3 {
            manager.spawn_marble(&mut world, i, Color::RED, &spawn_area);
        }

        assert_eq!(manager.active_count(), 3);
        assert_eq!(manager.eliminated_marbles().len(), 0);

        // Eliminate one
        manager.get_marble_mut(1).unwrap().eliminate();

        assert_eq!(manager.active_count(), 2);
        assert_eq!(manager.eliminated_marbles().len(), 1);
    }
}
