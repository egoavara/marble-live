//! Roulette map configuration and terrain generation.

use rapier2d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::physics::PhysicsWorld;

/// Map metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMeta {
    pub name: String,
    pub width: f32,
    pub height: f32,
}

/// Line segment wall definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineWall {
    pub start: [f32; 2],
    pub end: [f32; 2],
}

/// Wall type in the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Wall {
    Line(LineWall),
}

/// Circle obstacle definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleObstacle {
    pub center: [f32; 2],
    pub radius: f32,
}

/// Rectangle obstacle definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectObstacle {
    pub center: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
}

/// Obstacle type in the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Obstacle {
    Circle(CircleObstacle),
    Rect(RectObstacle),
}

/// Hole (elimination zone) definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hole {
    pub center: [f32; 2],
    pub radius: f32,
}

/// Spawn area bounds for marbles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnArea {
    pub x: [f32; 2],
    pub y: [f32; 2],
}

/// Complete roulette map configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouletteConfig {
    pub meta: MapMeta,
    pub walls: Vec<Wall>,
    pub obstacles: Vec<Obstacle>,
    pub holes: Vec<Hole>,
    pub spawn_area: SpawnArea,
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

    /// Creates a default classic roulette map.
    pub fn default_classic() -> Self {
        Self {
            meta: MapMeta {
                name: "Classic".to_string(),
                width: 800.0,
                height: 600.0,
            },
            walls: vec![
                // Boundary walls
                Wall::Line(LineWall {
                    start: [0.0, 0.0],
                    end: [800.0, 0.0],
                }),
                Wall::Line(LineWall {
                    start: [800.0, 0.0],
                    end: [800.0, 600.0],
                }),
                Wall::Line(LineWall {
                    start: [800.0, 600.0],
                    end: [0.0, 600.0],
                }),
                Wall::Line(LineWall {
                    start: [0.0, 600.0],
                    end: [0.0, 0.0],
                }),
            ],
            obstacles: vec![
                // Center obstacle
                Obstacle::Circle(CircleObstacle {
                    center: [400.0, 300.0],
                    radius: 30.0,
                }),
                // Corner obstacles
                Obstacle::Rect(RectObstacle {
                    center: [200.0, 200.0],
                    size: [60.0, 10.0],
                    rotation: 30.0,
                }),
                Obstacle::Rect(RectObstacle {
                    center: [600.0, 200.0],
                    size: [60.0, 10.0],
                    rotation: -30.0,
                }),
                Obstacle::Rect(RectObstacle {
                    center: [200.0, 400.0],
                    size: [60.0, 10.0],
                    rotation: -30.0,
                }),
                Obstacle::Rect(RectObstacle {
                    center: [600.0, 400.0],
                    size: [60.0, 10.0],
                    rotation: 30.0,
                }),
            ],
            holes: vec![Hole {
                center: [400.0, 550.0],
                radius: 60.0,
            }],
            spawn_area: SpawnArea {
                x: [100.0, 700.0],
                y: [50.0, 150.0],
            },
        }
    }

    /// Applies the map configuration to a physics world by creating colliders.
    /// Returns handles to the created hole colliders for collision detection.
    pub fn apply_to_world(&self, world: &mut PhysicsWorld) -> Vec<ColliderHandle> {
        // Create wall colliders
        for wall in &self.walls {
            match wall {
                Wall::Line(line) => {
                    let start = Vector::new(line.start[0], line.start[1]);
                    let end = Vector::new(line.end[0], line.end[1]);

                    // Calculate segment properties
                    let mid = Vector::new(
                        f32::midpoint(start.x, end.x),
                        f32::midpoint(start.y, end.y),
                    );
                    let diff = end - start;
                    let length = diff.length();
                    let angle = diff.y.atan2(diff.x);

                    // Create a thin cuboid as wall
                    let collider = ColliderBuilder::cuboid(length / 2.0, 2.0)
                        .translation(Vector::new(mid.x, mid.y))
                        .rotation(angle)
                        .friction(0.3)
                        .restitution(0.5)
                        .build();

                    world.add_static_collider(collider);
                }
            }
        }

        // Create obstacle colliders
        for obstacle in &self.obstacles {
            match obstacle {
                Obstacle::Circle(circle) => {
                    let collider = ColliderBuilder::ball(circle.radius)
                        .translation(Vector::new(circle.center[0], circle.center[1]))
                        .friction(0.3)
                        .restitution(0.6)
                        .build();

                    world.add_static_collider(collider);
                }
                Obstacle::Rect(rect) => {
                    let half_width = rect.size[0] / 2.0;
                    let half_height = rect.size[1] / 2.0;
                    let rotation_rad = rect.rotation.to_radians();

                    let collider = ColliderBuilder::cuboid(half_width, half_height)
                        .translation(Vector::new(rect.center[0], rect.center[1]))
                        .rotation(rotation_rad)
                        .friction(0.3)
                        .restitution(0.6)
                        .build();

                    world.add_static_collider(collider);
                }
            }
        }

        // Create hole colliders (sensors for elimination detection)
        let mut hole_handles = Vec::new();
        for hole in &self.holes {
            let collider = ColliderBuilder::ball(hole.radius)
                .translation(Vector::new(hole.center[0], hole.center[1]))
                .sensor(true)
                .build();

            let handle = world.add_static_collider(collider);
            hole_handles.push(handle);
        }

        hole_handles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_classic_map() {
        let config = RouletteConfig::default_classic();
        assert_eq!(config.meta.name, "Classic");
        assert_eq!(config.meta.width, 800.0);
        assert_eq!(config.meta.height, 600.0);
        assert_eq!(config.walls.len(), 4);
        assert_eq!(config.obstacles.len(), 5);
        assert_eq!(config.holes.len(), 1);
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        let config = RouletteConfig::default_classic();
        let json = config.to_json().expect("Failed to serialize");
        let loaded = RouletteConfig::from_json(&json).expect("Failed to deserialize");

        assert_eq!(loaded.meta.name, config.meta.name);
        assert_eq!(loaded.walls.len(), config.walls.len());
        assert_eq!(loaded.obstacles.len(), config.obstacles.len());
        assert_eq!(loaded.holes.len(), config.holes.len());
    }

    #[test]
    fn test_apply_to_world() {
        let config = RouletteConfig::default_classic();
        let mut world = PhysicsWorld::new();

        let hole_handles = config.apply_to_world(&mut world);

        // Should have one hole handle
        assert_eq!(hole_handles.len(), 1);

        // Verify colliders were created
        // 4 walls + 5 obstacles + 1 hole = 10 colliders
        assert_eq!(world.collider_set.len(), 10);
    }

    #[test]
    fn test_json_parsing() {
        let json = r#"{
            "meta": {
                "name": "Test",
                "width": 400,
                "height": 300
            },
            "walls": [
                { "type": "line", "start": [0, 0], "end": [400, 0] }
            ],
            "obstacles": [
                { "type": "circle", "center": [200, 150], "radius": 20 },
                { "type": "rect", "center": [100, 100], "size": [30, 10], "rotation": 45 }
            ],
            "holes": [
                { "center": [200, 280], "radius": 25 }
            ],
            "spawn_area": {
                "x": [50, 350],
                "y": [30, 80]
            }
        }"#;

        let config = RouletteConfig::from_json(json).expect("Failed to parse JSON");

        assert_eq!(config.meta.name, "Test");
        assert_eq!(config.walls.len(), 1);
        assert_eq!(config.obstacles.len(), 2);
        assert_eq!(config.holes.len(), 1);
        assert_eq!(config.spawn_area.x, [50.0, 350.0]);
    }
}
