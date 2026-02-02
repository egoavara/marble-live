//! Gizmo rendering systems for the editor.

use bevy::prelude::*;

use crate::bevy::{GameCamera, GuidelineMarker, MainCamera, MapConfig, MapObjectMarker};
use crate::map::{EvaluatedShape, Keyframe, ObjectRole, PivotMode};

use super::{
    calculate_distance_lines, EditorStateRes, GizmoColors, GizmoHandle, LineSnapTarget,
    SnapConfig, SnapTarget,
};

/// System to render selection highlight and gizmos.
pub fn render_editor_gizmos(
    mut gizmos: Gizmos,
    editor_state: Res<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
    camera_query: Query<(&GameCamera, &GlobalTransform), With<MainCamera>>,
) {
    let Some(map_config) = map_config else {
        return;
    };

    // Skip object gizmo when a keyframe is selected (keyframe gizmo takes priority)
    if editor_state.selected_keyframe.is_some() && editor_state.selected_sequence.is_some() {
        return;
    }

    let Some(selected_idx) = editor_state.selected_object else {
        return;
    };

    let Some(obj) = map_config.0.objects.get(selected_idx) else {
        return;
    };

    // Get camera for zoom-independent sizing
    let zoom = camera_query
        .single()
        .map(|(cam, _)| cam.zoom)
        .unwrap_or(100.0);

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let shape = obj.shape.evaluate(&ctx);

    // Get hovered handle for highlighting
    let hovered = editor_state.hovered_handle;
    let active = editor_state.active_handle;

    // Draw selection highlight
    draw_selection_highlight(&mut gizmos, &shape);

    // Draw gizmo based on shape type
    match &shape {
        EvaluatedShape::Circle { center, radius } => {
            let pos = Vec2::new(center[0], center[1]);
            draw_circle_gizmo(&mut gizmos, pos, *radius, zoom, hovered, active);
        }
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let pos = Vec2::new(center[0], center[1]);
            let sz = Vec2::new(size[0], size[1]);
            draw_standard_gizmo(&mut gizmos, pos, sz, rotation.to_radians(), zoom, hovered, active);
        }
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            draw_line_gizmo(&mut gizmos, start_pos, end_pos, zoom, hovered, active);
        }
        EvaluatedShape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let ctrl1 = Vec2::new(control1[0], control1[1]);
            let ctrl2 = Vec2::new(control2[0], control2[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            draw_bezier_gizmo(&mut gizmos, start_pos, ctrl1, ctrl2, end_pos, zoom, hovered, active);
        }
    }
}

/// Check if a handle is hovered or active.
fn is_highlighted(handle: GizmoHandle, hovered: Option<GizmoHandle>, active: Option<GizmoHandle>) -> bool {
    active == Some(handle) || hovered == Some(handle)
}

/// Get color for a handle (with hover/active highlighting).
fn get_handle_color(base_color: Color, handle: GizmoHandle, hovered: Option<GizmoHandle>, active: Option<GizmoHandle>) -> Color {
    if active == Some(handle) {
        // Active (dragging): bright yellow
        Color::srgb(1.0, 1.0, 0.2)
    } else if hovered == Some(handle) {
        // Hover: bright cyan/white mix for visibility
        Color::srgb(0.4, 1.0, 1.0)
    } else {
        base_color
    }
}

/// Draw selection highlight around the shape.
fn draw_selection_highlight(gizmos: &mut Gizmos, shape: &EvaluatedShape) {
    match shape {
        EvaluatedShape::Circle { center, radius } => {
            let pos = Vec2::new(center[0], center[1]);
            gizmos.circle_2d(
                Isometry2d::from_translation(pos),
                *radius + 0.05,
                GizmoColors::SELECTED,
            );
        }
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let pos = Vec2::new(center[0], center[1]);
            let rot = Rot2::radians(rotation.to_radians());
            let isometry = Isometry2d::new(pos, rot);
            let expanded = Vec2::new(size[0] + 0.1, size[1] + 0.1);
            gizmos.rect_2d(isometry, expanded, GizmoColors::SELECTED);
        }
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            gizmos.line_2d(start_pos, end_pos, GizmoColors::SELECTED);
        }
        EvaluatedShape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            // Draw bezier curve highlight
            let points = bezier_to_points(
                &[start[0], start[1]],
                &[control1[0], control1[1]],
                &[control2[0], control2[1]],
                &[end[0], end[1]],
                20,
            );
            for i in 0..points.len() - 1 {
                gizmos.line_2d(points[i], points[i + 1], GizmoColors::SELECTED);
            }
        }
    }
}

