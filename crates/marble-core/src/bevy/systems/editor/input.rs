//! Input handling systems for the editor.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy::{GameCamera, MainCamera, MapConfig};
use crate::bevy::events::UpdateKeyframeEvent;
use crate::map::{EvaluatedShape, Keyframe, ObjectRole, PivotMode};

use super::{
    EditorStateRes, EditorStateStore, GizmoHandle, ObjectTransform,
    SelectObjectEvent, SnapManager, UpdateObjectEvent,
};

/// Gizmo hit test tolerance (in world units).
const GIZMO_TOLERANCE: f32 = 0.08;
/// Arrow length for world coordinate move gizmos.
const ARROW_LENGTH: f32 = 0.5;
/// Arrow length for local coordinate move gizmos (shorter).
const LOCAL_ARROW_LENGTH: f32 = 0.35;
/// Center handle half-size (the white square is this size on each side from center).
const CENTER_HALF_SIZE: f32 = 0.06;
/// Rotation threshold for showing local gizmos (radians).
const ROTATION_THRESHOLD: f32 = 0.01;

/// System to track mouse position.
pub fn track_mouse_position(
    mut editor_state: ResMut<EditorStateRes>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform, &GameCamera), With<MainCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    editor_state.mouse_screen = Vec2::new(cursor_position.x, cursor_position.y);

    // Convert to world coordinates
    let Ok((camera, camera_transform, _game_camera)) = camera_query.single() else {
        return;
    };

    if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
        editor_state.mouse_world = world_pos;
    }
}

