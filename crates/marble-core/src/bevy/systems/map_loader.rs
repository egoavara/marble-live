//! Map loading systems.
//!
//! Handles spawning map objects into the ECS world.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::bevy::{
    AddObjectEvent, AnimatedObject, BlackholeZone, DeleteObjectEvent, GuidelineMarker,
    InitialTransforms, KeyframeExecutors, KeyframeTarget, LoadMapEvent, MapConfig,
    MapLoadedEvent, MapObjectMarker, ObjectEntityMap, SpawnerZone, TriggerZone,
};
use crate::dsl::GameContext;
use crate::keyframe::KeyframeExecutor;
use crate::map::{EvaluatedShape, Keyframe, KeyframeSequence, ObjectRole, RouletteConfig};

/// System to handle map loading requests.
pub fn handle_load_map(
    mut commands: Commands,
    mut events: MessageReader<LoadMapEvent>,
    mut map_loaded: MessageWriter<MapLoadedEvent>,
    mut object_map: ResMut<ObjectEntityMap>,
    mut initial_transforms: ResMut<InitialTransforms>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    existing_objects: Query<Entity, With<MapObjectMarker>>,
) {
    for event in events.read() {
        // Clear existing map objects
        for entity in existing_objects.iter() {
            commands.entity(entity).despawn();
        }
        object_map.clear();
        initial_transforms.clear();
        keyframe_executors.clear();

        let config = &event.config;
        let ctx = GameContext::new(0.0, 0);
        let keyframe_targets: HashSet<String> = collect_keyframe_target_ids(config);

        let mut trigger_index = 0;

        // Spawn map objects
        for obj in &config.objects {
            let shape = obj.shape.evaluate(&ctx);
            let is_animated = is_animatable(obj, &keyframe_targets);

            match obj.role {
                ObjectRole::Spawner => {
                    spawn_spawner(&mut commands, obj);
                }
                ObjectRole::Obstacle => {
                    let entity = spawn_obstacle(&mut commands, obj, &shape, &ctx, is_animated);

                    if let Some(ref id) = obj.id {
                        object_map.map.insert(id.clone(), entity);

                        if is_animated {
                            let (pos, rot) = get_shape_transform(&shape);
                            initial_transforms.insert(id.clone(), pos, rot);
                        }
                    }

                    // Add blackhole if present
                    if let Some(bh) = &obj.properties.blackhole {
                        commands.entity(entity).insert(BlackholeZone {
                            shape: obj.shape.clone(),
                            force: bh.force.clone(),
                        });
                    }

                    // Add keyframe target if applicable (including roll objects)
                    if let Some(ref id) = obj.id {
                        if keyframe_targets.contains(id.as_str()) || obj.properties.roll.is_some() {
                            commands.entity(entity).insert(KeyframeTarget {
                                object_id: id.clone(),
                            });
                        }
                    }
                }
                ObjectRole::Trigger => {
                    let entity = spawn_trigger(&mut commands, obj, &shape, trigger_index);

                    if let Some(ref id) = obj.id {
                        object_map.map.insert(id.clone(), entity);
                    }

                    // Add blackhole if present
                    if let Some(bh) = &obj.properties.blackhole {
                        commands.entity(entity).insert(BlackholeZone {
                            shape: obj.shape.clone(),
                            force: bh.force.clone(),
                        });
                    }

                    trigger_index += 1;
                }
                ObjectRole::Guideline => {
                    // Guidelines are editor-only, spawned without physics colliders
                    let entity = spawn_guideline(&mut commands, obj, &shape);

                    if let Some(ref id) = obj.id {
                        object_map.map.insert(id.clone(), entity);
                    }
                }
            }
        }

        // Create synthetic keyframe sequences for roll objects
        let mut modified_config = config.clone();

        // 1. Remove existing property_managed sequences to avoid duplicates
        modified_config
            .keyframes
            .retain(|seq| !seq.property_managed);

        // 2. Create new property-managed sequences for roll objects
        for obj in &config.objects {
            if let (Some(id), Some(roll)) = (&obj.id, &obj.properties.roll) {
                let synthetic_name = format!("__roll_{}", id);
                let synthetic_seq = KeyframeSequence {
                    name: synthetic_name.clone(),
                    target_ids: vec![id.clone()],
                    keyframes: vec![
                        Keyframe::LoopStart { count: None }, // Infinite loop
                        Keyframe::ContinuousRotate {
                            speed: roll.speed,
                            direction: roll.direction,
                        },
                        Keyframe::LoopEnd,
                    ],
                    autoplay: true,
                    property_managed: true,
                };
                modified_config.keyframes.push(synthetic_seq);
            }
        }

        // Initialize keyframe executors (including synthetic roll sequences)
        for seq in &modified_config.keyframes {
            if seq.autoplay {
                keyframe_executors.add(KeyframeExecutor::new(seq.name.clone()));
            }
        }

        // Insert map config as resource (with synthetic sequences added)
        commands.insert_resource(MapConfig::new(modified_config));

        // Send map loaded event
        map_loaded.write(MapLoadedEvent {
            map_name: config.meta.name.clone(),
        });
    }
}

