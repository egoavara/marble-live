//! Roulette map configuration V2 with unified object structure.
//!
//! V2 replaces separate walls/obstacles/holes arrays with a unified `objects[]`
//! array where each object has a `role` (spawner, obstacle, trigger).
//! Supports CEL DSL expressions for dynamic properties.

use std::collections::HashMap;

use rapier2d::prelude::{ColliderBuilder, ColliderHandle, RigidBodyHandle, Vector};
use serde::{Deserialize, Serialize};

use crate::dsl::{GameContext, NumberOrExpr, Vec2OrExpr};
use crate::physics::PhysicsWorld;

/// Shape definition supporting CEL expressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    Line {
        start: Vec2OrExpr,
        end: Vec2OrExpr,
    },
    Circle {
        center: Vec2OrExpr,
        radius: NumberOrExpr,
    },
    Rect {
        center: Vec2OrExpr,
        size: Vec2OrExpr,
        #[serde(default)]
        rotation: NumberOrExpr,
    },
}

impl Shape {
    /// Evaluates the shape with the given context.
    pub fn evaluate(&self, ctx: &GameContext) -> EvaluatedShape {
        match self {
            Self::Line { start, end } => EvaluatedShape::Line {
                start: start.evaluate(ctx),
                end: end.evaluate(ctx),
            },
            Self::Circle { center, radius } => EvaluatedShape::Circle {
                center: center.evaluate(ctx),
                radius: radius.evaluate(ctx),
            },
            Self::Rect {
                center,
                size,
                rotation,
            } => EvaluatedShape::Rect {
                center: center.evaluate(ctx),
                size: size.evaluate(ctx),
                rotation: rotation.evaluate(ctx),
            },
        }
    }

    /// Returns true if any property is dynamic (uses CEL expression).
    pub fn is_dynamic(&self) -> bool {
        match self {
            Self::Line { start, end } => start.is_dynamic() || end.is_dynamic(),
            Self::Circle { center, radius } => center.is_dynamic() || radius.is_dynamic(),
            Self::Rect {
                center,
                size,
                rotation,
            } => center.is_dynamic() || size.is_dynamic() || rotation.is_dynamic(),
        }
    }
}

/// Evaluated shape with concrete f32 values.
#[derive(Debug, Clone)]
pub enum EvaluatedShape {
    Line {
        start: [f32; 2],
        end: [f32; 2],
    },
    Circle {
        center: [f32; 2],
        radius: f32,
    },
    Rect {
        center: [f32; 2],
        size: [f32; 2],
        rotation: f32,
    },
}

/// Object role in the map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectRole {
    Spawner,
    Obstacle,
    Trigger,
}

/// Spawn properties for spawner objects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpawnProperties {
    #[serde(default = "default_spawn_mode")]
    pub mode: String,
    #[serde(default = "default_initial_force")]
    pub initial_force: String,
}

fn default_spawn_mode() -> String {
    "random".to_string()
}

fn default_initial_force() -> String {
    "random".to_string()
}

/// Bumper properties for bouncy obstacles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BumperProperties {
    pub force: NumberOrExpr,
}

/// Blackhole properties for attractive forces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackholeProperties {
    pub force: NumberOrExpr,
}

/// Trigger properties for game rule triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerProperties {
    pub action: String,
}

/// Roll direction for continuous rotation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RollDirection {
    #[default]
    Clockwise,
    Counterclockwise,
}

/// Roll properties for continuous rotation animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollProperties {
    #[serde(default)]
    pub direction: RollDirection,
    /// Rotation speed in degrees per second.
    #[serde(default = "default_roll_speed")]
    pub speed: f32,
}

fn default_roll_speed() -> f32 {
    45.0
}