/// System to update hovered gizmo handle.
pub fn update_gizmo_hover(
    mut editor_state: ResMut<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
) {
    // Don't update hover while dragging
    if editor_state.is_dragging {
        return;
    }

    // Skip object gizmo hover when a keyframe is selected (keyframe gizmo takes priority)
    if editor_state.selected_keyframe.is_some() && editor_state.selected_sequence.is_some() {
        return;
    }

    let Some(map_config) = map_config else {
        editor_state.hovered_handle = None;
        return;
    };

    let Some(selected) = editor_state.selected_object else {
        editor_state.hovered_handle = None;
        return;
    };

    let Some(obj) = map_config.0.objects.get(selected) else {
        editor_state.hovered_handle = None;
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let shape = obj.shape.evaluate(&ctx);
    let mouse_pos = editor_state.mouse_world;

    // Use guideline-specific hit test for guidelines
    if obj.role == ObjectRole::Guideline {
        editor_state.hovered_handle = hit_test_guideline_gizmo(&shape, mouse_pos);
    } else {
        editor_state.hovered_handle = hit_test_gizmo(&shape, mouse_pos);
    }
}

/// System to handle mouse clicks for selection.
pub fn handle_mouse_click(
    mut editor_state: ResMut<EditorStateRes>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    map_config: Option<Res<MapConfig>>,
    mut select_events: MessageWriter<SelectObjectEvent>,
) {
    let Some(map_config) = map_config else {
        return;
    };

    // Only handle left click when not dragging
    if !mouse_button.just_pressed(MouseButton::Left) || editor_state.is_dragging {
        return;
    }

    // Skip object gizmo interaction and selection when a keyframe is selected
    // (keyframe mode disables object editing)
    if editor_state.selected_keyframe.is_some() && editor_state.selected_sequence.is_some() {
        return;
    }

    let mouse_pos = editor_state.mouse_world;
    let ctx = crate::dsl::GameContext::new(0.0, 0);

    // Check if clicking on a gizmo handle first
    if let Some(selected) = editor_state.selected_object {
        if let Some(obj) = map_config.0.objects.get(selected) {
            let shape = obj.shape.evaluate(&ctx);
            if let Some(handle) = hit_test_gizmo(&shape, mouse_pos) {
                // Start dragging - save start positions for relative movement
                let transform = get_shape_transform(&shape);
                editor_state.is_dragging = true;
                editor_state.active_handle = Some(handle);
                editor_state.drag_start_mouse = Some(mouse_pos);
                editor_state.drag_start_object_center = Some(transform.center);
                editor_state.drag_start_size = Some(transform.size);
                editor_state.drag_start_rotation = Some(transform.rotation);

                // For rotation, calculate initial angle from center to mouse
                if matches!(handle, GizmoHandle::Rotate) {
                    let angle = (mouse_pos - transform.center).to_angle();
                    editor_state.drag_start_angle = Some(angle);
                }
                return;
            }
        }
    }

    // Check for object selection
    let mut new_selection = None;
    for (idx, obj) in map_config.0.objects.iter().enumerate() {
        let shape = obj.shape.evaluate(&ctx);
        if hit_test_shape(&shape, mouse_pos) {
            new_selection = Some(idx);
            break;
        }
    }

    if new_selection != editor_state.selected_object {
        editor_state.selected_object = new_selection;
        select_events.write(SelectObjectEvent(new_selection));
    }
}

/// System to handle mouse drag with snap support.
pub fn handle_mouse_drag(
    mut editor_state: ResMut<EditorStateRes>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut map_config: Option<ResMut<MapConfig>>,
    snap_manager: SnapManager,
    mut update_events: MessageWriter<UpdateObjectEvent>,
) {
    // End drag on mouse release
    if mouse_button.just_released(MouseButton::Left) && editor_state.is_dragging {
        editor_state.is_dragging = false;
        editor_state.active_handle = None;
        editor_state.drag_start_mouse = None;
        editor_state.drag_start_object_center = None;
        editor_state.drag_start_size = None;
        editor_state.drag_start_rotation = None;
        editor_state.drag_start_angle = None;
        editor_state.snapped_target_index = None;
        return;
    }

    // Continue drag
    if !editor_state.is_dragging || editor_state.active_handle.is_none() {
        return;
    }

    let Some(ref mut map_config) = map_config else {
        return;
    };

    let Some(selected) = editor_state.selected_object else {
        return;
    };

    let handle = editor_state.active_handle.unwrap();
    let mouse_pos = editor_state.mouse_world;
    let drag_start_mouse = editor_state.drag_start_mouse.unwrap_or(mouse_pos);
    let drag_start_center = editor_state.drag_start_object_center.unwrap_or(mouse_pos);

    // Calculate delta from drag start
    let delta = mouse_pos - drag_start_mouse;

    // Get selected object ID for excluding self from snap targets
    let selected_object_id = map_config.0.objects.get(selected).and_then(|o| o.id.clone());

    // Now get mutable reference to the object
    let Some(obj) = map_config.0.objects.get_mut(selected) else {
        return;
    };

    let shift_pressed = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Apply transform based on handle type
    match handle {
        GizmoHandle::MoveFree => {
            let new_center = drag_start_center + delta;
            let result = snap_manager.snap(new_center, shift_pressed, selected_object_id.as_ref());
            obj.shape = move_shape_center(&obj.shape, result.position);
        }
        GizmoHandle::MoveX => {
            let new_center = Vec2::new(drag_start_center.x + delta.x, drag_start_center.y);
            let result = snap_manager.snap(new_center, shift_pressed, selected_object_id.as_ref());
            obj.shape = move_shape_center(&obj.shape, result.position);
        }
        GizmoHandle::MoveY => {
            let new_center = Vec2::new(drag_start_center.x, drag_start_center.y + delta.y);
            let result = snap_manager.snap(new_center, shift_pressed, selected_object_id.as_ref());
            obj.shape = move_shape_center(&obj.shape, result.position);
        }
        GizmoHandle::LocalMoveX => {
            // Move along object's local X axis
            let rotation = editor_state.drag_start_rotation.unwrap_or(0.0);
            let local_x_axis = Rot2::radians(rotation) * Vec2::X;
            // Project delta onto local X axis
            let projected_distance = delta.dot(local_x_axis);
            let new_center = drag_start_center + local_x_axis * projected_distance;
            // Local move: use snap_local with object's rotation, no shift snapping
            let result = snap_manager.snap_local(new_center, drag_start_center, rotation, false, selected_object_id.as_ref());
            obj.shape = move_shape_center(&obj.shape, result.position);
        }
        GizmoHandle::LocalMoveY => {
            // Move along object's local Y axis
            let rotation = editor_state.drag_start_rotation.unwrap_or(0.0);
            let local_y_axis = Rot2::radians(rotation) * Vec2::Y;
            // Project delta onto local Y axis
            let projected_distance = delta.dot(local_y_axis);
            let new_center = drag_start_center + local_y_axis * projected_distance;
            // Local move: use snap_local with object's rotation, no shift snapping
            let result = snap_manager.snap_local(new_center, drag_start_center, rotation, false, selected_object_id.as_ref());
            obj.shape = move_shape_center(&obj.shape, result.position);
        }
        GizmoHandle::LineStart => {
            if let crate::map::Shape::Line { start, .. } = &mut obj.shape {
                // Grid snap only (shift=false)
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *start = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::LineEnd => {
            if let crate::map::Shape::Line { end, .. } = &mut obj.shape {
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *end = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::BezierStart => {
            if let crate::map::Shape::Bezier { start, .. } = &mut obj.shape {
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *start = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::BezierEnd => {
            if let crate::map::Shape::Bezier { end, .. } = &mut obj.shape {
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *end = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::BezierControl1 => {
            if let crate::map::Shape::Bezier { control1, .. } = &mut obj.shape {
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *control1 = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::BezierControl2 => {
            if let crate::map::Shape::Bezier { control2, .. } = &mut obj.shape {
                let result = snap_manager.snap(mouse_pos, false, selected_object_id.as_ref());
                *control2 = crate::dsl::Vec2OrExpr::Static([result.position.x, result.position.y]);
            }
        }
        GizmoHandle::Rotate => {
            // Rotation handle - calculate angle from center to mouse
            if let crate::map::Shape::Rect { rotation, .. } = &mut obj.shape {
                let start_rotation = editor_state.drag_start_rotation.unwrap_or(0.0);
                let start_angle = editor_state.drag_start_angle.unwrap_or(0.0);
                let current_angle = (mouse_pos - drag_start_center).to_angle();
                let angle_delta = current_angle - start_angle;
                let new_rotation_rad = start_rotation + angle_delta;
                let snapped_rotation = snap_manager.snap_angle(new_rotation_rad);

                *rotation = crate::dsl::NumberOrExpr::Number(snapped_rotation.to_degrees());
            }
        }
        GizmoHandle::ScaleTopLeft
        | GizmoHandle::ScaleTopRight
        | GizmoHandle::ScaleBottomLeft
        | GizmoHandle::ScaleBottomRight => {
            // Corner scale handles - uniform scaling from corners
            if let crate::map::Shape::Rect { center, size, rotation, .. } = &mut obj.shape {
                let start_size = editor_state.drag_start_size.unwrap_or(Vec2::ONE);
                let start_rotation_rad = editor_state.drag_start_rotation.unwrap_or(0.0);

                // Transform mouse delta to local space (rotated)
                let rot = Rot2::radians(-start_rotation_rad);
                let local_delta = rot * delta;

                // Determine scale direction based on handle
                let (scale_x_sign, scale_y_sign) = match handle {
                    GizmoHandle::ScaleTopLeft => (-1.0, 1.0),
                    GizmoHandle::ScaleTopRight => (1.0, 1.0),
                    GizmoHandle::ScaleBottomLeft => (-1.0, -1.0),
                    GizmoHandle::ScaleBottomRight => (1.0, -1.0),
                    _ => (0.0, 0.0),
                };

                // Scale by 2x delta because we're scaling from center
                let new_width = snap_manager.snap_scalar(
                    (start_size.x + local_delta.x * scale_x_sign * 2.0).max(0.1),
                );
                let new_height = snap_manager.snap_scalar(
                    (start_size.y + local_delta.y * scale_y_sign * 2.0).max(0.1),
                );

                *center = crate::dsl::Vec2OrExpr::Static([drag_start_center.x, drag_start_center.y]);
                *size = crate::dsl::Vec2OrExpr::Static([new_width, new_height]);
                *rotation = crate::dsl::NumberOrExpr::Number(start_rotation_rad.to_degrees());
            }
        }
        GizmoHandle::ScaleTop | GizmoHandle::ScaleBottom => {
            // Vertical edge scale handles
            if let crate::map::Shape::Rect { center, size, rotation, .. } = &mut obj.shape {
                let start_size = editor_state.drag_start_size.unwrap_or(Vec2::ONE);
                let start_rotation_rad = editor_state.drag_start_rotation.unwrap_or(0.0);

                let rot = Rot2::radians(-start_rotation_rad);
                let local_delta = rot * delta;

                let scale_y_sign = if matches!(handle, GizmoHandle::ScaleTop) {
                    1.0
                } else {
                    -1.0
                };

                let new_height = snap_manager.snap_scalar(
                    (start_size.y + local_delta.y * scale_y_sign * 2.0).max(0.1),
                );

                *center = crate::dsl::Vec2OrExpr::Static([drag_start_center.x, drag_start_center.y]);
                *size = crate::dsl::Vec2OrExpr::Static([start_size.x, new_height]);
                *rotation = crate::dsl::NumberOrExpr::Number(start_rotation_rad.to_degrees());
            }
        }
        GizmoHandle::ScaleLeft | GizmoHandle::ScaleRight => {
            // Horizontal edge scale handles
            if let crate::map::Shape::Rect { center, size, rotation, .. } = &mut obj.shape {
                let start_size = editor_state.drag_start_size.unwrap_or(Vec2::ONE);
                let start_rotation_rad = editor_state.drag_start_rotation.unwrap_or(0.0);

                let rot = Rot2::radians(-start_rotation_rad);
                let local_delta = rot * delta;

                let scale_x_sign = if matches!(handle, GizmoHandle::ScaleRight) {
                    1.0
                } else {
                    -1.0
                };

                let new_width = snap_manager.snap_scalar(
                    (start_size.x + local_delta.x * scale_x_sign * 2.0).max(0.1),
                );

                *center = crate::dsl::Vec2OrExpr::Static([drag_start_center.x, drag_start_center.y]);
                *size = crate::dsl::Vec2OrExpr::Static([new_width, start_size.y]);
                *rotation = crate::dsl::NumberOrExpr::Number(start_rotation_rad.to_degrees());
            }
        }
        GizmoHandle::RadiusTop | GizmoHandle::RadiusBottom => {
            // Radius handles (vertical) - use Y distance from center
            if let crate::map::Shape::Circle { center, radius } = &mut obj.shape {
                let raw_radius = (mouse_pos.y - drag_start_center.y).abs().max(0.05);
                let new_radius = snap_manager.snap_scalar(raw_radius);
                *center = crate::dsl::Vec2OrExpr::Static([drag_start_center.x, drag_start_center.y]);
                *radius = crate::dsl::NumberOrExpr::Number(new_radius);
            }
        }
        GizmoHandle::RadiusLeft | GizmoHandle::RadiusRight => {
            // Radius handles (horizontal) - use X distance from center
            if let crate::map::Shape::Circle { center, radius } = &mut obj.shape {
                let raw_radius = (mouse_pos.x - drag_start_center.x).abs().max(0.05);
                let new_radius = snap_manager.snap_scalar(raw_radius);
                *center = crate::dsl::Vec2OrExpr::Static([drag_start_center.x, drag_start_center.y]);
                *radius = crate::dsl::NumberOrExpr::Number(new_radius);
            }
        }
        GizmoHandle::Pivot => {
            // Pivot handle - not yet implemented
        }
        // Guideline handles
        GizmoHandle::GuidelineMove => {
            // Move entire guideline by delta
            if let crate::map::Shape::Line { start, end } = &mut obj.shape {
                let ctx = crate::dsl::GameContext::new(0.0, 0);
                let start_val = start.evaluate(&ctx);
                let end_val = end.evaluate(&ctx);
                let old_center = Vec2::new(
                    (start_val[0] + end_val[0]) / 2.0,
                    (start_val[1] + end_val[1]) / 2.0,
                );
                let new_center = drag_start_center + delta;
                let move_delta = new_center - old_center;

                *start = crate::dsl::Vec2OrExpr::Static([
                    start_val[0] + move_delta.x + (drag_start_center.x - old_center.x),
                    start_val[1] + move_delta.y + (drag_start_center.y - old_center.y),
                ]);
                *end = crate::dsl::Vec2OrExpr::Static([
                    end_val[0] + move_delta.x + (drag_start_center.x - old_center.x),
                    end_val[1] + move_delta.y + (drag_start_center.y - old_center.y),
                ]);
            }
        }
        GizmoHandle::GuidelineStart => {
            if let crate::map::Shape::Line { start, .. } = &mut obj.shape {
                *start = crate::dsl::Vec2OrExpr::Static([mouse_pos.x, mouse_pos.y]);
            }
        }
        GizmoHandle::GuidelineEnd => {
            if let crate::map::Shape::Line { end, .. } = &mut obj.shape {
                *end = crate::dsl::Vec2OrExpr::Static([mouse_pos.x, mouse_pos.y]);
            }
        }
        // Keyframe handles are handled by handle_keyframe_drag system
        GizmoHandle::KeyframePivot |
        GizmoHandle::KeyframeAngle |
        GizmoHandle::KeyframeTranslateX |
        GizmoHandle::KeyframeTranslateY |
        GizmoHandle::KeyframeTranslateFree => {
            return; // Handled by separate system
        }
    }

    // Emit update event
    update_events.write(UpdateObjectEvent {
        index: selected,
        object: obj.clone(),
    });
}

/// System to sync editor state from store (Yew -> Bevy).
pub fn sync_editor_state_from_store(
    mut editor_state: ResMut<EditorStateRes>,
    editor_store: Res<EditorStateStore>,
    mut select_events: MessageWriter<SelectObjectEvent>,
) {
    // Handle pending selection
    if let Some(selection) = editor_store.take_pending_selection() {
        if selection != editor_state.selected_object {
            editor_state.selected_object = selection;
            select_events.write(SelectObjectEvent(selection));
        }
    }

    // Sync simulation state
    editor_state.is_simulating = editor_store.is_simulating();
}

/// System to sync editor state to store (Bevy -> Yew).
pub fn sync_editor_state_to_store(editor_state: Res<EditorStateRes>, editor_store: Res<EditorStateStore>) {
    editor_store.sync_from_bevy(&editor_state);
}

/// Calculate distance from a point to a line segment.
fn point_to_segment_distance(point: Vec2, seg_start: Vec2, seg_end: Vec2) -> f32 {
    let line = seg_end - seg_start;
    let len_sq = line.length_squared();
    if len_sq < 0.0001 {
        return point.distance(seg_start);
    }
    let t = ((point - seg_start).dot(line) / len_sq).clamp(0.0, 1.0);
    let closest = seg_start + t * line;
    point.distance(closest)
}

/// Check if a point is inside a rectangle (axis-aligned).
fn point_in_rect(point: Vec2, center: Vec2, half_size: f32) -> bool {
    let local = point - center;
    local.x.abs() <= half_size && local.y.abs() <= half_size
}

/// Hit test a gizmo and return which handle was hit.
fn hit_test_gizmo(shape: &EvaluatedShape, point: Vec2) -> Option<GizmoHandle> {
    match shape {
        EvaluatedShape::Circle { center, radius } => {
            let pos = Vec2::new(center[0], center[1]);

            // Center (free move) - check FIRST, entire square area is clickable
            if point_in_rect(point, pos, CENTER_HALF_SIZE) {
                return Some(GizmoHandle::MoveFree);
            }

            // Radius handles (4 cardinal directions)
            let radius_handles = [
                (pos + Vec2::new(0.0, *radius), GizmoHandle::RadiusTop),
                (pos + Vec2::new(0.0, -*radius), GizmoHandle::RadiusBottom),
                (pos + Vec2::new(-*radius, 0.0), GizmoHandle::RadiusLeft),
                (pos + Vec2::new(*radius, 0.0), GizmoHandle::RadiusRight),
            ];

            for (handle_pos, handle) in radius_handles {
                if point.distance(handle_pos) < GIZMO_TOLERANCE {
                    return Some(handle);
                }
            }

            // X arrow (entire line)
            let x_end = pos + Vec2::new(ARROW_LENGTH, 0.0);
            if point_to_segment_distance(point, pos, x_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveX);
            }

            // Y arrow (entire line)
            let y_end = pos + Vec2::new(0.0, ARROW_LENGTH);
            if point_to_segment_distance(point, pos, y_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveY);
            }

            None
        }
        EvaluatedShape::Rect { center, size, rotation } => {
            let pos = Vec2::new(center[0], center[1]);
            let rotation_rad = rotation.to_radians();
            let rot = Rot2::radians(rotation_rad);

            // Center (free move) - check FIRST, entire square area is clickable
            if point_in_rect(point, pos, CENTER_HALF_SIZE) {
                return Some(GizmoHandle::MoveFree);
            }

            // Local coordinate gizmos (check first, only when rotated)
            if rotation_rad.abs() > ROTATION_THRESHOLD {
                // Local X arrow (rotated, shorter)
                let local_x_end = pos + rot * Vec2::new(LOCAL_ARROW_LENGTH, 0.0);
                if point_to_segment_distance(point, pos, local_x_end) < GIZMO_TOLERANCE {
                    return Some(GizmoHandle::LocalMoveX);
                }

                // Local Y arrow (rotated, shorter)
                let local_y_end = pos + rot * Vec2::new(0.0, LOCAL_ARROW_LENGTH);
                if point_to_segment_distance(point, pos, local_y_end) < GIZMO_TOLERANCE {
                    return Some(GizmoHandle::LocalMoveY);
                }
            }

            // World coordinate X arrow (not rotated)
            let x_end = pos + Vec2::new(ARROW_LENGTH, 0.0);
            if point_to_segment_distance(point, pos, x_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveX);
            }

            // World coordinate Y arrow (not rotated)
            let y_end = pos + Vec2::new(0.0, ARROW_LENGTH);
            if point_to_segment_distance(point, pos, y_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveY);
            }

            // Scale corners
            let half = Vec2::new(size[0], size[1]) / 2.0;
            let corners = [
                (pos + rot * Vec2::new(-half.x, half.y), GizmoHandle::ScaleTopLeft),
                (pos + rot * Vec2::new(half.x, half.y), GizmoHandle::ScaleTopRight),
                (pos + rot * Vec2::new(-half.x, -half.y), GizmoHandle::ScaleBottomLeft),
                (pos + rot * Vec2::new(half.x, -half.y), GizmoHandle::ScaleBottomRight),
            ];

            for (corner, handle) in corners {
                if point.distance(corner) < GIZMO_TOLERANCE {
                    return Some(handle);
                }
            }

            // Rotation ring
            let rotate_radius = Vec2::new(size[0], size[1]).max_element() / 2.0 + 0.2;
            let dist_from_center = point.distance(pos);
            if (dist_from_center - rotate_radius).abs() < GIZMO_TOLERANCE {
                return Some(GizmoHandle::Rotate);
            }

            None
        }
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            let line_center = (start_pos + end_pos) / 2.0;

            // Endpoints (highest priority)
            if point.distance(start_pos) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::LineStart);
            }
            if point.distance(end_pos) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::LineEnd);
            }

            // Center (free move) - check before arrows, entire square area is clickable
            if point_in_rect(point, line_center, CENTER_HALF_SIZE) {
                return Some(GizmoHandle::MoveFree);
            }

            // X arrow from center
            let x_end = line_center + Vec2::new(ARROW_LENGTH, 0.0);
            if point_to_segment_distance(point, line_center, x_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveX);
            }

            // Y arrow from center
            let y_end = line_center + Vec2::new(0.0, ARROW_LENGTH);
            if point_to_segment_distance(point, line_center, y_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveY);
            }

            None
        }
        EvaluatedShape::Bezier { start, control1, control2, end, .. } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let ctrl1 = Vec2::new(control1[0], control1[1]);
            let ctrl2 = Vec2::new(control2[0], control2[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            let bezier_center = (start_pos + end_pos) / 2.0;

            // Control points and endpoints (highest priority)
            if point.distance(start_pos) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::BezierStart);
            }
            if point.distance(ctrl1) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::BezierControl1);
            }
            if point.distance(ctrl2) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::BezierControl2);
            }
            if point.distance(end_pos) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::BezierEnd);
            }

            // Center (free move) - check before arrows, entire square area is clickable
            if point_in_rect(point, bezier_center, CENTER_HALF_SIZE) {
                return Some(GizmoHandle::MoveFree);
            }

            // X arrow from center
            let x_end = bezier_center + Vec2::new(ARROW_LENGTH, 0.0);
            if point_to_segment_distance(point, bezier_center, x_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveX);
            }

            // Y arrow from center
            let y_end = bezier_center + Vec2::new(0.0, ARROW_LENGTH);
            if point_to_segment_distance(point, bezier_center, y_end) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::MoveY);
            }

            None
        }
    }
}