fn spawn_spawner(commands: &mut Commands, obj: &crate::map::MapObject) {
    let spawn_props = obj.properties.spawn.as_ref();

    commands.spawn((
        MapObjectMarker {
            object_id: obj.id.clone(),
            role: ObjectRole::Spawner,
        },
        SpawnerZone {
            shape: obj.shape.clone(),
            mode: spawn_props
                .map(|p| p.mode.clone())
                .unwrap_or_else(|| "random".to_string()),
            initial_force: spawn_props
                .map(|p| p.initial_force.clone())
                .unwrap_or_else(|| "random".to_string()),
        },
        Transform::default(),
        Visibility::default(),
    ));
}

fn spawn_obstacle(
    commands: &mut Commands,
    obj: &crate::map::MapObject,
    shape: &EvaluatedShape,
    ctx: &GameContext,
    is_animated: bool,
) -> Entity {
    let (position, rotation, collider) = create_obstacle_collider(shape);

    let mut entity_commands = commands.spawn((
        MapObjectMarker {
            object_id: obj.id.clone(),
            role: ObjectRole::Obstacle,
        },
        Transform::from_translation(position.extend(0.0))
            .with_rotation(Quat::from_rotation_z(rotation)),
        collider,
        Friction::coefficient(0.3),
    ));

    if is_animated {
        entity_commands.insert((
            RigidBody::KinematicPositionBased,
            AnimatedObject {
                initial_position: position,
                initial_rotation: rotation,
            },
        ));
        // Set appropriate restitution
        let restitution = if let Some(bumper) = &obj.properties.bumper {
            let force = bumper.force.evaluate(ctx);
            0.6 + force * 0.4
        } else {
            0.6
        };
        entity_commands.insert(Restitution::coefficient(restitution));
    } else {
        // Static collider
        let restitution = if let Some(bumper) = &obj.properties.bumper {
            let force = bumper.force.evaluate(ctx);
            0.6 + force * 0.4
        } else {
            0.5
        };
        entity_commands.insert(Restitution::coefficient(restitution));
    }

    entity_commands.id()
}

fn spawn_trigger(
    commands: &mut Commands,
    obj: &crate::map::MapObject,
    shape: &EvaluatedShape,
    trigger_index: usize,
) -> Entity {
    let (position, rotation) = match shape {
        EvaluatedShape::Circle { center, .. } => (Vec2::new(center[0], center[1]), 0.0),
        EvaluatedShape::Rect {
            center, rotation, ..
        } => (Vec2::new(center[0], center[1]), rotation.to_radians()),
        _ => (Vec2::ZERO, 0.0),
    };

    let collider = match shape {
        EvaluatedShape::Circle { radius, .. } => Collider::ball(*radius),
        EvaluatedShape::Rect { size, .. } => Collider::cuboid(size[0] / 2.0, size[1] / 2.0),
        _ => Collider::ball(0.1),
    };

    let action = obj
        .properties
        .trigger
        .as_ref()
        .map(|t| t.action.clone())
        .unwrap_or_else(|| "gamerule".to_string());

    commands
        .spawn((
            MapObjectMarker {
                object_id: obj.id.clone(),
                role: ObjectRole::Trigger,
            },
            TriggerZone {
                action,
                trigger_index,
            },
            Transform::from_translation(position.extend(0.0))
                .with_rotation(Quat::from_rotation_z(rotation)),
            collider,
            Sensor,
        ))
        .id()
}