/// Easing type for keyframe animations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EasingType {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl EasingType {
    /// Applies the easing function to a normalized time value (0.0 to 1.0).
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// A single keyframe in an animation sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Keyframe {
    /// Marks the start of a loop block.
    LoopStart {
        /// Number of times to loop. None means infinite.
        #[serde(default)]
        count: Option<u32>,
    },
    /// Marks the end of a loop block.
    LoopEnd,
    /// Delays execution for a duration.
    Delay {
        /// Duration in seconds.
        duration: f32,
    },
    /// Applies a transformation to target objects.
    Apply {
        /// IDs of objects to animate.
        target_ids: Vec<String>,
        /// Translation offset from the initial position.
        #[serde(default)]
        translation: Option<[f32; 2]>,
        /// Rotation offset from the initial rotation (degrees).
        #[serde(default)]
        rotation: Option<f32>,
        /// Duration of the animation in seconds.
        duration: f32,
        /// Easing function to use.
        #[serde(default)]
        easing: EasingType,
    },
}

/// A sequence of keyframes forming an animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeSequence {
    /// Name of the sequence.
    pub name: String,
    /// List of keyframes.
    pub keyframes: Vec<Keyframe>,
    /// Whether to automatically start this animation.
    #[serde(default = "default_true")]
    pub autoplay: bool,
}

fn default_true() -> bool {
    true
}

/// Combined object properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawn: Option<SpawnProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bumper: Option<BumperProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blackhole: Option<BlackholeProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<TriggerProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll: Option<RollProperties>,
}

/// A map object with role, shape, and properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub role: ObjectRole,
    pub shape: Shape,
    #[serde(default)]
    pub properties: ObjectProperties,
}

/// Map metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMeta {
    pub name: String,
    #[serde(default)]
    pub gamerule: Vec<String>,
}

/// Complete roulette map configuration (V2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouletteConfig {
    pub meta: MapMeta,
    pub objects: Vec<MapObject>,
    /// Keyframe animation sequences.
    #[serde(default)]
    pub keyframes: Vec<KeyframeSequence>,
}

/// Data returned after applying map to physics world.
#[derive(Debug, Clone)]
pub struct MapWorldData {
    /// Trigger (hole) collider handles for elimination detection.
    pub trigger_handles: Vec<ColliderHandle>,
    /// Spawner data for marble spawning.
    pub spawners: Vec<SpawnerData>,
    /// Object ID to collider handle mapping.
    pub object_handles: HashMap<String, ColliderHandle>,
    /// Objects with blackhole properties for force application.
    pub blackholes: Vec<BlackholeData>,
    /// Kinematic body handles for animated objects (object_id -> body_handle).
    pub kinematic_bodies: HashMap<String, RigidBodyHandle>,
    /// Initial positions and rotations of kinematic bodies for keyframe animations.
    pub kinematic_initial_transforms: HashMap<String, ([f32; 2], f32)>,
}

/// Spawner data for marble spawning.
#[derive(Debug, Clone)]
pub struct SpawnerData {
    pub shape: Shape,
    pub properties: Option<SpawnProperties>,
}

/// Blackhole data for force application.
#[derive(Debug, Clone)]
pub struct BlackholeData {
    pub shape: Shape,
    pub force: NumberOrExpr,
}

impl RouletteConfig {
    /// Loads a map configuration from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serializes the map configuration to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Creates a default classic roulette map (V2 format).
    /// Loaded from maps/default.json at compile time.
    pub fn default_classic() -> Self {
        const DEFAULT_MAP_JSON: &str = include_str!("../maps/default.json");
        Self::from_json(DEFAULT_MAP_JSON).expect("Failed to parse default map JSON")
    }

    /// Collects all object IDs that are targeted by keyframe animations.
    fn collect_keyframe_target_ids(&self) -> std::collections::HashSet<String> {
        let mut targets = std::collections::HashSet::new();
        for seq in &self.keyframes {
            for kf in &seq.keyframes {
                if let Keyframe::Apply { target_ids, .. } = kf {
                    targets.extend(target_ids.iter().cloned());
                }
            }
        }
        targets
    }

