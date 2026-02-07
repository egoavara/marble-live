//! Keyframe animation systems.
//!
//! Uses the existing KeyframeExecutor logic from the legacy system.

use std::collections::HashMap;

use bevy::ecs::prelude::*;
use bevy::prelude::*;

use crate::bevy::{
    GameContextRes, InitialTransforms, KeyframeExecutors, KeyframeTarget, MapConfig,
    ObjectEntityMap,
};
use crate::physics::PHYSICS_DT;

/// Collected keyframe updates to apply.
#[derive(Resource, Default)]
pub struct KeyframeUpdates {
    pub updates: Vec<(Entity, [f32; 2], f32)>,
}

/// System to calculate keyframe animations.
///
/// This system calculates the new positions but stores them in a resource.
/// The actual transform updates are applied by `apply_keyframe_updates`.
pub fn update_keyframe_animations(
    mut executors: ResMut<KeyframeExecutors>,
    map_config: Option<Res<MapConfig>>,
    mut game_context: ResMut<GameContextRes>,
    keyframe_targets: Query<(&KeyframeTarget, &Transform)>,
    initial_transforms: Res<InitialTransforms>,
    object_entity_map: Res<ObjectEntityMap>,
    mut keyframe_updates: ResMut<KeyframeUpdates>,
) {
    // Clear previous updates
    keyframe_updates.updates.clear();

    let Some(config) = map_config else {
        return;
    };

    if executors.executors.is_empty() {
        return;
    }

    // Collect current positions from keyframe targets
    let mut current_positions: HashMap<String, ([f32; 2], f32)> = HashMap::new();
    for (target, transform) in keyframe_targets.iter() {
        let pos = transform.translation.truncate();
        let rot = transform.rotation.to_euler(EulerRot::ZYX).0;
        current_positions.insert(target.object_id.clone(), ([pos.x, pos.y], rot));
    }

    // Convert initial transforms to the format expected by KeyframeExecutor
    let initial_transforms_map: HashMap<String, ([f32; 2], f32)> = initial_transforms
        .transforms
        .iter()
        .map(|(k, (pos, rot))| (k.clone(), ([pos.x, pos.y], *rot)))
        .collect();

    // Clone activated state to avoid borrow conflict
    let activated = executors.activated.clone();

    // Update each executor (only if activated)
    for executor in &mut executors.executors {
        // Check if this sequence should be executed
        if !activated.should_execute(executor.sequence_name()) {
            continue;
        }

        let updates = executor.update(
            PHYSICS_DT,
            &config.0.keyframes,
            &current_positions,
            &initial_transforms_map,
            &mut game_context.context,
        );

        // Store updates for application
        for (id, pos, rot) in updates {
            if let Some(entity) = object_entity_map.get(&id) {
                keyframe_updates.updates.push((entity, pos, rot));
            }
        }
    }

    // Remove finished executors (but keep infinite loops running)
    executors.retain_active();
}

/// System to apply keyframe updates to transforms.
///
/// This runs after `update_keyframe_animations` and applies the calculated
/// transforms to entities.
pub fn apply_keyframe_updates(
    keyframe_updates: Res<KeyframeUpdates>,
    mut transforms: Query<&mut Transform>,
) {
    for &(entity, pos, rot) in &keyframe_updates.updates {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation.x = pos[0];
            transform.translation.y = pos[1];
            transform.rotation = Quat::from_rotation_z(rot);
        }
    }
}

/// System to update keyframe target transforms (separate for borrow checker).
#[allow(dead_code)]
pub fn apply_keyframe_transforms(
    _keyframe_targets: Query<(&KeyframeTarget, &mut Transform)>,
    _executors: Res<KeyframeExecutors>,
    _map_config: Option<Res<MapConfig>>,
    _game_context: Res<GameContextRes>,
    _initial_transforms: Res<InitialTransforms>,
) {
    // This system runs after update_keyframe_animations
    // It applies the calculated transforms from a shared cache
    // For now, we handle this in update_keyframe_animations directly
}