/// Draw circle gizmo (move, radius).
fn draw_circle_gizmo(
    gizmos: &mut Gizmos,
    center: Vec2,
    radius: f32,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0;
    let arrow_length = 0.5;

    // World coordinate X axis (red)
    let x_end = center + Vec2::new(arrow_length, 0.0);
    let x_highlighted = is_highlighted(GizmoHandle::MoveX, hovered, active);
    let x_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::MoveX, hovered, active);
    let x_handle_size = if x_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(center, x_end, x_color);
    draw_arrow_head(gizmos, x_end, Vec2::X, x_handle_size, x_color);

    // World coordinate Y axis (green)
    let y_end = center + Vec2::new(0.0, arrow_length);
    let y_highlighted = is_highlighted(GizmoHandle::MoveY, hovered, active);
    let y_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::MoveY, hovered, active);
    let y_handle_size = if y_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(center, y_end, y_color);
    draw_arrow_head(gizmos, y_end, Vec2::Y, y_handle_size, y_color);

    // Free move (center square)
    let free_highlighted = is_highlighted(GizmoHandle::MoveFree, hovered, active);
    let free_color = get_handle_color(GizmoColors::FREE, GizmoHandle::MoveFree, hovered, active);
    let free_size = if free_highlighted { base_handle_size * 2.0 } else { base_handle_size * 1.5 };
    gizmos.rect_2d(
        Isometry2d::from_translation(center),
        Vec2::splat(free_size),
        free_color,
    );

    // Radius handles (4 cardinal directions on the circle edge)
    let radius_handles = [
        (center + Vec2::new(0.0, radius), GizmoHandle::RadiusTop),
        (center + Vec2::new(0.0, -radius), GizmoHandle::RadiusBottom),
        (center + Vec2::new(-radius, 0.0), GizmoHandle::RadiusLeft),
        (center + Vec2::new(radius, 0.0), GizmoHandle::RadiusRight),
    ];

    for (pos, handle) in radius_handles {
        let highlighted = is_highlighted(handle, hovered, active);
        let color = get_handle_color(GizmoColors::SCALE, handle, hovered, active);
        let size = if highlighted { base_handle_size * 1.3 } else { base_handle_size };
        // Draw diamond shape for radius handles
        draw_diamond(gizmos, pos, size, color);
    }

    // Draw radius indicator line (from center to top handle)
    let radius_line_color = Color::srgba(0.9, 0.6, 0.2, 0.5);
    gizmos.line_2d(center, center + Vec2::new(0.0, radius), radius_line_color);
}

/// Draw a diamond shape for radius handles.
fn draw_diamond(gizmos: &mut Gizmos, center: Vec2, size: f32, color: Color) {
    let half = size * 0.7;
    let top = center + Vec2::new(0.0, half);
    let bottom = center + Vec2::new(0.0, -half);
    let left = center + Vec2::new(-half, 0.0);
    let right = center + Vec2::new(half, 0.0);

    gizmos.line_2d(top, right, color);
    gizmos.line_2d(right, bottom, color);
    gizmos.line_2d(bottom, left, color);
    gizmos.line_2d(left, top, color);
}

/// Draw standard gizmo (move, scale, rotate) for rect.
fn draw_standard_gizmo(
    gizmos: &mut Gizmos,
    center: Vec2,
    size: Vec2,
    rotation: f32,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0; // Scale-independent handle size
    let arrow_length = 0.5;
    let local_arrow_length = 0.35; // Shorter for local gizmo
    let rot = Rot2::radians(rotation);

    // World coordinate X axis (red) - solid line, no rotation
    let x_end = center + Vec2::new(arrow_length, 0.0);
    let x_highlighted = is_highlighted(GizmoHandle::MoveX, hovered, active);
    let x_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::MoveX, hovered, active);
    let x_handle_size = if x_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(center, x_end, x_color);
    draw_arrow_head(gizmos, x_end, Vec2::X, x_handle_size, x_color);

    // World coordinate Y axis (green) - solid line, no rotation
    let y_end = center + Vec2::new(0.0, arrow_length);
    let y_highlighted = is_highlighted(GizmoHandle::MoveY, hovered, active);
    let y_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::MoveY, hovered, active);
    let y_handle_size = if y_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(center, y_end, y_color);
    draw_arrow_head(gizmos, y_end, Vec2::Y, y_handle_size, y_color);

    // Local coordinate gizmos (only when rotation is not zero)
    let rotation_threshold = 0.01; // ~0.5 degrees
    if rotation.abs() > rotation_threshold {
        // Local X axis - dashed line, rotated
        let local_x_end = center + rot * Vec2::new(local_arrow_length, 0.0);
        let local_x_highlighted = is_highlighted(GizmoHandle::LocalMoveX, hovered, active);
        let local_x_color = get_handle_color(GizmoColors::LOCAL_X_AXIS, GizmoHandle::LocalMoveX, hovered, active);
        let local_x_handle_size = if local_x_highlighted { base_handle_size * 1.3 } else { base_handle_size };

        draw_dashed_line(gizmos, center, local_x_end, local_x_color, 0.03);
        draw_arrow_head(gizmos, local_x_end, rot * Vec2::X, local_x_handle_size * 0.8, local_x_color);

        // Local Y axis - dashed line, rotated
        let local_y_end = center + rot * Vec2::new(0.0, local_arrow_length);
        let local_y_highlighted = is_highlighted(GizmoHandle::LocalMoveY, hovered, active);
        let local_y_color = get_handle_color(GizmoColors::LOCAL_Y_AXIS, GizmoHandle::LocalMoveY, hovered, active);
        let local_y_handle_size = if local_y_highlighted { base_handle_size * 1.3 } else { base_handle_size };

        draw_dashed_line(gizmos, center, local_y_end, local_y_color, 0.03);
        draw_arrow_head(gizmos, local_y_end, rot * Vec2::Y, local_y_handle_size * 0.8, local_y_color);
    }

    // Free move (center square)
    let free_highlighted = is_highlighted(GizmoHandle::MoveFree, hovered, active);
    let free_color = get_handle_color(GizmoColors::FREE, GizmoHandle::MoveFree, hovered, active);
    let free_size = if free_highlighted { base_handle_size * 2.0 } else { base_handle_size * 1.5 };
    gizmos.rect_2d(
        Isometry2d::from_translation(center),
        Vec2::splat(free_size),
        free_color,
    );

    // Scale handles (corners)
    let half = size / 2.0;
    let scale_handles = [
        (center + rot * Vec2::new(-half.x, half.y), GizmoHandle::ScaleTopLeft),
        (center + rot * Vec2::new(half.x, half.y), GizmoHandle::ScaleTopRight),
        (center + rot * Vec2::new(-half.x, -half.y), GizmoHandle::ScaleBottomLeft),
        (center + rot * Vec2::new(half.x, -half.y), GizmoHandle::ScaleBottomRight),
    ];

    for (corner, handle) in scale_handles {
        let highlighted = is_highlighted(handle, hovered, active);
        let color = get_handle_color(GizmoColors::SCALE, handle, hovered, active);
        let size = if highlighted { base_handle_size * 1.3 } else { base_handle_size };
        gizmos.rect_2d(
            Isometry2d::from_translation(corner),
            Vec2::splat(size),
            color,
        );
    }

    // Rotation ring
    let rotate_highlighted = is_highlighted(GizmoHandle::Rotate, hovered, active);
    let rotate_color = get_handle_color(GizmoColors::ROTATE, GizmoHandle::Rotate, hovered, active);
    let rotate_radius = size.max_element() / 2.0 + 0.2;
    gizmos.circle_2d(
        Isometry2d::from_translation(center),
        rotate_radius,
        rotate_color,
    );
    // Draw thicker if highlighted
    if rotate_highlighted {
        gizmos.circle_2d(
            Isometry2d::from_translation(center),
            rotate_radius + 0.02,
            rotate_color,
        );
    }
}