    /// Checks if an object should be created as a kinematic body.
    fn is_animatable(&self, obj: &MapObject, keyframe_targets: &std::collections::HashSet<String>) -> bool {
        // Has roll property
        if obj.properties.roll.is_some() {
            return true;
        }
        // Is a keyframe target
        if let Some(id) = &obj.id {
            if keyframe_targets.contains(id) {
                return true;
            }
        }
        false
    }

    /// Applies the map configuration to a physics world.
    /// Returns `MapWorldData` containing handles and spawner data.
    pub fn apply_to_world(&self, world: &mut PhysicsWorld) -> MapWorldData {
        let ctx = GameContext::new(0.0, 0);
        let keyframe_targets = self.collect_keyframe_target_ids();

        let mut trigger_handles = Vec::new();
        let mut spawners = Vec::new();
        let mut object_handles = HashMap::new();
        let mut blackholes = Vec::new();
        let mut kinematic_bodies = HashMap::new();
        let mut kinematic_initial_transforms = HashMap::new();

        for obj in &self.objects {
            let shape = obj.shape.evaluate(&ctx);
            let is_animated = self.is_animatable(obj, &keyframe_targets);

            match obj.role {
                ObjectRole::Obstacle => {
                    if is_animated {
                        // Create as kinematic body
                        if let Some(id) = &obj.id {
                            let (body_handle, collider_handle, initial_pos, initial_rot) =
                                self.create_kinematic_obstacle(world, &shape, &obj.properties, &ctx);
                            kinematic_bodies.insert(id.clone(), body_handle);
                            kinematic_initial_transforms.insert(id.clone(), (initial_pos, initial_rot));
                            object_handles.insert(id.clone(), collider_handle);
                        }
                    } else {
                        let handle = self.create_obstacle_collider(world, &shape, &obj.properties, &ctx);
                        if let Some(id) = &obj.id {
                            object_handles.insert(id.clone(), handle);
                        }
                    }
                }
                ObjectRole::Trigger => {
                    let handle = self.create_trigger_collider(world, &shape);
                    trigger_handles.push(handle);
                    if let Some(id) = &obj.id {
                        object_handles.insert(id.clone(), handle);
                    }
                    // Collect blackhole if present
                    if let Some(bh) = &obj.properties.blackhole {
                        blackholes.push(BlackholeData {
                            shape: obj.shape.clone(),
                            force: bh.force.clone(),
                        });
                    }
                }
                ObjectRole::Spawner => {
                    spawners.push(SpawnerData {
                        shape: obj.shape.clone(),
                        properties: obj.properties.spawn.clone(),
                    });
                }
            }

            // Obstacles can also have blackhole property
            if obj.role == ObjectRole::Obstacle {
                if let Some(bh) = &obj.properties.blackhole {
                    blackholes.push(BlackholeData {
                        shape: obj.shape.clone(),
                        force: bh.force.clone(),
                    });
                }
            }
        }

        MapWorldData {
            trigger_handles,
            spawners,
            object_handles,
            blackholes,
            kinematic_bodies,
            kinematic_initial_transforms,
        }
    }

    fn create_obstacle_collider(
        &self,
        world: &mut PhysicsWorld,
        shape: &EvaluatedShape,
        props: &ObjectProperties,
        ctx: &GameContext,
    ) -> ColliderHandle {
        let collider = match shape {
            EvaluatedShape::Line { start, end } => {
                let mid = [
                    f32::midpoint(start[0], end[0]),
                    f32::midpoint(start[1], end[1]),
                ];
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let length = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                ColliderBuilder::cuboid(length / 2.0, 2.0)
                    .translation(Vector::new(mid[0], mid[1]))
                    .rotation(angle)
                    .friction(0.3)
                    .restitution(0.5)
                    .build()
            }
            EvaluatedShape::Circle { center, radius } => {
                let mut builder = ColliderBuilder::ball(*radius)
                    .translation(Vector::new(center[0], center[1]))
                    .friction(0.3);

                // Apply bumper restitution if present
                if let Some(bumper) = &props.bumper {
                    let force = bumper.force.evaluate(ctx);
                    builder = builder.restitution(0.6 + force * 0.4);
                } else {
                    builder = builder.restitution(0.6);
                }

                builder.build()
            }
            EvaluatedShape::Rect {
                center,
                size,
                rotation,
            } => {
                let rotation_rad = rotation.to_radians();

                ColliderBuilder::cuboid(size[0] / 2.0, size[1] / 2.0)
                    .translation(Vector::new(center[0], center[1]))
                    .rotation(rotation_rad)
                    .friction(0.3)
                    .restitution(0.6)
                    .build()
            }
        };

        world.add_static_collider(collider)
    }