fn create_obstacle_collider(shape: &EvaluatedShape) -> (Vec2, f32, Collider) {
    match shape {
        EvaluatedShape::Line { start, end } => {
            let mid = Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0);
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let length = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);

            (mid, angle, Collider::cuboid(length / 2.0, 0.02))
        }
        EvaluatedShape::Circle { center, radius } => {
            (Vec2::new(center[0], center[1]), 0.0, Collider::ball(*radius))
        }
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let rotation_rad = rotation.to_radians();
            (
                Vec2::new(center[0], center[1]),
                rotation_rad,
                Collider::cuboid(size[0] / 2.0, size[1] / 2.0),
            )
        }
        EvaluatedShape::Bezier { .. } => {
            // Convert bezier to polyline
            let points = shape.bezier_to_points().unwrap_or_default();
            let vertices: Vec<_> = points.iter().map(|p| Vect::new(p[0], p[1])).collect();

            if vertices.len() >= 2 {
                let indices: Vec<[u32; 2]> = (0..vertices.len() as u32 - 1)
                    .map(|i| [i, i + 1])
                    .collect();
                (
                    Vec2::ZERO,
                    0.0,
                    Collider::polyline(vertices, Some(indices)),
                )
            } else {
                (Vec2::ZERO, 0.0, Collider::ball(0.1))
            }
        }
    }
}

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

fn collect_keyframe_target_ids(config: &RouletteConfig) -> HashSet<String> {
    let mut targets = HashSet::new();
    for seq in &config.keyframes {
        targets.extend(seq.target_ids.iter().cloned());
    }
    targets
}

fn is_animatable(obj: &crate::map::MapObject, keyframe_targets: &HashSet<String>) -> bool {
    if obj.properties.roll.is_some() {
        return true;
    }
    if let Some(id) = &obj.id {
        if keyframe_targets.contains(id.as_str()) {
            return true;
        }
    }
    false
}

/// System to handle adding a new object to the map.
pub fn handle_add_object(
    mut commands: Commands,
    mut events: MessageReader<AddObjectEvent>,
    map_config: Option<Res<MapConfig>>,
    mut object_map: ResMut<ObjectEntityMap>,
    mut initial_transforms: ResMut<InitialTransforms>,
) {
    for event in events.read() {
        let obj = &event.object;
        let ctx = GameContext::new(0.0, 0);

        // Get keyframe targets from config
        let keyframe_targets: HashSet<String> = map_config
            .as_ref()
            .map(|c| collect_keyframe_target_ids(&c.0))
            .unwrap_or_default();

        let is_animated = is_animatable(obj, &keyframe_targets);

        match obj.role {
            ObjectRole::Spawner => {
                spawn_spawner(&mut commands, obj);
            }
            ObjectRole::Obstacle => {
                let shape = obj.shape.evaluate(&ctx);
                let entity = spawn_obstacle(&mut commands, obj, &shape, &ctx, is_animated);

                if let Some(ref id) = obj.id {
                    object_map.map.insert(id.clone(), entity);

                    if is_animated {
                        let (pos, rot) = get_shape_transform(&shape);
                        initial_transforms.insert(id.clone(), pos, rot);
                    }
                }

                // Add blackhole if present
                if let Some(bh) = &obj.properties.blackhole {
                    commands.entity(entity).insert(BlackholeZone {
                        shape: obj.shape.clone(),
                        force: bh.force.clone(),
                    });
                }

                // Add keyframe target if applicable
                if let Some(ref id) = obj.id {
                    if keyframe_targets.contains(id.as_str()) || obj.properties.roll.is_some() {
                        commands.entity(entity).insert(KeyframeTarget {
                            object_id: id.clone(),
                        });
                    }
                }
            }
            ObjectRole::Trigger => {
                let shape = obj.shape.evaluate(&ctx);
                // Use a placeholder trigger index (this is for editor preview only)
                let trigger_index = 999;
                let entity = spawn_trigger(&mut commands, obj, &shape, trigger_index);

                if let Some(ref id) = obj.id {
                    object_map.map.insert(id.clone(), entity);
                }

                // Add blackhole if present
                if let Some(bh) = &obj.properties.blackhole {
                    commands.entity(entity).insert(BlackholeZone {
                        shape: obj.shape.clone(),
                        force: bh.force.clone(),
                    });
                }
            }
            ObjectRole::Guideline => {
                let shape = obj.shape.evaluate(&ctx);
                let entity = spawn_guideline(&mut commands, obj, &shape);

                if let Some(ref id) = obj.id {
                    object_map.map.insert(id.clone(), entity);
                }
            }
        }

        tracing::info!("[map_loader] Added object: {:?}", obj.id);
    }
}

