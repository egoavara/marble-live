//! Marble-related systems.
//!
//! Handles marble spawning, tracking, and visual updates.

use bevy::prelude::*;
use rand::Rng;
use rapier2d::prelude::*;
use tracing::warn;

use crate::bevy::plugin::EditorState;
use crate::bevy::rapier_plugin::{
    PhysicsBody, PhysicsExternalForce, PhysicsWorldRes, USER_DATA_MARBLE, encode_user_data,
};
use crate::bevy::{
    ClearMarblesEvent, DeterministicRng, GameContextRes, MapConfig, Marble, MarbleGameState,
    MarbleVisual, SpawnMarblesAtEvent, SpawnMarblesEvent,
};
use crate::map::{EvaluatedShape, ObjectRole};
use crate::marble::DEFAULT_MARBLE_RADIUS;

/// System to handle marble spawning requests.
///
/// Note: Marble clearing is handled separately via ClearMarblesEvent.
/// Use the `yield` command to ensure frame separation between clear and spawn.
/// In editor mode, marbles can only be spawned during simulation.
pub fn handle_spawn_marbles(
    mut commands: Commands,
    mut events: MessageReader<SpawnMarblesEvent>,
    game_state: Res<MarbleGameState>,
    map_config: Option<Res<MapConfig>>,
    mut rng: ResMut<DeterministicRng>,
    game_context: Res<GameContextRes>,
    editor_state: Option<Res<State<EditorState>>>,
    mut physics: ResMut<PhysicsWorldRes>,
) {
    for _ in events.read() {
        // In editor mode, only allow spawning during simulation
        if let Some(state) = &editor_state {
            if **state != EditorState::Simulating {
                warn!("Cannot spawn marbles outside of simulation mode");
                continue;
            }
        }

        if game_state.players.is_empty() {
            warn!("No players registered, cannot spawn marbles. Call add_player first.");
            continue;
        }

        tracing::info!("Spawning marbles for {} players", game_state.players.len());

        // Get spawner from MapConfig (single source of truth for shape)
        let Some(config) = &map_config else {
            warn!("No map config found");
            continue;
        };

        let Some(spawner_obj) = config
            .0
            .objects
            .iter()
            .find(|o| o.role == ObjectRole::Spawner)
        else {
            warn!("No spawner found in map config");
            continue;
        };

        // Spawn a marble for each player
        for player in &game_state.players {
            let shape = spawner_obj.shape.evaluate(&game_context.context);
            let (x, y) = random_position_in_shape(&shape, &mut rng.rng);

            tracing::info!(
                "Spawning marble for player {} at ({:.2}, {:.2})",
                player.id,
                x,
                y
            );

            let entity = spawn_marble_at(
                &mut commands,
                &mut physics,
                player.id,
                player.color,
                Vec2::new(x, y),
                DEFAULT_MARBLE_RADIUS,
            );
            tracing::info!("Created marble entity {:?}", entity);
        }
    }
}

/// System to handle marble clearing requests.
pub fn handle_clear_marbles(
    mut commands: Commands,
    mut events: MessageReader<ClearMarblesEvent>,
    marbles: Query<(Entity, Option<&PhysicsBody>), With<Marble>>,
    mut game_state: ResMut<MarbleGameState>,
    mut physics: ResMut<PhysicsWorldRes>,
) {
    for _ in events.read() {
        for (entity, body) in marbles.iter() {
            // Remove from physics world
            if let Some(body) = body {
                physics.world.remove_rigid_body(body.0);
            }
            commands.entity(entity).despawn();
        }
        game_state.arrival_order.clear();
    }
}

/// System to handle marble spawning at specific positions (peer: host-provided coordinates).
pub fn handle_spawn_marbles_at(
    mut commands: Commands,
    mut events: MessageReader<SpawnMarblesAtEvent>,
    game_state: Res<MarbleGameState>,
    mut physics: ResMut<PhysicsWorldRes>,
) {
    for event in events.read() {
        if game_state.players.is_empty() {
            warn!("No players registered, cannot spawn marbles at positions.");
            continue;
        }

        tracing::info!(
            "SpawnMarblesAt: {} positions for {} players",
            event.positions.len(),
            game_state.players.len()
        );

        for (i, player) in game_state.players.iter().enumerate() {
            let pos = event.positions.get(i).copied().unwrap_or([0.0, 0.0]);
            spawn_marble_at(
                &mut commands,
                &mut physics,
                player.id,
                player.color,
                Vec2::new(pos[0], pos[1]),
                DEFAULT_MARBLE_RADIUS,
            );
            tracing::info!(
                "Spawned marble for player {} at ({:.2}, {:.2}) from host",
                player.id,
                pos[0],
                pos[1]
            );
        }
    }
}