    /// Creates a kinematic obstacle (for animated objects).
    /// Returns (body_handle, collider_handle, initial_position, initial_rotation_radians).
    fn create_kinematic_obstacle(
        &self,
        world: &mut PhysicsWorld,
        shape: &EvaluatedShape,
        props: &ObjectProperties,
        ctx: &GameContext,
    ) -> (RigidBodyHandle, ColliderHandle, [f32; 2], f32) {
        let (position, rotation_rad, collider) = match shape {
            EvaluatedShape::Line { start, end } => {
                let mid = [
                    f32::midpoint(start[0], end[0]),
                    f32::midpoint(start[1], end[1]),
                ];
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let length = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                let collider = ColliderBuilder::cuboid(length / 2.0, 2.0)
                    .friction(0.3)
                    .restitution(0.5)
                    .build();
                (mid, angle, collider)
            }
            EvaluatedShape::Circle { center, radius } => {
                let mut builder = ColliderBuilder::ball(*radius).friction(0.3);

                if let Some(bumper) = &props.bumper {
                    let force = bumper.force.evaluate(ctx);
                    builder = builder.restitution(0.6 + force * 0.4);
                } else {
                    builder = builder.restitution(0.6);
                }

                (*center, 0.0, builder.build())
            }
            EvaluatedShape::Rect {
                center,
                size,
                rotation,
            } => {
                let rotation_rad = rotation.to_radians();
                let collider = ColliderBuilder::cuboid(size[0] / 2.0, size[1] / 2.0)
                    .friction(0.3)
                    .restitution(0.6)
                    .build();
                (*center, rotation_rad, collider)
            }
        };

        // Create kinematic body at the position
        let body_handle = world.add_kinematic_body(
            Vector::new(position[0], position[1]),
            rotation_rad,
        );
        let collider_handle = world.add_kinematic_collider(collider, body_handle);

        (body_handle, collider_handle, position, rotation_rad)
    }

    fn create_trigger_collider(&self, world: &mut PhysicsWorld, shape: &EvaluatedShape) -> ColliderHandle {
        let collider = match shape {
            EvaluatedShape::Circle { center, radius } => ColliderBuilder::ball(*radius)
                .translation(Vector::new(center[0], center[1]))
                .sensor(true)
                .build(),
            EvaluatedShape::Rect {
                center,
                size,
                rotation,
            } => {
                let rotation_rad = rotation.to_radians();
                ColliderBuilder::cuboid(size[0] / 2.0, size[1] / 2.0)
                    .translation(Vector::new(center[0], center[1]))
                    .rotation(rotation_rad)
                    .sensor(true)
                    .build()
            }
            EvaluatedShape::Line { .. } => {
                panic!("Line shape not supported for triggers");
            }
        };

        world.add_static_collider(collider)
    }