/// System to handle deleting an object from the map.
pub fn handle_delete_object(
    mut commands: Commands,
    mut events: MessageReader<DeleteObjectEvent>,
    map_config: Option<Res<MapConfig>>,
    mut object_map: ResMut<ObjectEntityMap>,
    existing_objects: Query<(Entity, &MapObjectMarker)>,
) {
    for event in events.read() {
        let index = event.index;
        tracing::info!("[map_loader] Deleting object at index: {}", index);

        // Get the object ID from the config
        let object_id = map_config
            .as_ref()
            .and_then(|c| c.0.objects.get(index))
            .and_then(|obj| obj.id.clone());

        // Find and despawn the entity
        if let Some(id) = &object_id {
            if let Some(entity) = object_map.map.remove(id) {
                commands.entity(entity).despawn();
                tracing::info!("[map_loader] Despawned entity for object: {}", id);
            }
        } else {
            // If no ID, try to find by index (less reliable)
            // This is a fallback for objects without IDs
            let mut found = false;
            for (_entity, marker) in existing_objects.iter() {
                if marker.object_id.is_none() {
                    // Can't reliably match by index, just log
                    tracing::warn!("[map_loader] Object at index {} has no ID, cannot reliably delete", index);
                    // For now, don't delete to avoid deleting wrong object
                    found = true;
                    break;
                }
            }
            if !found {
                tracing::warn!("[map_loader] Could not find object at index {} to delete", index);
            }
        }
    }
}

/// Spawn a guideline entity (editor-only, no physics collider).
fn spawn_guideline(
    commands: &mut Commands,
    obj: &crate::map::MapObject,
    shape: &EvaluatedShape,
) -> Entity {
    use bevy::prelude::Color;

    // Extract line endpoints for snap calculations
    let (position, rotation, line_start, line_end) = match shape {
        EvaluatedShape::Line { start, end } => {
            let mid = Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0);
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let angle = dy.atan2(dx);
            (
                mid,
                angle,
                Vec2::new(start[0], start[1]),
                Vec2::new(end[0], end[1]),
            )
        }
        // Other shapes default to center at origin (shouldn't happen for guidelines)
        EvaluatedShape::Circle { center, .. } => (
            Vec2::new(center[0], center[1]),
            0.0,
            Vec2::ZERO,
            Vec2::new(0.0, 10.0),
        ),
        EvaluatedShape::Rect { center, rotation, .. } => (
            Vec2::new(center[0], center[1]),
            rotation.to_radians(),
            Vec2::ZERO,
            Vec2::new(0.0, 10.0),
        ),
        EvaluatedShape::Bezier { start, end, .. } => {
            let mid = Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0);
            (
                mid,
                0.0,
                Vec2::new(start[0], start[1]),
                Vec2::new(end[0], end[1]),
            )
        }
    };

    // Get guideline properties or use defaults
    let guideline_props = obj.properties.guideline.as_ref();

    let color = guideline_props
        .and_then(|p| p.color)
        .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
        .unwrap_or(Color::srgba(0.0, 0.8, 0.8, 0.8)); // Default cyan

    let show_ruler = guideline_props.map(|p| p.show_ruler).unwrap_or(true);
    let snap_enabled = guideline_props.map(|p| p.snap_enabled).unwrap_or(true);
    let snap_distance = guideline_props.map(|p| p.snap_distance).unwrap_or(0.15);
    let ruler_interval = guideline_props.map(|p| p.ruler_interval).unwrap_or(0.5);

    commands
        .spawn((
            MapObjectMarker {
                object_id: obj.id.clone(),
                role: ObjectRole::Guideline,
            },
            GuidelineMarker {
                color,
                show_ruler,
                snap_enabled,
                snap_distance,
                ruler_interval,
                start: line_start,
                end: line_end,
            },
            Transform::from_translation(position.extend(0.0))
                .with_rotation(Quat::from_rotation_z(rotation)),
            Visibility::default(),
        ))
        .id()
}