/// Hit test if a point is inside a shape.
fn hit_test_shape(shape: &EvaluatedShape, point: Vec2) -> bool {
    match shape {
        EvaluatedShape::Circle { center, radius } => {
            let pos = Vec2::new(center[0], center[1]);
            point.distance(pos) <= *radius
        }
        EvaluatedShape::Rect { center, size, rotation } => {
            let pos = Vec2::new(center[0], center[1]);
            let half = Vec2::new(size[0], size[1]) / 2.0;
            let rot = Rot2::radians(-rotation.to_radians());
            let local = rot * (point - pos);
            local.x.abs() <= half.x && local.y.abs() <= half.y
        }
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            point_to_segment_distance(point, start_pos, end_pos) < 0.1
        }
        EvaluatedShape::Bezier { start, control1, control2, end, .. } => {
            let points = super::gizmo::bezier_to_points(
                &[start[0], start[1]],
                &[control1[0], control1[1]],
                &[control2[0], control2[1]],
                &[end[0], end[1]],
                20,
            );
            for i in 0..points.len() - 1 {
                if point_to_segment_distance(point, points[i], points[i + 1]) < 0.1 {
                    return true;
                }
            }
            false
        }
    }
}

/// Get the transform (center, size, rotation) of a shape.
fn get_shape_transform(shape: &EvaluatedShape) -> ObjectTransform {
    match shape {
        EvaluatedShape::Circle { center, radius } => ObjectTransform {
            center: Vec2::new(center[0], center[1]),
            size: Vec2::new(*radius * 2.0, *radius * 2.0),
            rotation: 0.0,
        },
        EvaluatedShape::Rect { center, size, rotation } => ObjectTransform {
            center: Vec2::new(center[0], center[1]),
            size: Vec2::new(size[0], size[1]),
            rotation: rotation.to_radians(),
        },
        EvaluatedShape::Line { start, end } => {
            let s = Vec2::new(start[0], start[1]);
            let e = Vec2::new(end[0], end[1]);
            let diff = e - s;
            ObjectTransform {
                center: (s + e) / 2.0,
                size: Vec2::new(diff.length(), 0.04), // Line thickness
                rotation: diff.to_angle(),
            }
        }
        EvaluatedShape::Bezier { start, end, .. } => {
            let s = Vec2::new(start[0], start[1]);
            let e = Vec2::new(end[0], end[1]);
            ObjectTransform {
                center: (s + e) / 2.0,
                size: Vec2::ZERO,
                rotation: 0.0,
            }
        }
    }
}