    /// Finds trigger handles in an existing physics world by locating sensor colliders.
    /// Used after restoring from a snapshot.
    pub fn find_trigger_handles(&self, world: &PhysicsWorld) -> Vec<ColliderHandle> {
        let ctx = GameContext::new(0.0, 0);
        let mut trigger_handles = Vec::new();

        for obj in &self.objects {
            if obj.role != ObjectRole::Trigger {
                continue;
            }

            let shape = obj.shape.evaluate(&ctx);
            let (target_x, target_y) = match shape {
                EvaluatedShape::Circle { center, .. } => (center[0], center[1]),
                EvaluatedShape::Rect { center, .. } => (center[0], center[1]),
                EvaluatedShape::Line { .. } => continue,
            };

            // Find collider at this position
            for (handle, collider) in world.collider_set.iter() {
                if collider.is_sensor() {
                    let pos = collider.translation();
                    let dx = pos.x - target_x;
                    let dy = pos.y - target_y;
                    let dist_sq = dx * dx + dy * dy;

                    if dist_sq < 1.0 {
                        trigger_handles.push(handle);
                        break;
                    }
                }
            }
        }

        trigger_handles
    }

    /// Gets all spawner data from the map.
    pub fn get_spawners(&self) -> Vec<SpawnerData> {
        self.objects
            .iter()
            .filter(|obj| obj.role == ObjectRole::Spawner)
            .map(|obj| SpawnerData {
                shape: obj.shape.clone(),
                properties: obj.properties.spawn.clone(),
            })
            .collect()
    }

    /// Gets all blackhole data from the map.
    pub fn get_blackholes(&self) -> Vec<BlackholeData> {
        self.objects
            .iter()
            .filter_map(|obj| {
                obj.properties.blackhole.as_ref().map(|bh| BlackholeData {
                    shape: obj.shape.clone(),
                    force: bh.force.clone(),
                })
            })
            .collect()
    }

