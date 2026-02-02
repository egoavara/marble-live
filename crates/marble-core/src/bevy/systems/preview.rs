//! Keyframe preview systems for the editor.
//!
//! Allows previewing keyframe animations without running the simulation.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::bevy::events::PreviewSequenceEvent;
use crate::bevy::plugin::EditorState;
use crate::bevy::resources::{InitialTransforms, KeyframeExecutors, MapConfig, ObjectEntityMap};
use crate::bevy::systems::editor::EditorStateRes;
use crate::bevy::systems::keyframe::KeyframeUpdates;
use crate::dsl::GameContext;
use crate::keyframe::KeyframeExecutor;
use crate::map::{EvaluatedShape, MapObject};

/// Local resource to track previous keyframe selection state.
#[derive(Default)]
pub struct PreviousKeyframeSelection {
    pub keyframe_index: Option<usize>,
    pub sequence_index: Option<usize>,
}

/// System to handle preview sequence event.
///
/// Toggles between Editing and Preview state and activates the selected sequence.
/// When starting preview, fast-forwards the executor to the selected keyframe.
pub fn handle_preview_sequence(
    mut state: ResMut<NextState<EditorState>>,
    current_state: Res<State<EditorState>>,
    mut events: MessageReader<PreviewSequenceEvent>,
    mut editor_state: ResMut<EditorStateRes>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    map_config: Option<Res<MapConfig>>,
    initial_transforms: Res<InitialTransforms>,
    object_entity_map: Res<ObjectEntityMap>,
    mut transforms: Query<&mut Transform>,
) {
    for event in events.read() {
        if event.start {
            tracing::info!("[preview] Starting preview");
            state.set(EditorState::Preview);
            editor_state.is_previewing = true;

            // Set up keyframe executor for the selected sequence only
            if let Some(seq_index) = editor_state.selected_sequence {
                if let Some(config) = &map_config {
                    if let Some(sequence) = config.0.keyframes.get(seq_index) {
                        // Clear existing executors and add only the selected sequence
                        keyframe_executors.clear();

                        let mut executor = KeyframeExecutor::new(sequence.name.clone());

                        // Convert InitialTransforms to HashMap format
                        let initial_transforms_map: HashMap<String, ([f32; 2], f32)> =
                            initial_transforms
                                .transforms
                                .iter()
                                .map(|(k, (pos, rot))| (k.clone(), ([pos.x, pos.y], *rot)))
                                .collect();

                        // If a specific keyframe is selected, fast-forward to it
                        if let Some(kf_idx) = editor_state.selected_keyframe {
                            tracing::info!(
                                "[preview] Fast-forwarding to keyframe {} in sequence {}",
                                kf_idx,
                                sequence.name
                            );

                            let state = executor.fast_forward_to(
                                kf_idx,
                                &config.0.keyframes,
                                &initial_transforms_map,
                            );

                            // Apply the fast-forwarded state immediately
                            for (target_id, (pos, rot)) in &state {
                                if let Some(entity) = object_entity_map.get(target_id) {
                                    if let Ok(mut transform) = transforms.get_mut(entity) {
                                        transform.translation.x = pos[0];
                                        transform.translation.y = pos[1];
                                        transform.rotation = Quat::from_rotation_z(*rot);
                                    }
                                }
                            }
                        }

                        keyframe_executors.add(executor);
                        // Activate only this specific sequence
                        keyframe_executors.activate_sequences(vec![sequence.name.clone()]);
                        tracing::info!("[preview] Activated sequence: {}", sequence.name);
                    }
                }
            }
        } else {
            tracing::info!("[preview] Stopping preview");
            // Only transition back to Editing if we were in Preview
            if *current_state.get() == EditorState::Preview {
                state.set(EditorState::Editing);
            }
            editor_state.is_previewing = false;

            // Deactivate and clear executors
            keyframe_executors.deactivate();
            keyframe_executors.clear();
        }
    }
}

