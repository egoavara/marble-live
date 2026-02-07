//! Marble-related systems.
//!
//! Handles marble spawning, tracking, and visual updates.

use bevy::prelude::*;
use tracing::warn;
use bevy_rapier2d::prelude::*;
use rand::Rng;

use crate::bevy::{
    ClearMarblesEvent, DeterministicRng, GameContextRes, MapConfig, Marble, MarbleGameState,
    MarbleVisual, SpawnMarblesAtEvent, SpawnMarblesEvent,
};
use crate::bevy::plugin::EditorState;
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

        let Some(spawner_obj) = config.0.objects.iter().find(|o| o.role == ObjectRole::Spawner) else {
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
    marbles: Query<Entity, With<Marble>>,
    mut game_state: ResMut<MarbleGameState>,
) {
    for _ in events.read() {
        for entity in marbles.iter() {
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
fn spawn_marble_at(
    commands: &mut Commands,
    owner_id: u32,
    color: crate::marble::Color,
    position: Vec2,
    radius: f32,
) -> Entity {
    commands
        .spawn((
            Marble::new(owner_id),
            MarbleVisual { color, radius },
            Transform::from_translation(position.extend(0.0)),
            RigidBody::Dynamic,
            Collider::ball(radius),
            Restitution::coefficient(0.7),
            Friction::coefficient(0.3),
            Damping {
                linear_damping: 0.5,
                angular_damping: 0.5,
            },
            Velocity::default(),
            ExternalForce::default(),
            Ccd::enabled(),
            ActiveEvents::COLLISION_EVENTS,
        ))
        .id()
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