    /// Finds kinematic body handles in an existing physics world.
    /// Used after restoring from a snapshot.
    /// Returns (kinematic_bodies, kinematic_initial_transforms).
    pub fn find_kinematic_handles(
        &self,
        world: &PhysicsWorld,
    ) -> (HashMap<String, RigidBodyHandle>, HashMap<String, ([f32; 2], f32)>) {
        let ctx = GameContext::new(0.0, 0);
        let keyframe_targets = self.collect_keyframe_target_ids();

        let mut kinematic_bodies = HashMap::new();
        let mut kinematic_initial_transforms = HashMap::new();

        for obj in &self.objects {
            if obj.role != ObjectRole::Obstacle {
                continue;
            }

            if !self.is_animatable(obj, &keyframe_targets) {
                continue;
            }

            let id = match &obj.id {
                Some(id) => id,
                None => continue,
            };

            let shape = obj.shape.evaluate(&ctx);
            let (target_x, target_y, initial_rot) = match shape {
                EvaluatedShape::Circle { center, .. } => (center[0], center[1], 0.0),
                EvaluatedShape::Rect { center, rotation, .. } => {
                    (center[0], center[1], rotation.to_radians())
                }
                EvaluatedShape::Line { start, end } => {
                    let mid_x = f32::midpoint(start[0], end[0]);
                    let mid_y = f32::midpoint(start[1], end[1]);
                    let dx = end[0] - start[0];
                    let dy = end[1] - start[1];
                    let angle = dy.atan2(dx);
                    (mid_x, mid_y, angle)
                }
            };

            // Find kinematic body at this position
            for (handle, body) in world.rigid_body_set.iter() {
                if !body.is_kinematic() {
                    continue;
                }

                let pos = body.translation();
                let dx = pos.x - target_x;
                let dy = pos.y - target_y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq < 1.0 {
                    kinematic_bodies.insert(id.clone(), handle);
                    kinematic_initial_transforms.insert(id.clone(), ([target_x, target_y], initial_rot));
                    break;
                }
            }
        }

        (kinematic_bodies, kinematic_initial_transforms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_classic_map() {
        let config = RouletteConfig::default_classic();
        // Map name comes from maps/default.json
        assert_eq!(config.meta.name, "Animated Example");

        // Count by role
        let spawners: Vec<_> = config
            .objects
            .iter()
            .filter(|o| o.role == ObjectRole::Spawner)
            .collect();
        let obstacles: Vec<_> = config
            .objects
            .iter()
            .filter(|o| o.role == ObjectRole::Obstacle)
            .collect();
        let triggers: Vec<_> = config
            .objects
            .iter()
            .filter(|o| o.role == ObjectRole::Trigger)
            .collect();

        assert_eq!(spawners.len(), 1);
        // 4 walls + center bumper + 2 rotating boxes + 2 moving walls = 9
        assert_eq!(obstacles.len(), 9);
        assert_eq!(triggers.len(), 1);

        // Verify keyframes are loaded
        assert!(!config.keyframes.is_empty());
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        let config = RouletteConfig::default_classic();
        let json = config.to_json().expect("Failed to serialize");
        let loaded = RouletteConfig::from_json(&json).expect("Failed to deserialize");

        assert_eq!(loaded.meta.name, config.meta.name);
        assert_eq!(loaded.objects.len(), config.objects.len());
    }

    #[test]
    fn test_apply_to_world() {
        let config = RouletteConfig::default_classic();
        let mut world = PhysicsWorld::new();

        let map_data = config.apply_to_world(&mut world);

        // Should have one trigger (hole)
        assert_eq!(map_data.trigger_handles.len(), 1);
        // Should have one spawner
        assert_eq!(map_data.spawners.len(), 1);
        // Should have one blackhole
        assert_eq!(map_data.blackholes.len(), 1);

        // Verify colliders were created
        // 9 obstacles + 1 trigger = 10 colliders
        assert_eq!(world.collider_set.len(), 10);
    }

    #[test]
    fn test_v2_json_parsing() {
        let json = r#"{
            "meta": {
                "name": "Test V2",
                "gamerule": ["top_n"]
            },
            "objects": [
                {
                    "role": "spawner",
                    "shape": { "type": "rect", "center": [400, 100], "size": [600, 100], "rotation": 0 },
                    "properties": { "spawn": { "mode": "random", "initial_force": "random" } }
                },
                {
                    "role": "obstacle",
                    "shape": { "type": "line", "start": [0, 0], "end": [800, 0] }
                },
                {
                    "role": "obstacle",
                    "shape": { "type": "circle", "center": [400, 300], "radius": 30 },
                    "properties": { "bumper": { "force": 1.0 } }
                },
                {
                    "id": "goal",
                    "role": "trigger",
                    "shape": { "type": "circle", "center": [400, 550], "radius": 40 },
                    "properties": {
                        "blackhole": { "force": 0.2 },
                        "trigger": { "action": "gamerule" }
                    }
                }
            ],
            "keyframes": []
        }"#;

        let config = RouletteConfig::from_json(json).expect("Failed to parse JSON");

        assert_eq!(config.meta.name, "Test V2");
        assert_eq!(config.objects.len(), 4);
        assert_eq!(config.meta.gamerule, vec!["top_n"]);
    }

    #[test]
    fn test_cel_expression_parsing() {
        let json = r#"{
            "meta": { "name": "CEL Test", "gamerule": [] },
            "objects": [
                {
                    "id": "goal",
                    "role": "trigger",
                    "shape": { "type": "circle", "center": [400, 550], "radius": 40 },
                    "properties": {
                        "blackhole": { "force": "0.2 + 0.1 * game.time" },
                        "trigger": { "action": "gamerule" }
                    }
                }
            ],
            "keyframes": []
        }"#;

        let config = RouletteConfig::from_json(json).expect("Failed to parse JSON");

        // Verify CEL expression was parsed
        let trigger = &config.objects[0];
        let blackhole = trigger.properties.blackhole.as_ref().unwrap();
        assert!(blackhole.force.is_dynamic());

        // Evaluate at different times
        let ctx0 = GameContext::new(0.0, 0);
        let ctx10 = GameContext::new(10.0, 600);

        let force0 = blackhole.force.evaluate(&ctx0);
        let force10 = blackhole.force.evaluate(&ctx10);

        assert!((force0 - 0.2).abs() < 0.001);
        assert!((force10 - 1.2).abs() < 0.001);
    }
}