/// Draw a dashed line.
fn draw_dashed_line(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color, dash_length: f32) {
    let dir = end - start;
    let length = dir.length();
    if length < 0.001 {
        return;
    }
    let dir_normalized = dir / length;
    let gap_length = dash_length * 0.6;
    let segment_length = dash_length + gap_length;

    let mut current = 0.0;
    while current < length {
        let dash_start = start + dir_normalized * current;
        let dash_end_dist = (current + dash_length).min(length);
        let dash_end = start + dir_normalized * dash_end_dist;
        gizmos.line_2d(dash_start, dash_end, color);
        current += segment_length;
    }
}

/// Draw arrow head.
fn draw_arrow_head(gizmos: &mut Gizmos, tip: Vec2, direction: Vec2, size: f32, color: Color) {
    let perp = Vec2::new(-direction.y, direction.x);
    let base = tip - direction * size;
    gizmos.line_2d(tip, base + perp * size * 0.5, color);
    gizmos.line_2d(tip, base - perp * size * 0.5, color);
}

/// Draw line gizmo.
fn draw_line_gizmo(
    gizmos: &mut Gizmos,
    start: Vec2,
    end: Vec2,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0;
    let line_center = (start + end) / 2.0;
    let arrow_length = 0.5;

    // Start handle
    let start_highlighted = is_highlighted(GizmoHandle::LineStart, hovered, active);
    let start_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::LineStart, hovered, active);
    let start_size = if start_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.circle_2d(
        Isometry2d::from_translation(start),
        start_size,
        start_color,
    );

    // End handle
    let end_highlighted = is_highlighted(GizmoHandle::LineEnd, hovered, active);
    let end_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::LineEnd, hovered, active);
    let end_size = if end_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.circle_2d(
        Isometry2d::from_translation(end),
        end_size,
        end_color,
    );

    // X axis arrow from center
    let x_end = line_center + Vec2::new(arrow_length, 0.0);
    let x_highlighted = is_highlighted(GizmoHandle::MoveX, hovered, active);
    let x_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::MoveX, hovered, active);
    let x_handle_size = if x_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.line_2d(line_center, x_end, x_color);
    draw_arrow_head(gizmos, x_end, Vec2::X, x_handle_size, x_color);

    // Y axis arrow from center
    let y_end = line_center + Vec2::new(0.0, arrow_length);
    let y_highlighted = is_highlighted(GizmoHandle::MoveY, hovered, active);
    let y_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::MoveY, hovered, active);
    let y_handle_size = if y_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.line_2d(line_center, y_end, y_color);
    draw_arrow_head(gizmos, y_end, Vec2::Y, y_handle_size, y_color);

    // Center (free move)
    let free_highlighted = is_highlighted(GizmoHandle::MoveFree, hovered, active);
    let free_color = get_handle_color(GizmoColors::FREE, GizmoHandle::MoveFree, hovered, active);
    let free_size = if free_highlighted { base_handle_size * 2.0 } else { base_handle_size * 1.5 };
    gizmos.rect_2d(
        Isometry2d::from_translation(line_center),
        Vec2::splat(free_size),
        free_color,
    );
}