/// Move a shape's center to a new position.
fn move_shape_center(shape: &crate::map::Shape, new_center: Vec2) -> crate::map::Shape {
    use crate::dsl::Vec2OrExpr;
    use crate::map::Shape;

    let ctx = crate::dsl::GameContext::new(0.0, 0);

    match shape.clone() {
        Shape::Circle { radius, .. } => Shape::Circle {
            center: Vec2OrExpr::Static([new_center.x, new_center.y]),
            radius,
        },
        Shape::Rect { size, rotation, .. } => Shape::Rect {
            center: Vec2OrExpr::Static([new_center.x, new_center.y]),
            size,
            rotation,
        },
        Shape::Line { start, end } => {
            let start_val = start.evaluate(&ctx);
            let end_val = end.evaluate(&ctx);
            let old_center = Vec2::new(
                (start_val[0] + end_val[0]) / 2.0,
                (start_val[1] + end_val[1]) / 2.0,
            );
            let delta = new_center - old_center;
            Shape::Line {
                start: Vec2OrExpr::Static([start_val[0] + delta.x, start_val[1] + delta.y]),
                end: Vec2OrExpr::Static([end_val[0] + delta.x, end_val[1] + delta.y]),
            }
        }
        Shape::Bezier {
            start,
            control1,
            control2,
            end,
            segments,
        } => {
            let start_val = start.evaluate(&ctx);
            let end_val = end.evaluate(&ctx);
            let ctrl1_val = control1.evaluate(&ctx);
            let ctrl2_val = control2.evaluate(&ctx);
            let old_center = Vec2::new(
                (start_val[0] + end_val[0]) / 2.0,
                (start_val[1] + end_val[1]) / 2.0,
            );
            let delta = new_center - old_center;
            Shape::Bezier {
                start: Vec2OrExpr::Static([start_val[0] + delta.x, start_val[1] + delta.y]),
                control1: Vec2OrExpr::Static([ctrl1_val[0] + delta.x, ctrl1_val[1] + delta.y]),
                control2: Vec2OrExpr::Static([ctrl2_val[0] + delta.x, ctrl2_val[1] + delta.y]),
                end: Vec2OrExpr::Static([end_val[0] + delta.x, end_val[1] + delta.y]),
                segments,
            }
        }
    }
}

