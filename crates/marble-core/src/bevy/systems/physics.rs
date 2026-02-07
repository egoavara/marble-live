//! Physics-related systems.
//!
//! Handles vector field forces and physics configuration.

use bevy::prelude::*;

use crate::bevy::rapier_plugin::PhysicsExternalForce;
use crate::bevy::{GameContextRes, Marble, VectorFieldZone};
use crate::map::{EvaluatedShape, VectorFieldFalloff};
use crate::physics::PHYSICS_DT;

/// System to apply vector field forces to all active marbles within field areas.
pub fn apply_vector_field_forces(
    vector_fields: Query<&VectorFieldZone>,
    mut marbles: Query<(&Marble, &mut PhysicsExternalForce, &Transform), Without<VectorFieldZone>>,
    game_context: Res<GameContextRes>,
) {
    for field in vector_fields.iter() {
        // Check if field is enabled
        if !field.enabled.evaluate(&game_context.context) {
            continue;
        }

        // Evaluate direction and magnitude
        let dir = field.direction.evaluate(&game_context.context);
        let dir_vec = Vec2::new(dir[0], dir[1]).normalize_or_zero();
        if dir_vec.length_squared() < f32::EPSILON {
            continue;
        }

        let magnitude = field.magnitude.evaluate(&game_context.context);
        if magnitude.abs() < f32::EPSILON {
            continue;
        }

        // Get field shape for area detection
        let shape = field.shape.evaluate(&game_context.context);
        let center = get_shape_center(&shape);

        // Apply force to marbles inside the field
        for (marble, mut ext_force, transform) in marbles.iter_mut() {
            if marble.eliminated {
                continue;
            }

            let marble_pos = transform.translation.truncate();
            if !is_point_in_shape(&marble_pos, &shape) {
                continue;
            }

            let force = match field.falloff {
                VectorFieldFalloff::Uniform => dir_vec * magnitude,
                VectorFieldFalloff::DistanceBased => {
                    let dist = (marble_pos - center).length().max(0.01);
                    dir_vec * magnitude * 10.0 / dist
                }
            };
            ext_force.force += force;
        }
    }
}

/// Gets the center point of a shape.
fn get_shape_center(shape: &EvaluatedShape) -> Vec2 {
    match shape {
        EvaluatedShape::Circle { center, .. } => Vec2::new(center[0], center[1]),
        EvaluatedShape::Rect { center, .. } => Vec2::new(center[0], center[1]),
        _ => Vec2::ZERO,
    }
}

/// Checks if a point is inside a shape.
fn is_point_in_shape(point: &Vec2, shape: &EvaluatedShape) -> bool {
    match shape {
        EvaluatedShape::Circle { center, radius } => {
            let d = Vec2::new(point.x - center[0], point.y - center[1]);
            d.length_squared() <= radius * radius
        }
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let local = Vec2::new(point.x - center[0], point.y - center[1]);
            let (sin, cos) = rotation.to_radians().sin_cos();
            let rotated = Vec2::new(
                local.x * cos + local.y * sin,
                -local.x * sin + local.y * cos,
            );
            rotated.x.abs() <= size[0] / 2.0 && rotated.y.abs() <= size[1] / 2.0
        }
        _ => false, // Line, Bezier are not supported as areas
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
pub fn clear_external_forces(mut forces: Query<&mut PhysicsExternalForce>) {
    for mut force in forces.iter_mut() {
        force.force = Vec2::ZERO;
        force.torque = 0.0;
    }
}
