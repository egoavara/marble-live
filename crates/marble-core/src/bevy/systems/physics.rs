//! Physics-related systems.
//!
//! Handles blackhole forces and physics configuration.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::bevy::{BlackholeZone, GameContextRes, Marble};
use crate::map::EvaluatedShape;
use crate::physics::PHYSICS_DT;

/// System to apply blackhole forces to all active marbles.
pub fn apply_blackhole_forces(
    blackholes: Query<&BlackholeZone>,
    mut marbles: Query<(&Marble, &mut ExternalForce, &Transform), Without<BlackholeZone>>,
    game_context: Res<GameContextRes>,
) {
    for blackhole in blackholes.iter() {
        let force_magnitude = blackhole.force.evaluate(&game_context.context);
        if force_magnitude.abs() < f32::EPSILON {
            continue;
        }

        // Get blackhole center
        let shape = blackhole.shape.evaluate(&game_context.context);
        let center = match shape {
            EvaluatedShape::Circle { center, .. } => Vec2::new(center[0], center[1]),
            EvaluatedShape::Rect { center, .. } => Vec2::new(center[0], center[1]),
            _ => continue,
        };

        // Apply force to all active marbles
        for (marble, mut ext_force, transform) in marbles.iter_mut() {
            if marble.eliminated {
                continue;
            }

            let marble_pos = transform.translation.truncate();
            let direction = center - marble_pos;
            let dist = direction.length().max(0.01);

            // Force magnitude inversely proportional to distance
            let calculated_force = direction.normalize() * force_magnitude * 10.0 / dist;
            ext_force.force += calculated_force;
        }
    }
}

/// System to update the game context each frame.
pub fn update_game_context(
    mut game_context: ResMut<GameContextRes>,
    mut game_state: ResMut<crate::bevy::MarbleGameState>,
) {
    game_state.frame += 1;
    let time_secs = game_state.frame as f32 * PHYSICS_DT;
    game_context.update(time_secs, game_state.frame);
}

/// System to clear external forces at the start of each physics step.
pub fn clear_external_forces(mut forces: Query<&mut ExternalForce>) {
    for mut force in forces.iter_mut() {
        force.force = Vec2::ZERO;
        force.torque = 0.0;
    }
}