/// System to handle keyframe selection changes during preview.
///
/// This system detects when the selected keyframe changes and resets the
/// KeyframeExecutor to fast-forward to the new selection. It does NOT
/// directly manipulate transforms - that's handled by `apply_keyframe_updates`.
///
/// This prevents the double-update issue where both this system and the
/// keyframe animation system were writing different values to transforms.
pub fn update_preview_transforms(
    editor_state: Res<EditorStateRes>,
    mut prev_selection: Local<PreviousKeyframeSelection>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    map_config: Option<Res<MapConfig>>,
    initial_transforms: Res<InitialTransforms>,
    object_entity_map: Res<ObjectEntityMap>,
    mut transforms: Query<&mut Transform>,
    mut keyframe_updates: ResMut<KeyframeUpdates>,
) {
    // Only run when previewing
    if !editor_state.is_previewing {
        // Reset previous selection when not previewing
        prev_selection.keyframe_index = None;
        prev_selection.sequence_index = None;
        return;
    }

    let Some(config) = map_config else {
        return;
    };

    let Some(seq_index) = editor_state.selected_sequence else {
        return;
    };

    let current_keyframe = editor_state.selected_keyframe;

    // Check if selection changed
    let selection_changed = prev_selection.keyframe_index != current_keyframe
        || prev_selection.sequence_index != Some(seq_index);

    if !selection_changed {
        // No change - let KeyframeExecutor handle smooth animation
        return;
    }

    // Selection changed - update tracking
    prev_selection.keyframe_index = current_keyframe;
    prev_selection.sequence_index = Some(seq_index);

    tracing::info!(
        "[preview] Keyframe selection changed to {:?}, fast-forwarding executor",
        current_keyframe
    );

    // Convert InitialTransforms to HashMap format
    let initial_transforms_map: HashMap<String, ([f32; 2], f32)> = initial_transforms
        .transforms
        .iter()
        .map(|(k, (pos, rot))| (k.clone(), ([pos.x, pos.y], *rot)))
        .collect();

    let Some(keyframe_index) = current_keyframe else {
        // No keyframe selected - reset to initial transforms
        // Apply initial transforms directly (one-time reset)
        for (object_id, (pos, rot)) in &initial_transforms_map {
            if let Some(entity) = object_entity_map.get(object_id) {
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.translation.x = pos[0];
                    transform.translation.y = pos[1];
                    transform.rotation = Quat::from_rotation_z(*rot);
                }
            }
        }

        // Reset all executors
        for executor in &mut keyframe_executors.executors {
            executor.reset();
        }
        return;
    };

    // Fast-forward each executor to the selected keyframe
    // and apply the calculated state immediately
    for executor in &mut keyframe_executors.executors {
        let state = executor.fast_forward_to(
            keyframe_index,
            &config.0.keyframes,
            &initial_transforms_map,
        );

        // Apply the fast-forwarded state immediately (one-time)
        // and queue updates for the animation system to continue from
        for (target_id, (pos, rot)) in &state {
            if let Some(entity) = object_entity_map.get(target_id) {
                // Apply immediately for instant visual feedback
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.translation.x = pos[0];
                    transform.translation.y = pos[1];
                    transform.rotation = Quat::from_rotation_z(*rot);
                }

                // Also queue the update so keyframe system has correct starting state
                keyframe_updates.updates.push((entity, *pos, *rot));
            }
        }
    }
}

/// Build initial transforms map from MapConfig objects.
fn build_initial_transforms_from_config(objects: &[MapObject]) -> HashMap<String, ([f32; 2], f32)> {
    let ctx = GameContext::new(0.0, 0);
    let mut transforms = HashMap::new();

    for obj in objects {
        if let Some(ref id) = obj.id {
            let shape = obj.shape.evaluate(&ctx);
            let (pos, rot) = get_shape_transform(&shape);
            transforms.insert(id.clone(), ([pos.x, pos.y], rot));
        }
    }

    transforms
}

/// Get position and rotation from evaluated shape.
fn get_shape_transform(shape: &EvaluatedShape) -> (Vec2, f32) {
    match shape {
        EvaluatedShape::Line { start, end } => {
            let mid = Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0);
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let angle = dy.atan2(dx);
            (mid, angle)
        }
        EvaluatedShape::Circle { center, .. } => (Vec2::new(center[0], center[1]), 0.0),
        EvaluatedShape::Rect {
            center, rotation, ..
        } => (Vec2::new(center[0], center[1]), rotation.to_radians()),
        EvaluatedShape::Bezier { .. } => (Vec2::ZERO, 0.0),
    }
}

/// System to restore transforms when exiting preview mode.
pub fn on_exit_preview(
    map_config: Option<Res<MapConfig>>,
    object_entity_map: Res<ObjectEntityMap>,
    mut transforms: Query<&mut Transform>,
) {
    tracing::info!("[preview] Exiting preview, restoring transforms");

    let Some(config) = map_config else {
        return;
    };

    // Build initial transforms from current MapConfig
    let initial_transforms = build_initial_transforms_from_config(&config.0.objects);

    // Reset all objects to their initial transforms
    for (object_id, (pos, rot)) in &initial_transforms {
        if let Some(entity) = object_entity_map.get(object_id) {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.translation.x = pos[0];
                transform.translation.y = pos[1];
                transform.rotation = Quat::from_rotation_z(*rot);
            }
        }
    }
}