/// Draw bezier gizmo.
fn draw_bezier_gizmo(
    gizmos: &mut Gizmos,
    start: Vec2,
    control1: Vec2,
    control2: Vec2,
    end: Vec2,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0;
    let bezier_center = (start + end) / 2.0;
    let arrow_length = 0.5;

    // Control lines
    gizmos.line_2d(start, control1, GizmoColors::BEZIER_CONTROL);
    gizmos.line_2d(end, control2, GizmoColors::BEZIER_CONTROL);

    // Start point
    let start_highlighted = is_highlighted(GizmoHandle::BezierStart, hovered, active);
    let start_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::BezierStart, hovered, active);
    let start_size = if start_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.circle_2d(
        Isometry2d::from_translation(start),
        start_size,
        start_color,
    );

    // Control point 1
    let ctrl1_highlighted = is_highlighted(GizmoHandle::BezierControl1, hovered, active);
    let ctrl1_color = get_handle_color(GizmoColors::BEZIER_CONTROL, GizmoHandle::BezierControl1, hovered, active);
    let ctrl1_size = if ctrl1_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.rect_2d(
        Isometry2d::from_translation(control1),
        Vec2::splat(ctrl1_size),
        ctrl1_color,
    );

    // Control point 2
    let ctrl2_highlighted = is_highlighted(GizmoHandle::BezierControl2, hovered, active);
    let ctrl2_color = get_handle_color(GizmoColors::BEZIER_CONTROL, GizmoHandle::BezierControl2, hovered, active);
    let ctrl2_size = if ctrl2_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.rect_2d(
        Isometry2d::from_translation(control2),
        Vec2::splat(ctrl2_size),
        ctrl2_color,
    );

    // End point
    let end_highlighted = is_highlighted(GizmoHandle::BezierEnd, hovered, active);
    let end_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::BezierEnd, hovered, active);
    let end_size = if end_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.circle_2d(
        Isometry2d::from_translation(end),
        end_size,
        end_color,
    );

    // X axis arrow from center
    let x_end = bezier_center + Vec2::new(arrow_length, 0.0);
    let x_highlighted = is_highlighted(GizmoHandle::MoveX, hovered, active);
    let x_color = get_handle_color(GizmoColors::X_AXIS, GizmoHandle::MoveX, hovered, active);
    let x_handle_size = if x_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.line_2d(bezier_center, x_end, x_color);
    draw_arrow_head(gizmos, x_end, Vec2::X, x_handle_size, x_color);

    // Y axis arrow from center
    let y_end = bezier_center + Vec2::new(0.0, arrow_length);
    let y_highlighted = is_highlighted(GizmoHandle::MoveY, hovered, active);
    let y_color = get_handle_color(GizmoColors::Y_AXIS, GizmoHandle::MoveY, hovered, active);
    let y_handle_size = if y_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    gizmos.line_2d(bezier_center, y_end, y_color);
    draw_arrow_head(gizmos, y_end, Vec2::Y, y_handle_size, y_color);

    // Center (free move)
    let free_highlighted = is_highlighted(GizmoHandle::MoveFree, hovered, active);
    let free_color = get_handle_color(GizmoColors::FREE, GizmoHandle::MoveFree, hovered, active);
    let free_size = if free_highlighted { base_handle_size * 2.0 } else { base_handle_size * 1.5 };
    gizmos.rect_2d(
        Isometry2d::from_translation(bezier_center),
        Vec2::splat(free_size),
        free_color,
    );
}

/// Convert bezier curve to line segments.
pub fn bezier_to_points(
    start: &[f32; 2],
    control1: &[f32; 2],
    control2: &[f32; 2],
    end: &[f32; 2],
    segments: usize,
) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(segments + 1);

    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        let x = mt3 * start[0]
            + 3.0 * mt2 * t * control1[0]
            + 3.0 * mt * t2 * control2[0]
            + t3 * end[0];
        let y = mt3 * start[1]
            + 3.0 * mt2 * t * control1[1]
            + 3.0 * mt * t2 * control2[1]
            + t3 * end[1];

        points.push(Vec2::new(x, y));
    }

    points
}

