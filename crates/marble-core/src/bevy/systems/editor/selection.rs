//! Selection handling systems for the editor.

use bevy::prelude::*;
use bevy_rapier2d::prelude::Sensor;

use super::{EditorStateRes, SelectObjectEvent, UpdateObjectEvent};
use crate::bevy::systems::map_loader::{create_obstacle_collider, create_trigger_collider};
use crate::bevy::{GuidelineMarker, MapConfig, VectorFieldZone};
use crate::dsl::GameContext;
use crate::map::{EvaluatedShape, ObjectRole};

/// System to handle selection events.
pub fn handle_selection_events(
    mut editor_state: ResMut<EditorStateRes>,
    mut events: MessageReader<SelectObjectEvent>,
) {
    for event in events.read() {
        editor_state.selected_object = event.0;
        // Clear keyframe selection when object selection changes
        if event.0.is_some() {
            editor_state.selected_sequence = None;
            editor_state.selected_keyframe = None;
        }
    }
}

/// System to handle object update events.
///
/// Updates both MapConfig and entity transforms when objects change.
/// For guidelines, also updates the GuidelineMarker component.
/// For obstacles and triggers, also updates the Collider component.
pub fn handle_object_updates(
    mut commands: Commands,
    mut map_config: Option<ResMut<MapConfig>>,
    mut events: MessageReader<UpdateObjectEvent>,
    object_map: Res<crate::bevy::ObjectEntityMap>,
    mut transforms: Query<&mut Transform>,
    mut guideline_markers: Query<&mut GuidelineMarker>,
    mut vector_field_zones: Query<&mut VectorFieldZone>,
) {
    let Some(ref mut config) = map_config else {
        return;
    };

    let ctx = GameContext::new(0.0, 0);

    for event in events.read() {
        // Update MapConfig
        if let Some(obj) = config.0.objects.get_mut(event.index) {
            *obj = event.object.clone();
        }

        // Find entity by index (most reliable method)
        let entity = object_map.get_by_index(event.index);

        // Fallback: try to find by ID if index lookup fails
        let entity = entity.or_else(|| {
            event.object.id.as_ref().and_then(|id| object_map.get(id))
        });

        let Some(entity) = entity else {
            tracing::warn!(
                "[selection] Could not find entity for object at index {}: {:?}",
                event.index,
                event.object.id
            );
            continue;
        };

        let shape = event.object.shape.evaluate(&ctx);

        // Update transform
        if let Ok(mut transform) = transforms.get_mut(entity) {
            let (pos, rot) = get_shape_transform(&shape);
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
            transform.rotation = Quat::from_rotation_z(rot);
        }

        // Update Collider for physics objects
        match event.object.role {
            ObjectRole::Obstacle => {
                let (_, _, collider) = create_obstacle_collider(&shape);
                commands.entity(entity).insert(collider);
            }
            ObjectRole::Trigger => {
                let (_, _, collider) = create_trigger_collider(&shape);
                commands.entity(entity).insert((collider, Sensor));
            }
            _ => {} // Spawner, VectorField, Guideline don't have physics colliders
        }

        // Update GuidelineMarker if this is a guideline
        if event.object.role == ObjectRole::Guideline {
            if let Ok(mut guideline) = guideline_markers.get_mut(entity) {
                // Update start/end from shape
                if let EvaluatedShape::Line { start, end } = shape {
                    guideline.start = Vec2::new(start[0], start[1]);
                    guideline.end = Vec2::new(end[0], end[1]);
                }

                // Update properties from object
                if let Some(props) = &event.object.properties.guideline {
                    guideline.show_ruler = props.show_ruler;
                    guideline.snap_enabled = props.snap_enabled;
                    guideline.snap_distance = props.snap_distance;
                    guideline.ruler_interval = props.ruler_interval;
                    if let Some(color) = props.color {
                        guideline.color = Color::srgba(color[0], color[1], color[2], color[3]);
                    }
                }
            }
        }

        // Update VectorFieldZone if this is a vector field
        if event.object.role == ObjectRole::VectorField {
            if let Ok(mut zone) = vector_field_zones.get_mut(entity) {
                // Update shape
                zone.shape = event.object.shape.clone();

                // Update properties from object
                if let Some(props) = &event.object.properties.vector_field {
                    zone.direction = props.direction.clone();
                    zone.magnitude = props.magnitude.clone();
                    zone.enabled = props.enabled.clone();
                    zone.falloff = props.falloff;
                }
            }
        }
    }
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

/// System to validate selection (ensure it's still valid after map changes).
pub fn validate_selection(
    mut editor_state: ResMut<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
) {
    let Some(config) = map_config else {
        return;
    };

    // Validate object selection
    if let Some(idx) = editor_state.selected_object {
        if idx >= config.0.objects.len() {
            editor_state.selected_object = None;
        }
    }

    // Validate sequence selection
    if let Some(idx) = editor_state.selected_sequence {
        if idx >= config.0.keyframes.len() {
            editor_state.selected_sequence = None;
            editor_state.selected_keyframe = None;
        } else if let Some(kf_idx) = editor_state.selected_keyframe {
            if kf_idx >= config.0.keyframes[idx].keyframes.len() {
                editor_state.selected_keyframe = None;
            }
        }
    }
}