// ========== Keyframe Gizmo Interaction ==========

/// Keyframe gizmo hit test tolerance.
const KEYFRAME_GIZMO_TOLERANCE: f32 = 0.1;
/// Arrow length for keyframe translation gizmos.
const KEYFRAME_ARROW_LENGTH: f32 = 0.5;
/// Center handle half-size for keyframe gizmos.
const KEYFRAME_CENTER_HALF_SIZE: f32 = 0.06;

/// System to update hovered keyframe gizmo handle.
pub fn update_keyframe_gizmo_hover(
    mut editor_state: ResMut<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
) {
    // Don't update hover while dragging
    if editor_state.is_dragging {
        return;
    }

    // Only check keyframe gizmos if we have a selected keyframe
    let Some(seq_idx) = editor_state.selected_sequence else {
        return;
    };

    let Some(kf_idx) = editor_state.selected_keyframe else {
        return;
    };

    let Some(map_config) = map_config else {
        return;
    };

    let Some(sequence) = map_config.0.keyframes.get(seq_idx) else {
        return;
    };

    let Some(keyframe) = sequence.keyframes.get(kf_idx) else {
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let mouse_pos = editor_state.mouse_world;

    // Get target center
    let target_centers: Vec<Vec2> = sequence
        .target_ids
        .iter()
        .filter_map(|target_id| {
            map_config.0.objects.iter()
                .find(|o| o.id.as_ref() == Some(target_id))
                .map(|obj| {
                    let shape = obj.shape.evaluate(&ctx);
                    get_shape_center_for_hit_test(&shape)
                })
        })
        .collect();

    if target_centers.is_empty() {
        return;
    }

    let avg_center = target_centers.iter().fold(Vec2::ZERO, |acc, c| acc + *c) / target_centers.len() as f32;

    // Try to hit test keyframe gizmo
    if let Some(handle) = hit_test_keyframe_gizmo(keyframe, avg_center, mouse_pos) {
        editor_state.hovered_handle = Some(handle);
    }
}

/// Get the center of an evaluated shape for hit testing.
fn get_shape_center_for_hit_test(shape: &EvaluatedShape) -> Vec2 {
    match shape {
        EvaluatedShape::Circle { center, .. } => Vec2::new(center[0], center[1]),
        EvaluatedShape::Rect { center, .. } => Vec2::new(center[0], center[1]),
        EvaluatedShape::Line { start, end } => {
            Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0)
        }
        EvaluatedShape::Bezier { start, end, .. } => {
            Vec2::new((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0)
        }
    }
}

/// Hit test keyframe gizmo and return which handle was hit.
fn hit_test_keyframe_gizmo(
    keyframe: &Keyframe,
    target_center: Vec2,
    point: Vec2,
) -> Option<GizmoHandle> {
    match keyframe {
        Keyframe::PivotRotate { pivot, pivot_mode, angle, .. } => {
            hit_test_pivot_rotate_gizmo(*pivot, *pivot_mode, *angle, target_center, point)
        }
        Keyframe::Apply { translation, rotation, .. } => {
            hit_test_apply_gizmo(*translation, *rotation, target_center, point)
        }
        // ContinuousRotate, LoopStart, LoopEnd, Delay don't have interactive gizmos
        _ => None,
    }
}

/// Hit test PivotRotate gizmo.
fn hit_test_pivot_rotate_gizmo(
    pivot: [f32; 2],
    pivot_mode: PivotMode,
    angle: f32,
    target_center: Vec2,
    point: Vec2,
) -> Option<GizmoHandle> {
    // Calculate world pivot position based on pivot mode
    let pivot_pos = match pivot_mode {
        PivotMode::Absolute => Vec2::new(pivot[0], pivot[1]),
        PivotMode::Relative => target_center + Vec2::new(pivot[0], pivot[1]),
    };

    // Pivot marker (diamond) - check first
    if point.distance(pivot_pos) < KEYFRAME_GIZMO_TOLERANCE * 1.5 {
        return Some(GizmoHandle::KeyframePivot);
    }

    // Rotation arc - check distance from pivot and if within angle range
    let arc_radius = pivot_pos.distance(target_center).max(0.3);
    let dist_from_pivot = point.distance(pivot_pos);

    if (dist_from_pivot - arc_radius).abs() < KEYFRAME_GIZMO_TOLERANCE {
        // Check if point is within the angle range
        let point_angle = (point - pivot_pos).to_angle();
        let angle_rad = angle.to_radians();

        // Normalize angles to [0, 2π]
        let normalized_point = normalize_angle(point_angle);
        let normalized_end = normalize_angle(angle_rad);

        let min_angle = 0.0f32.min(normalized_end);
        let max_angle = 0.0f32.max(normalized_end);

        if normalized_point >= min_angle - 0.2 && normalized_point <= max_angle + 0.2 {
            return Some(GizmoHandle::KeyframeAngle);
        }
    }

    None
}

/// Hit test Apply gizmo.
fn hit_test_apply_gizmo(
    translation: Option<[f32; 2]>,
    rotation: Option<f32>,
    target_center: Vec2,
    point: Vec2,
) -> Option<GizmoHandle> {
    let trans = translation.unwrap_or([0.0, 0.0]);

    // Center (free translate) - check FIRST
    if point_in_rect_for_kf(point, target_center, KEYFRAME_CENTER_HALF_SIZE) {
        return Some(GizmoHandle::KeyframeTranslateFree);
    }

    // X arrow - direction follows translation sign
    let x_dir = if trans[0] >= 0.0 { 1.0 } else { -1.0 };
    let x_end = target_center + Vec2::new(KEYFRAME_ARROW_LENGTH * x_dir, 0.0);
    if point_to_segment_distance_kf(point, target_center, x_end) < KEYFRAME_GIZMO_TOLERANCE {
        return Some(GizmoHandle::KeyframeTranslateX);
    }

    // Y arrow - direction follows translation sign
    let y_dir = if trans[1] >= 0.0 { 1.0 } else { -1.0 };
    let y_end = target_center + Vec2::new(0.0, KEYFRAME_ARROW_LENGTH * y_dir);
    if point_to_segment_distance_kf(point, target_center, y_end) < KEYFRAME_GIZMO_TOLERANCE {
        return Some(GizmoHandle::KeyframeTranslateY);
    }

    // Rotation arc (if rotation is set)
    if let Some(rot_deg) = rotation {
        if rot_deg.abs() > 0.001 {
            let arc_radius = 0.4;
            let dist_from_center = point.distance(target_center);

            if (dist_from_center - arc_radius).abs() < KEYFRAME_GIZMO_TOLERANCE {
                return Some(GizmoHandle::KeyframeAngle);
            }
        }
    }

    None
}

/// Check if a point is inside a rectangle (axis-aligned).
fn point_in_rect_for_kf(point: Vec2, center: Vec2, half_size: f32) -> bool {
    let local = point - center;
    local.x.abs() <= half_size && local.y.abs() <= half_size
}

/// Calculate distance from a point to a line segment.
fn point_to_segment_distance_kf(point: Vec2, seg_start: Vec2, seg_end: Vec2) -> f32 {
    let line = seg_end - seg_start;
    let len_sq = line.length_squared();
    if len_sq < 0.0001 {
        return point.distance(seg_start);
    }
    let t = ((point - seg_start).dot(line) / len_sq).clamp(0.0, 1.0);
    let closest = seg_start + t * line;
    point.distance(closest)
}

/// Normalize angle to [0, 2π].
fn normalize_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::PI * 2.0;
    let mut a = angle % two_pi;
    if a < 0.0 {
        a += two_pi;
    }
    a
}