/// Spawns a marble at the given position.
///
/// Creates a Rapier dynamic body + collider in PhysicsWorldRes,
/// then spawns a Bevy entity with PhysicsBody + PhysicsExternalForce components.
fn spawn_marble_at(
    commands: &mut Commands,
    physics: &mut ResMut<PhysicsWorldRes>,
    owner_id: u32,
    color: crate::marble::Color,
    position: Vec2,
    radius: f32,
) -> Entity {
    // First spawn the entity to get its ID
    let entity = commands
        .spawn((
            Marble::new(owner_id),
            MarbleVisual { color, radius },
            Transform::from_translation(position.extend(0.0)),
            PhysicsExternalForce::default(),
        ))
        .id();

    // Create rapier rigid body with entity bits as user_data
    let body = RigidBodyBuilder::dynamic()
        .translation(Vector::new(position.x, position.y))
        .ccd_enabled(true)
        .user_data(entity.to_bits() as u128)
        .build();
    let body_handle = physics.world.add_rigid_body(body);

    // Create rapier collider
    let collider = ColliderBuilder::ball(radius)
        .restitution(0.7)
        .friction(0.3)
        .density(1.0)
        .active_events(ActiveEvents::COLLISION_EVENTS)
        .user_data(encode_user_data(USER_DATA_MARBLE, owner_id as u64))
        .build();
    physics.world.add_collider(collider, body_handle);

    // Set damping on the body
    if let Some(body) = physics.world.get_rigid_body_mut(body_handle) {
        body.set_linear_damping(0.5);
        body.set_angular_damping(0.5);
    }

    // Insert the PhysicsBody component
    commands.entity(entity).insert(PhysicsBody(body_handle));

    entity
}

/// Returns a random position within the given shape.
fn random_position_in_shape(shape: &EvaluatedShape, rng: &mut impl Rng) -> (f32, f32) {
    match shape {
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let local_x = rng.random_range(-size[0] / 2.0..size[0] / 2.0);
            let local_y = rng.random_range(-size[1] / 2.0..size[1] / 2.0);
            let rad = rotation.to_radians();
            let cos_r = rad.cos();
            let sin_r = rad.sin();
            (
                center[0] + local_x * cos_r - local_y * sin_r,
                center[1] + local_x * sin_r + local_y * cos_r,
            )
        }
        EvaluatedShape::Circle { center, radius } => {
            let angle = rng.random_range(0.0..std::f32::consts::TAU);
            let r = rng.random_range(0.0..*radius);
            (center[0] + r * angle.cos(), center[1] + r * angle.sin())
        }
        EvaluatedShape::Line { start, end } => {
            let t = rng.random_range(0.0..1.0);
            (
                start[0] + (end[0] - start[0]) * t,
                start[1] + (end[1] - start[1]) * t,
            )
        }
        EvaluatedShape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            let t: f32 = rng.random_range(0.0..1.0);
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;
            (
                mt3 * start[0]
                    + 3.0 * mt2 * t * control1[0]
                    + 3.0 * mt * t2 * control2[0]
                    + t3 * end[0],
                mt3 * start[1]
                    + 3.0 * mt2 * t * control1[1]
                    + 3.0 * mt * t2 * control2[1]
                    + t3 * end[1],
            )
        }
    }
}

/// System to get marble positions for live ranking.
#[allow(dead_code)]
pub fn calculate_live_ranking(
    marbles: Query<(&Marble, &Transform), Without<crate::bevy::TriggerZone>>,
    _game_state: Res<MarbleGameState>,
    map_config: Option<Res<MapConfig>>,
) -> Vec<(u32, f32)> {
    let mut rankings: Vec<(u32, f32)> = marbles
        .iter()
        .filter(|(marble, _)| !marble.eliminated)
        .map(|(marble, transform)| {
            let pos = transform.translation.truncate();
            let score = calculate_ranking_score(pos, map_config.as_deref());
            (marble.owner_id, score)
        })
        .collect();

    // Sort by score (lower is better)
    rankings.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    rankings
}