/// Render highlighted sequence targets.
pub fn render_sequence_targets(
    mut gizmos: Gizmos,
    editor_state: Res<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
) {
    let Some(map_config) = map_config else {
        return;
    };

    let Some(seq_idx) = editor_state.selected_sequence else {
        return;
    };

    let Some(sequence) = map_config.0.keyframes.get(seq_idx) else {
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);

    // Highlight all objects that are targets of this sequence
    for target_id in &sequence.target_ids {
        if let Some(obj) = map_config.0.objects.iter().find(|o| o.id.as_ref() == Some(target_id)) {
            let shape = obj.shape.evaluate(&ctx);
            let color = Color::srgba(0.9, 0.6, 0.2, 0.5); // Orange highlight

            match &shape {
                EvaluatedShape::Circle { center, radius } => {
                    let pos = Vec2::new(center[0], center[1]);
                    gizmos.circle_2d(
                        Isometry2d::from_translation(pos),
                        *radius + 0.03,
                        color,
                    );
                }
                EvaluatedShape::Rect {
                    center,
                    size,
                    rotation,
                } => {
                    let pos = Vec2::new(center[0], center[1]);
                    let rot = Rot2::radians(rotation.to_radians());
                    let isometry = Isometry2d::new(pos, rot);
                    let expanded = Vec2::new(size[0] + 0.06, size[1] + 0.06);
                    gizmos.rect_2d(isometry, expanded, color);
                }
                EvaluatedShape::Line { start, end } => {
                    let start_pos = Vec2::new(start[0], start[1]);
                    let end_pos = Vec2::new(end[0], end[1]);
                    gizmos.line_2d(start_pos, end_pos, color);
                }
                EvaluatedShape::Bezier {
                    start,
                    control1,
                    control2,
                    end,
                    ..
                } => {
                    let points = bezier_to_points(
                        &[start[0], start[1]],
                        &[control1[0], control1[1]],
                        &[control2[0], control2[1]],
                        &[end[0], end[1]],
                        20,
                    );
                    for i in 0..points.len() - 1 {
                        gizmos.line_2d(points[i], points[i + 1], color);
                    }
                }
            }
        }
    }
}

// ========== Keyframe Gizmo Rendering ==========

/// Render keyframe-specific gizmos (for PivotRotate, Apply, ContinuousRotate).
pub fn render_keyframe_gizmos(
    mut gizmos: Gizmos,
    editor_state: Res<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
    camera_query: Query<(&GameCamera, &GlobalTransform), With<MainCamera>>,
) {
    let Some(map_config) = map_config else {
        return;
    };

    // Need both sequence and keyframe selected
    let Some(seq_idx) = editor_state.selected_sequence else {
        return;
    };

    let Some(kf_idx) = editor_state.selected_keyframe else {
        return;
    };

    let Some(sequence) = map_config.0.keyframes.get(seq_idx) else {
        return;
    };

    let Some(keyframe) = sequence.keyframes.get(kf_idx) else {
        return;
    };

    // Get zoom for scale-independent sizing
    let zoom = camera_query
        .single()
        .map(|(cam, _)| cam.zoom)
        .unwrap_or(100.0);

    let ctx = crate::dsl::GameContext::new(0.0, 0);

    // Get target centers (average position of all targets)
    let target_centers: Vec<Vec2> = sequence
        .target_ids
        .iter()
        .filter_map(|target_id| {
            map_config.0.objects.iter()
                .find(|o| o.id.as_ref() == Some(target_id))
                .map(|obj| {
                    let shape = obj.shape.evaluate(&ctx);
                    get_shape_center(&shape)
                })
        })
        .collect();

    if target_centers.is_empty() {
        return;
    }

    // Average center of all targets
    let avg_center = target_centers.iter().fold(Vec2::ZERO, |acc, c| acc + *c) / target_centers.len() as f32;

    let hovered = editor_state.hovered_handle;
    let active = editor_state.active_handle;

    match keyframe {
        Keyframe::PivotRotate { pivot, pivot_mode, angle, .. } => {
            draw_pivot_rotate_gizmo(&mut gizmos, *pivot, *pivot_mode, *angle, avg_center, zoom, hovered, active);
        }
        Keyframe::Apply { translation, rotation, .. } => {
            draw_apply_gizmo(&mut gizmos, *translation, *rotation, avg_center, zoom, hovered, active);
        }
        Keyframe::ContinuousRotate { direction, .. } => {
            draw_continuous_rotate_gizmo(&mut gizmos, direction, avg_center, zoom);
        }
        // Other keyframe types (LoopStart, LoopEnd, Delay) don't have visual gizmos
        _ => {}
    }
}

