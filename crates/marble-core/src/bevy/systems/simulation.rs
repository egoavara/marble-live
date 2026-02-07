//! Simulation control systems for the editor.
//!
//! Handles start, stop, and reset simulation events.

use bevy::prelude::*;

use crate::bevy::MapLoadedEvent;
use crate::bevy::Marble;
use crate::bevy::events::{ResetSimulationEvent, StartSimulationEvent, StopSimulationEvent};
use crate::bevy::plugin::EditorState;
use crate::bevy::rapier_plugin::{PhysicsBody, PhysicsWorldRes};
use crate::bevy::resources::{
    DeterministicRng, InitialTransforms, KeyframeExecutors, MapConfig, MarbleGameState,
};
use crate::keyframe::KeyframeExecutor;

/// System to handle start simulation event.
///
/// Transitions from Editing to Simulating state and initializes keyframe executors.
pub fn handle_start_simulation(
    mut state: ResMut<NextState<EditorState>>,
    mut events: MessageReader<StartSimulationEvent>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    map_config: Option<Res<MapConfig>>,
) {
    for _ in events.read() {
        tracing::info!("[simulation] Starting simulation");
        state.set(EditorState::Simulating);

        // Initialize keyframe executors for all autoplay sequences
        if let Some(config) = &map_config {
            keyframe_executors.clear();
            for seq in &config.0.keyframes {
                if seq.autoplay {
                    tracing::info!(
                        "[simulation] Adding keyframe executor for sequence: {}",
                        seq.name
                    );
                    keyframe_executors.add(KeyframeExecutor::new(seq.name.clone()));
                }
            }
            // Activate all keyframes for simulation
            keyframe_executors.activate_all();
        }
    }
}

/// System to handle stop simulation event.
///
/// Transitions from Simulating back to Editing state, clears keyframe executors,
/// and despawns all marbles.
pub fn handle_stop_simulation(
    mut state: ResMut<NextState<EditorState>>,
    mut events: MessageReader<StopSimulationEvent>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    initial_transforms: Res<InitialTransforms>,
    object_entity_map: Res<crate::bevy::ObjectEntityMap>,
    mut transforms: Query<&mut Transform>,
    mut commands: Commands,
    marble_entities: Query<(Entity, Option<&PhysicsBody>), With<Marble>>,
    mut physics: ResMut<PhysicsWorldRes>,
) {
    for _ in events.read() {
        tracing::info!("[simulation] Stopping simulation");
        state.set(EditorState::Editing);

        // Deactivate and clear keyframe executors
        keyframe_executors.deactivate();
        keyframe_executors.clear();

        // Despawn all marbles (remove from physics too)
        for (entity, body) in marble_entities.iter() {
            if let Some(body) = body {
                physics.world.remove_rigid_body(body.0);
            }
            commands.entity(entity).despawn();
        }

        // Reset animated object positions to initial transforms
        for (object_id, (pos, rot)) in &initial_transforms.transforms {
            if let Some(entity) = object_entity_map.get(object_id) {
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.translation.x = pos.x;
                    transform.translation.y = pos.y;
                    transform.rotation = Quat::from_rotation_z(*rot);
                }
            }
        }
    }
}

/// System to handle reset simulation event.
///
/// Resets game state, marble positions, and keyframe state to initial values.
pub fn handle_reset_simulation(
    mut events: MessageReader<ResetSimulationEvent>,
    mut game_state: ResMut<MarbleGameState>,
    mut rng: ResMut<DeterministicRng>,
    initial_transforms: Res<InitialTransforms>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    mut commands: Commands,
    marble_entities: Query<(Entity, Option<&PhysicsBody>), With<Marble>>,
    mut transforms: Query<&mut Transform>,
    object_entity_map: Res<crate::bevy::ObjectEntityMap>,
    mut physics: ResMut<PhysicsWorldRes>,
) {
    for _ in events.read() {
        tracing::info!("[simulation] Resetting simulation");

        // Reset game state
        game_state.frame = 0;
        game_state.arrival_order.clear();

        // Reset RNG for determinism
        rng.reset();

        // Reset keyframe executors
        for executor in &mut keyframe_executors.executors {
            executor.reset();
        }

        // Despawn all marbles (remove from physics too)
        for (entity, body) in marble_entities.iter() {
            if let Some(body) = body {
                physics.world.remove_rigid_body(body.0);
            }
            commands.entity(entity).despawn();
        }

        // Reset animated object positions to initial transforms
        for (object_id, (pos, rot)) in &initial_transforms.transforms {
            if let Some(entity) = object_entity_map.get(object_id) {
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.translation.x = pos.x;
                    transform.translation.y = pos.y;
                    transform.rotation = Quat::from_rotation_z(*rot);
                }
            }
        }
    }
}

/// System to clear keyframe executors after map load in editor mode.
///
/// In the editor, we don't want autoplay sequences to run automatically.
/// They should only run when simulation is explicitly started.
pub fn clear_executors_on_map_load(
    mut events: MessageReader<MapLoadedEvent>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
) {
    for event in events.read() {
        tracing::info!(
            "[simulation] Map loaded ({}), clearing autoplay executors for editor",
            event.map_name
        );
        keyframe_executors.deactivate();
        keyframe_executors.clear();
    }
}