/// System to handle keyframe gizmo click (start drag).
pub fn handle_keyframe_gizmo_click(
    mut editor_state: ResMut<EditorStateRes>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    map_config: Option<Res<MapConfig>>,
) {
    // Only handle left click when not already dragging
    if !mouse_button.just_pressed(MouseButton::Left) || editor_state.is_dragging {
        return;
    }

    // Need sequence and keyframe selected
    let Some(seq_idx) = editor_state.selected_sequence else {
        return;
    };

    let Some(kf_idx) = editor_state.selected_keyframe else {
        return;
    };

    let Some(map_config) = map_config else {
        return;
    };

    let Some(sequence) = map_config.0.keyframes.get(seq_idx) else {
        return;
    };

    let Some(keyframe) = sequence.keyframes.get(kf_idx) else {
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let mouse_pos = editor_state.mouse_world;

    // Get target center
    let target_centers: Vec<Vec2> = sequence
        .target_ids
        .iter()
        .filter_map(|target_id| {
            map_config.0.objects.iter()
                .find(|o| o.id.as_ref() == Some(target_id))
                .map(|obj| {
                    let shape = obj.shape.evaluate(&ctx);
                    get_shape_center_for_hit_test(&shape)
                })
        })
        .collect();

    if target_centers.is_empty() {
        return;
    }

    let avg_center = target_centers.iter().fold(Vec2::ZERO, |acc, c| acc + *c) / target_centers.len() as f32;

    // Hit test keyframe gizmo
    if let Some(handle) = hit_test_keyframe_gizmo(keyframe, avg_center, mouse_pos) {
        // Start dragging
        editor_state.is_dragging = true;
        editor_state.active_handle = Some(handle);
        editor_state.drag_start_mouse = Some(mouse_pos);

        // Save start values based on keyframe type
        match keyframe {
            Keyframe::PivotRotate { pivot, angle, .. } => {
                editor_state.drag_start_keyframe_pivot = Some(*pivot);
                editor_state.drag_start_keyframe_angle = Some(*angle);
                editor_state.drag_start_object_center = Some(avg_center);
            }
            Keyframe::Apply { translation, rotation, .. } => {
                editor_state.drag_start_keyframe_translation = Some(translation.unwrap_or([0.0, 0.0]));
                editor_state.drag_start_keyframe_angle = *rotation;
                editor_state.drag_start_object_center = Some(avg_center);
            }
            _ => {}
        }
    }
}

/// System to handle keyframe gizmo drag.
pub fn handle_keyframe_drag(
    mut editor_state: ResMut<EditorStateRes>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut map_config: Option<ResMut<MapConfig>>,
    snap_manager: SnapManager,
    mut update_events: MessageWriter<UpdateKeyframeEvent>,
) {
    // End drag on mouse release
    if mouse_button.just_released(MouseButton::Left) && editor_state.is_dragging {
        // Check if we were dragging a keyframe handle
        if let Some(handle) = editor_state.active_handle {
            if matches!(handle,
                GizmoHandle::KeyframePivot |
                GizmoHandle::KeyframeAngle |
                GizmoHandle::KeyframeTranslateX |
                GizmoHandle::KeyframeTranslateY |
                GizmoHandle::KeyframeTranslateFree
            ) {
                // Clear keyframe-specific drag state
                editor_state.drag_start_keyframe_pivot = None;
                editor_state.drag_start_keyframe_angle = None;
                editor_state.drag_start_keyframe_translation = None;
            }
        }
        return;
    }

    // Check if we're dragging a keyframe handle
    let Some(handle) = editor_state.active_handle else {
        return;
    };

    if !matches!(handle,
        GizmoHandle::KeyframePivot |
        GizmoHandle::KeyframeAngle |
        GizmoHandle::KeyframeTranslateX |
        GizmoHandle::KeyframeTranslateY |
        GizmoHandle::KeyframeTranslateFree
    ) {
        return;
    }

    if !editor_state.is_dragging {
        return;
    }

    let Some(seq_idx) = editor_state.selected_sequence else {
        return;
    };

    let Some(kf_idx) = editor_state.selected_keyframe else {
        return;
    };

    let Some(ref mut map_config) = map_config else {
        return;
    };

    let Some(sequence) = map_config.0.keyframes.get_mut(seq_idx) else {
        return;
    };

    let Some(keyframe) = sequence.keyframes.get_mut(kf_idx) else {
        return;
    };

    let mouse_pos = editor_state.mouse_world;
    let drag_start_mouse = editor_state.drag_start_mouse.unwrap_or(mouse_pos);
    let delta = mouse_pos - drag_start_mouse;
    let target_center = editor_state.drag_start_object_center.unwrap_or(Vec2::ZERO);

    // Apply changes based on handle type
    let updated_keyframe = match (handle, keyframe.clone()) {
        (GizmoHandle::KeyframePivot, Keyframe::PivotRotate { pivot_mode, angle, duration, easing, .. }) => {
            // Move pivot to mouse position with grid snap
            // For Relative mode, convert world position to relative offset
            let result = snap_manager.snap(mouse_pos, false, None);
            let new_pivot = match pivot_mode {
                PivotMode::Absolute => [result.position.x, result.position.y],
                PivotMode::Relative => {
                    // Convert world position to offset from target center
                    [result.position.x - target_center.x, result.position.y - target_center.y]
                }
            };
            Some(Keyframe::PivotRotate {
                pivot: new_pivot,
                pivot_mode,
                angle,
                duration,
                easing,
            })
        }
        (GizmoHandle::KeyframeAngle, Keyframe::PivotRotate { pivot, pivot_mode, duration, easing, .. }) => {
            // Calculate world pivot position based on mode
            let pivot_pos = match pivot_mode {
                PivotMode::Absolute => Vec2::new(pivot[0], pivot[1]),
                PivotMode::Relative => target_center + Vec2::new(pivot[0], pivot[1]),
            };
            // Calculate angle from pivot to mouse with angle snap
            let angle_rad = (mouse_pos - pivot_pos).to_angle();
            let start_angle = editor_state.drag_start_keyframe_angle.unwrap_or(0.0);
            let start_angle_rad = (drag_start_mouse - pivot_pos).to_angle();
            let delta_angle = angle_rad - start_angle_rad;
            let new_angle_rad = (start_angle + delta_angle.to_degrees()).to_radians();
            let snapped_angle = snap_manager.snap_angle(new_angle_rad);

            Some(Keyframe::PivotRotate {
                pivot,
                pivot_mode,
                angle: snapped_angle.to_degrees(),
                duration,
                easing,
            })
        }
        (GizmoHandle::KeyframeTranslateX, Keyframe::Apply { rotation, duration, easing, .. }) => {
            let start_trans = editor_state.drag_start_keyframe_translation.unwrap_or([0.0, 0.0]);
            let new_x = snap_manager.snap_scalar(start_trans[0] + delta.x);
            let y = start_trans[1];
            let new_translation = if new_x.abs() < 0.001 && y.abs() < 0.001 {
                None
            } else {
                Some([new_x, y])
            };

            Some(Keyframe::Apply {
                translation: new_translation,
                rotation,
                duration,
                easing,
            })
        }
        (GizmoHandle::KeyframeTranslateY, Keyframe::Apply { rotation, duration, easing, .. }) => {
            let start_trans = editor_state.drag_start_keyframe_translation.unwrap_or([0.0, 0.0]);
            let x = start_trans[0];
            let new_y = snap_manager.snap_scalar(start_trans[1] + delta.y);
            let new_translation = if x.abs() < 0.001 && new_y.abs() < 0.001 {
                None
            } else {
                Some([x, new_y])
            };

            Some(Keyframe::Apply {
                translation: new_translation,
                rotation,
                duration,
                easing,
            })
        }
        (GizmoHandle::KeyframeTranslateFree, Keyframe::Apply { rotation, duration, easing, .. }) => {
            let start_trans = editor_state.drag_start_keyframe_translation.unwrap_or([0.0, 0.0]);
            let raw_pos = Vec2::new(start_trans[0] + delta.x, start_trans[1] + delta.y);
            let result = snap_manager.snap(raw_pos, false, None);
            let new_translation = if result.position.x.abs() < 0.001 && result.position.y.abs() < 0.001 {
                None
            } else {
                Some([result.position.x, result.position.y])
            };

            Some(Keyframe::Apply {
                translation: new_translation,
                rotation,
                duration,
                easing,
            })
        }
        (GizmoHandle::KeyframeAngle, Keyframe::Apply { translation, duration, easing, .. }) => {
            // Rotation handle for Apply with angle snap
            let angle_rad = (mouse_pos - target_center).to_angle();
            let start_angle = editor_state.drag_start_keyframe_angle.unwrap_or(0.0);
            let start_angle_rad = (drag_start_mouse - target_center).to_angle();
            let delta_angle = angle_rad - start_angle_rad;
            let new_rotation_rad = (start_angle + delta_angle.to_degrees()).to_radians();
            let snapped_rotation = snap_manager.snap_angle(new_rotation_rad);
            let new_rotation_deg = snapped_rotation.to_degrees();
            let new_rotation = if new_rotation_deg.abs() < 0.001 { None } else { Some(new_rotation_deg) };

            Some(Keyframe::Apply {
                translation,
                rotation: new_rotation,
                duration,
                easing,
            })
        }
        _ => None,
    };

    if let Some(new_keyframe) = updated_keyframe {
        *keyframe = new_keyframe.clone();

        // Emit update event
        update_events.write(UpdateKeyframeEvent {
            sequence_index: seq_idx,
            keyframe_index: kf_idx,
            keyframe: new_keyframe,
        });
    }
}

// ========== Guideline Gizmo Hit Testing ==========

/// Guideline gizmo tolerance.
const GUIDELINE_GIZMO_TOLERANCE: f32 = 0.1;

/// Hit test guideline gizmo and return which handle was hit.
fn hit_test_guideline_gizmo(shape: &EvaluatedShape, point: Vec2) -> Option<GizmoHandle> {
    match shape {
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            let line_center = (start_pos + end_pos) / 2.0;

            // Center move handle (square) - highest priority
            if point_in_rect(point, line_center, GUIDELINE_GIZMO_TOLERANCE) {
                return Some(GizmoHandle::GuidelineMove);
            }

            // Start endpoint (diamond)
            if point.distance(start_pos) < GUIDELINE_GIZMO_TOLERANCE {
                return Some(GizmoHandle::GuidelineStart);
            }

            // End endpoint (diamond)
            if point.distance(end_pos) < GUIDELINE_GIZMO_TOLERANCE {
                return Some(GizmoHandle::GuidelineEnd);
            }

            // Line body (for selection, not dragging)
            if point_to_segment_distance(point, start_pos, end_pos) < GIZMO_TOLERANCE {
                return Some(GizmoHandle::GuidelineMove);
            }

            None
        }
        // Non-line guidelines use standard gizmo handles
        _ => None,
    }
}