/// Get the center of an evaluated shape.
fn get_shape_center(shape: &EvaluatedShape) -> Vec2 {
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

/// Draw PivotRotate gizmo: pivot marker and rotation arc.
fn draw_pivot_rotate_gizmo(
    gizmos: &mut Gizmos,
    pivot: [f32; 2],
    pivot_mode: PivotMode,
    angle: f32,
    target_center: Vec2,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    // Calculate world pivot position based on pivot mode
    let pivot_pos = match pivot_mode {
        PivotMode::Absolute => Vec2::new(pivot[0], pivot[1]),
        PivotMode::Relative => target_center + Vec2::new(pivot[0], pivot[1]),
    };
    let base_handle_size = 0.08 / zoom * 100.0;

    // Draw connection line from pivot to target
    let connection_color = Color::srgba(0.9, 0.4, 0.9, 0.3);
    gizmos.line_2d(pivot_pos, target_center, connection_color);

    // Draw pivot marker (diamond shape, draggable)
    let pivot_highlighted = is_highlighted(GizmoHandle::KeyframePivot, hovered, active);
    let pivot_color = get_handle_color(GizmoColors::KEYFRAME_PIVOT, GizmoHandle::KeyframePivot, hovered, active);
    let pivot_size = if pivot_highlighted { base_handle_size * 1.5 } else { base_handle_size * 1.2 };
    draw_diamond(gizmos, pivot_pos, pivot_size, pivot_color);

    // Draw a small filled circle at pivot center
    gizmos.circle_2d(
        Isometry2d::from_translation(pivot_pos),
        base_handle_size * 0.3,
        pivot_color,
    );

    // Calculate start angle from pivot to target
    let start_angle_rad = (target_center - pivot_pos).to_angle();
    let end_angle_rad = start_angle_rad + angle.to_radians();

    // Draw rotation arc (from start_angle to end_angle, around pivot)
    let arc_radius = pivot_pos.distance(target_center).max(0.3);
    let angle_highlighted = is_highlighted(GizmoHandle::KeyframeAngle, hovered, active);
    let arc_color = get_handle_color(GizmoColors::KEYFRAME_ROTATE, GizmoHandle::KeyframeAngle, hovered, active);

    draw_rotation_arc(gizmos, pivot_pos, arc_radius, start_angle_rad, end_angle_rad, arc_color, angle_highlighted);

    // Draw angle indicator arrow at end of arc
    let arc_end = pivot_pos + Vec2::new(end_angle_rad.cos(), end_angle_rad.sin()) * arc_radius;
    // Tangent direction depends on rotation direction (CW vs CCW)
    let tangent_dir = if angle >= 0.0 { 1.0 } else { -1.0 };
    let tangent = Vec2::new(-end_angle_rad.sin(), end_angle_rad.cos()) * tangent_dir;
    let arrow_size = if angle_highlighted { base_handle_size * 1.3 } else { base_handle_size };
    draw_arrow_head(gizmos, arc_end, tangent, arrow_size, arc_color);
}

/// Draw Apply gizmo: translation arrows and optional rotation arc.
fn draw_apply_gizmo(
    gizmos: &mut Gizmos,
    translation: Option<[f32; 2]>,
    rotation: Option<f32>,
    target_center: Vec2,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0;
    let arrow_length = 0.5;

    // Translation handles (if translation is set or for editing)
    let trans = translation.unwrap_or([0.0, 0.0]);

    // X translation arrow - direction follows translation sign
    let x_dir = if trans[0] >= 0.0 { 1.0 } else { -1.0 };
    let x_end = target_center + Vec2::new(arrow_length * x_dir, 0.0);
    let x_highlighted = is_highlighted(GizmoHandle::KeyframeTranslateX, hovered, active);
    let x_color = get_handle_color(GizmoColors::KEYFRAME_TRANSLATE, GizmoHandle::KeyframeTranslateX, hovered, active);
    let x_handle_size = if x_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(target_center, x_end, x_color);
    draw_arrow_head(gizmos, x_end, Vec2::X * x_dir, x_handle_size, x_color);

    // Y translation arrow - direction follows translation sign
    let y_dir = if trans[1] >= 0.0 { 1.0 } else { -1.0 };
    let y_end = target_center + Vec2::new(0.0, arrow_length * y_dir);
    let y_highlighted = is_highlighted(GizmoHandle::KeyframeTranslateY, hovered, active);
    let y_color = get_handle_color(GizmoColors::KEYFRAME_TRANSLATE, GizmoHandle::KeyframeTranslateY, hovered, active);
    let y_handle_size = if y_highlighted { base_handle_size * 1.3 } else { base_handle_size };

    gizmos.line_2d(target_center, y_end, y_color);
    draw_arrow_head(gizmos, y_end, Vec2::Y * y_dir, y_handle_size, y_color);

    // Free translation (center square)
    let free_highlighted = is_highlighted(GizmoHandle::KeyframeTranslateFree, hovered, active);
    let free_color = get_handle_color(GizmoColors::KEYFRAME_TRANSLATE, GizmoHandle::KeyframeTranslateFree, hovered, active);
    let free_size = if free_highlighted { base_handle_size * 2.0 } else { base_handle_size * 1.5 };
    gizmos.rect_2d(
        Isometry2d::from_translation(target_center),
        Vec2::splat(free_size),
        free_color,
    );

    // Draw ghost line showing actual translation offset
    if trans[0].abs() > 0.001 || trans[1].abs() > 0.001 {
        let trans_end = target_center + Vec2::new(trans[0], trans[1]);
        let ghost_color = Color::srgba(0.3, 0.8, 0.3, 0.4);
        gizmos.line_2d(target_center, trans_end, ghost_color);
        // Draw small marker at translation end
        gizmos.circle_2d(
            Isometry2d::from_translation(trans_end),
            base_handle_size * 0.4,
            ghost_color,
        );
    }

    // Rotation arc (if rotation is set)
    if let Some(rot_deg) = rotation {
        if rot_deg.abs() > 0.001 {
            let arc_radius = 0.4;
            let angle_highlighted = is_highlighted(GizmoHandle::KeyframeAngle, hovered, active);
            let arc_color = get_handle_color(GizmoColors::KEYFRAME_ROTATE, GizmoHandle::KeyframeAngle, hovered, active);

            draw_rotation_arc(gizmos, target_center, arc_radius, 0.0, rot_deg.to_radians(), arc_color, angle_highlighted);
        }
    }
}

/// Draw ContinuousRotate gizmo: direction indicator (read-only).
fn draw_continuous_rotate_gizmo(
    gizmos: &mut Gizmos,
    direction: &crate::map::RollDirection,
    target_center: Vec2,
    zoom: f32,
) {
    let base_size = 0.08 / zoom * 100.0;
    let radius = 0.3;
    let color = Color::srgba(0.5, 0.5, 0.9, 0.7);

    // Draw circular arc indicating rotation direction
    let segments = 24;
    let arc_angle = std::f32::consts::PI * 1.5; // 270 degrees

    let start_angle = match direction {
        crate::map::RollDirection::Clockwise => 0.0,
        crate::map::RollDirection::Counterclockwise => 0.0,
    };

    let direction_mult = match direction {
        crate::map::RollDirection::Clockwise => 1.0,
        crate::map::RollDirection::Counterclockwise => -1.0,
    };

    for i in 0..segments {
        let t1 = i as f32 / segments as f32;
        let t2 = (i + 1) as f32 / segments as f32;
        let a1 = start_angle + arc_angle * t1 * direction_mult;
        let a2 = start_angle + arc_angle * t2 * direction_mult;

        let p1 = target_center + Vec2::new(a1.cos(), a1.sin()) * radius;
        let p2 = target_center + Vec2::new(a2.cos(), a2.sin()) * radius;

        gizmos.line_2d(p1, p2, color);
    }

    // Draw arrow at end of arc
    let end_angle = start_angle + arc_angle * direction_mult;
    let arc_end = target_center + Vec2::new(end_angle.cos(), end_angle.sin()) * radius;
    let tangent = Vec2::new(-end_angle.sin(), end_angle.cos()) * direction_mult;
    draw_arrow_head(gizmos, arc_end, tangent, base_size, color);
}

/// Draw a rotation arc from start_angle to end_angle.
fn draw_rotation_arc(
    gizmos: &mut Gizmos,
    center: Vec2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: Color,
    _highlighted: bool,
) {
    let segments = 32;
    let angle_span = end_angle - start_angle;

    // Draw arc segments
    for i in 0..segments {
        let t1 = i as f32 / segments as f32;
        let t2 = (i + 1) as f32 / segments as f32;
        let a1 = start_angle + angle_span * t1;
        let a2 = start_angle + angle_span * t2;

        let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
        let p2 = center + Vec2::new(a2.cos(), a2.sin()) * radius;

        gizmos.line_2d(p1, p2, color);
    }
    // Note: highlight effect is already applied via color from get_handle_color()
}

// ========== Guideline Gizmo Rendering ==========

/// Render gizmo for selected guideline.
pub fn render_guideline_gizmo(
    mut gizmos: Gizmos,
    editor_state: Res<EditorStateRes>,
    map_config: Option<Res<MapConfig>>,
    camera_query: Query<(&GameCamera, &GlobalTransform), With<MainCamera>>,
) {
    let Some(map_config) = map_config else {
        return;
    };

    // Skip if keyframe is selected (keyframe gizmo takes priority)
    if editor_state.selected_keyframe.is_some() && editor_state.selected_sequence.is_some() {
        return;
    }

    let Some(selected_idx) = editor_state.selected_object else {
        return;
    };

    let Some(obj) = map_config.0.objects.get(selected_idx) else {
        return;
    };

    // Only render for guidelines
    if obj.role != ObjectRole::Guideline {
        return;
    }

    let zoom = camera_query
        .single()
        .map(|(cam, _)| cam.zoom)
        .unwrap_or(100.0);

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let shape = obj.shape.evaluate(&ctx);

    let hovered = editor_state.hovered_handle;
    let active = editor_state.active_handle;

    match &shape {
        EvaluatedShape::Line { start, end } => {
            let start_pos = Vec2::new(start[0], start[1]);
            let end_pos = Vec2::new(end[0], end[1]);
            draw_guideline_gizmo(&mut gizmos, start_pos, end_pos, zoom, hovered, active);
        }
        _ => {
            // For non-line guidelines, just draw selection highlight
            draw_selection_highlight(&mut gizmos, &shape);
        }
    }
}

/// Draw guideline gizmo with endpoint handles.
fn draw_guideline_gizmo(
    gizmos: &mut Gizmos,
    start: Vec2,
    end: Vec2,
    zoom: f32,
    hovered: Option<GizmoHandle>,
    active: Option<GizmoHandle>,
) {
    let base_handle_size = 0.08 / zoom * 100.0;
    let line_center = (start + end) / 2.0;

    // Draw selection highlight (dashed line)
    draw_guideline_dashed_line(gizmos, start, end, GizmoColors::SELECTED, 0.08);

    // Start handle (diamond)
    let start_highlighted = is_highlighted(GizmoHandle::GuidelineStart, hovered, active);
    let start_color = get_handle_color(
        GizmoColors::GUIDELINE_ENDPOINT,
        GizmoHandle::GuidelineStart,
        hovered,
        active,
    );
    let start_size = if start_highlighted {
        base_handle_size * 1.5
    } else {
        base_handle_size * 1.2
    };
    draw_diamond(gizmos, start, start_size, start_color);

    // End handle (diamond)
    let end_highlighted = is_highlighted(GizmoHandle::GuidelineEnd, hovered, active);
    let end_color = get_handle_color(
        GizmoColors::GUIDELINE_ENDPOINT,
        GizmoHandle::GuidelineEnd,
        hovered,
        active,
    );
    let end_size = if end_highlighted {
        base_handle_size * 1.5
    } else {
        base_handle_size * 1.2
    };
    draw_diamond(gizmos, end, end_size, end_color);

    // Center move handle (square)
    let center_highlighted = is_highlighted(GizmoHandle::GuidelineMove, hovered, active);
    let center_color = get_handle_color(
        GizmoColors::GUIDELINE,
        GizmoHandle::GuidelineMove,
        hovered,
        active,
    );
    let center_size = if center_highlighted {
        base_handle_size * 2.0
    } else {
        base_handle_size * 1.5
    };
    gizmos.rect_2d(
        Isometry2d::from_translation(line_center),
        Vec2::splat(center_size),
        center_color,
    );
}

/// Draw a dashed line for guideline visualization.
fn draw_guideline_dashed_line(
    gizmos: &mut Gizmos,
    start: Vec2,
    end: Vec2,
    color: Color,
    dash_length: f32,
) {
    let dir = end - start;
    let length = dir.length();

    if length < 0.001 {
        return;
    }

    let dir_normalized = dir / length;
    let gap_length = dash_length * 0.6;
    let segment_length = dash_length + gap_length;

    let mut current = 0.0;
    while current < length {
        let dash_start = start + dir_normalized * current;
        let dash_end_dist = (current + dash_length).min(length);
        let dash_end = start + dir_normalized * dash_end_dist;
        gizmos.line_2d(dash_start, dash_end, color);
        current += segment_length;
    }
}

// ========== Distance Lines Rendering ==========

/// Render distance lines from selected object to nearby guidelines.
pub fn render_distance_lines(
    mut gizmos: Gizmos,
    editor_state: Res<EditorStateRes>,
    snap_config: Option<Res<SnapConfig>>,
    map_config: Option<Res<MapConfig>>,
    guidelines: Query<(&MapObjectMarker, &GuidelineMarker)>,
) {
    let Some(snap_config) = snap_config else {
        return;
    };

    if !snap_config.show_distance_lines {
        return;
    }

    let Some(map_config) = map_config else {
        return;
    };

    // Only show distance lines when dragging or hovering object
    if !editor_state.is_dragging && editor_state.selected_object.is_none() {
        return;
    }

    let Some(selected_idx) = editor_state.selected_object else {
        return;
    };

    let Some(obj) = map_config.0.objects.get(selected_idx) else {
        return;
    };

    // Don't show distance lines for guidelines themselves
    if obj.role == ObjectRole::Guideline {
        return;
    }

    let ctx = crate::dsl::GameContext::new(0.0, 0);
    let shape = obj.shape.evaluate(&ctx);
    let object_center = get_shape_center(&shape);

    // Collect snap targets from guidelines
    let mut snap_targets: Vec<Box<dyn SnapTarget>> = Vec::new();

    for (marker, guideline) in guidelines.iter() {
        if !guideline.snap_enabled {
            continue;
        }

        // Find the corresponding object in config to get shape
        if let Some(gl_obj) = map_config
            .0
            .objects
            .iter()
            .find(|o| o.id.as_ref() == marker.object_id.as_ref())
        {
            let gl_shape = gl_obj.shape.evaluate(&ctx);
            if let EvaluatedShape::Line { start, end } = gl_shape {
                snap_targets.push(Box::new(LineSnapTarget::new(
                    Vec2::new(start[0], start[1]),
                    Vec2::new(end[0], end[1]),
                    guideline.snap_distance,
                    guideline.ruler_interval,
                )));
            }
        }
    }

    // Convert to trait objects for calculate_distance_lines
    let target_refs: Vec<&dyn SnapTarget> = snap_targets.iter().map(|t| t.as_ref()).collect();

    // Calculate and render distance lines
    let distance_lines =
        calculate_distance_lines(object_center, &target_refs, snap_config.distance_line_threshold);

    for line in distance_lines {
        // Draw dashed line from object to guideline
        draw_distance_line(&mut gizmos, line.from, line.to, GizmoColors::DISTANCE_LINE);
    }
}

/// Draw a single distance line (dashed).
fn draw_distance_line(gizmos: &mut Gizmos, from: Vec2, to: Vec2, color: Color) {
    let dir = to - from;
    let length = dir.length();

    if length < 0.001 {
        return;
    }

    let dir_normalized = dir / length;
    let dash_length = 0.05;
    let gap_length = 0.03;
    let segment_length = dash_length + gap_length;

    let mut current = 0.0;
    while current < length {
        let dash_start = from + dir_normalized * current;
        let dash_end_dist = (current + dash_length).min(length);
        let dash_end = from + dir_normalized * dash_end_dist;
        gizmos.line_2d(dash_start, dash_end, color);
        current += segment_length;
    }
}