/// Calculates ranking score based on live_ranking configuration.
fn calculate_ranking_score(pos: Vec2, config: Option<&MapConfig>) -> f32 {
    match config {
        Some(MapConfig(config)) => {
            use crate::map::LiveRankingConfig;
            match &config.meta.live_ranking {
                LiveRankingConfig::YPosition => pos.y,
                LiveRankingConfig::Distance { target_id } => {
                    // Find target object center
                    if let Some(obj) = config
                        .objects
                        .iter()
                        .find(|o| o.id.as_deref() == Some(target_id))
                    {
                        let ctx = crate::dsl::GameContext::new(0.0, 0);
                        let shape = obj.shape.evaluate(&ctx);
                        let target_center = match shape {
                            EvaluatedShape::Circle { center, .. } => {
                                Vec2::new(center[0], center[1])
                            }
                            EvaluatedShape::Rect { center, .. } => Vec2::new(center[0], center[1]),
                            _ => return pos.y,
                        };
                        pos.distance(target_center)
                    } else {
                        pos.y
                    }
                }
            }
        }
        None => pos.y,
    }
}

#[cfg(test)]
mod tests {
    use crate::bevy::test_utils::TestApp;
    use crate::bevy::Marble;
    use crate::marble::Color;
    use crate::map::*;

    fn spawner_map() -> RouletteConfig {
        RouletteConfig {
            meta: MapMeta {
                name: "spawn_test".to_string(),
                gamerule: vec![],
                live_ranking: LiveRankingConfig::default(),
            },
            objects: vec![
                MapObject {
                    id: Some("spawner".to_string()),
                    role: ObjectRole::Spawner,
                    shape: Shape::Rect {
                        center: crate::dsl::Vec2OrExpr::Static([0.0, 5.0]),
                        size: crate::dsl::Vec2OrExpr::Static([3.0, 0.5]),
                        rotation: crate::dsl::NumberOrExpr::Number(0.0),
                    },
                    properties: ObjectProperties {
                        spawn: Some(SpawnProperties::default()),
                        ..Default::default()
                    },
                },
                // Floor obstacle
                MapObject {
                    id: Some("floor".to_string()),
                    role: ObjectRole::Obstacle,
                    shape: Shape::Rect {
                        center: crate::dsl::Vec2OrExpr::Static([0.0, -10.0]),
                        size: crate::dsl::Vec2OrExpr::Static([20.0, 0.5]),
                        rotation: crate::dsl::NumberOrExpr::Number(0.0),
                    },
                    properties: ObjectProperties::default(),
                },
            ],
            keyframes: vec![],
        }
    }

    #[test]
    fn test_spawn_marbles_creates_entities() {
        let mut app = TestApp::new();
        app.enter_game_mode();
        app.load_map(spawner_map());

        app.add_player("Alice", Color::new(255, 0, 0, 255));
        app.add_player("Bob", Color::new(0, 0, 255, 255));
        app.spawn_marbles();

        let mut query = app
            .world_mut()
            .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Marble>>();
        let marbles: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(marbles.len(), 2, "Expected one marble per player");
    }

    #[test]
    fn test_marble_falls_under_gravity() {
        let mut app = TestApp::new();
        app.enter_game_mode();
        app.load_map(spawner_map());

        app.add_player("Alice", Color::new(255, 0, 0, 255));
        app.spawn_marbles();

        // Record initial Y position
        let initial_y = {
            let mut query = app
                .world_mut()
                .query::<(&Marble, &bevy::prelude::Transform)>();
            let (_marble, transform) = query
                .iter(app.world())
                .next()
                .expect("Expected a marble");
            transform.translation.y
        };

        // Advance physics for ~1 second (60 steps at 60Hz)
        app.step_physics(60);

        // Check that marble has fallen
        let final_y = {
            let mut query = app
                .world_mut()
                .query::<(&Marble, &bevy::prelude::Transform)>();
            let (_marble, transform) = query
                .iter(app.world())
                .next()
                .expect("Marble should still exist");
            transform.translation.y
        };

        assert!(
            final_y < initial_y,
            "Marble should have fallen: initial_y={initial_y}, final_y={final_y}"
        );
    }

    #[test]
    fn test_clear_marbles_removes_entities() {
        let mut app = TestApp::new();
        app.enter_game_mode();
        app.load_map(spawner_map());

        app.add_player("Alice", Color::new(255, 0, 0, 255));
        app.spawn_marbles();

        // Verify marble exists
        let mut query = app
            .world_mut()
            .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Marble>>();
        let count = query.iter(app.world()).count();
        assert_eq!(count, 1);

        // Clear marbles
        app.push_command(crate::bevy::GameCommand::ClearMarbles);
        app.update();

        // Verify marble is gone
        let mut query = app
            .world_mut()
            .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Marble>>();
        let count = query.iter(app.world()).count();
        assert_eq!(count, 0, "Marbles should be cleared");
    }
}
