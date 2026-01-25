//! Blender-style unified gizmo for the map editor.
//!
//! Shows all handles (move, scale, rotate) simultaneously.

use marble_core::Color;

use super::interaction::{BezierTransform, GizmoHandle, LineTransform, ObjectTransform};
use crate::renderer::{CircleInstance, LineInstance, RectInstance};

/// Gizmo visual constants.
pub mod constants {
    use marble_core::Color;

    pub const ARROW_LENGTH: f32 = 50.0;
    pub const ARROW_WIDTH: f32 = 3.0;
    pub const ARROWHEAD_SIZE: f32 = 10.0;
    pub const CENTER_HANDLE_SIZE: f32 = 14.0;
    pub const SCALE_HANDLE_SIZE: f32 = 8.0;
    pub const ROTATE_RING_RADIUS: f32 = 70.0;
    pub const ROTATE_RING_WIDTH: f32 = 2.5;
    pub const HIT_TOLERANCE: f32 = 12.0;

    pub const COLOR_X_AXIS: Color = Color { r: 230, g: 80, b: 80, a: 255 };
    pub const COLOR_Y_AXIS: Color = Color { r: 80, g: 200, b: 80, a: 255 };
    pub const COLOR_FREE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const COLOR_SCALE: Color = Color { r: 255, g: 180, b: 50, a: 255 };
    pub const COLOR_ROTATE: Color = Color { r: 100, g: 160, b: 255, a: 255 };
    pub const COLOR_HOVER: Color = Color { r: 255, g: 255, b: 100, a: 255 };

    // Bezier gizmo constants
    pub const BEZIER_ENDPOINT_SIZE: f32 = 10.0;
    pub const BEZIER_CONTROL_SIZE: f32 = 8.0;
    pub const BEZIER_CENTER_SIZE: f32 = 12.0;
    pub const BEZIER_TANGENT_WIDTH: f32 = 1.5;
    pub const COLOR_BEZIER_ENDPOINT: Color = Color { r: 230, g: 80, b: 80, a: 255 };
    pub const COLOR_BEZIER_CONTROL: Color = Color { r: 100, g: 180, b: 255, a: 255 };
    pub const COLOR_BEZIER_CENTER: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const COLOR_BEZIER_TANGENT: Color = Color { r: 150, g: 150, b: 150, a: 180 };
}

#[derive(Debug, Clone, Default)]
pub struct GizmoRenderData {
    pub lines: Vec<LineInstance>,
    pub circles: Vec<CircleInstance>,
    pub rects: Vec<RectInstance>,
}

/// Generate unified gizmo with all handles visible.
pub fn generate_gizmo(
    transform: &ObjectTransform,
    zoom: f32,
    hovered_handle: Option<GizmoHandle>,
) -> GizmoRenderData {
    let mut data = GizmoRenderData::default();
    let center = transform.center;
    let scale = 1.0 / zoom;

    generate_rotate_ring(&mut data, center, scale, hovered_handle);
    generate_scale_handles(&mut data, center, transform, scale, hovered_handle);
    generate_move_arrows(&mut data, center, scale, hovered_handle);

    data
}

fn generate_move_arrows(data: &mut GizmoRenderData, center: (f32, f32), scale: f32, hovered: Option<GizmoHandle>) {
    use constants::*;
    let len = ARROW_LENGTH * scale;
    let w = ARROW_WIDTH * scale;
    let head = ARROWHEAD_SIZE * scale;
    let cs = CENTER_HANDLE_SIZE * scale;

    // X arrow
    let xc = if hovered == Some(GizmoHandle::MoveX) { COLOR_HOVER } else { COLOR_X_AXIS };
    data.lines.push(LineInstance::new(center, (center.0 + len, center.1), w, xc));
    data.lines.push(LineInstance::new((center.0 + len - head, center.1 - head * 0.5), (center.0 + len, center.1), w, xc));
    data.lines.push(LineInstance::new((center.0 + len - head, center.1 + head * 0.5), (center.0 + len, center.1), w, xc));

    // Y arrow
    let yc = if hovered == Some(GizmoHandle::MoveY) { COLOR_HOVER } else { COLOR_Y_AXIS };
    data.lines.push(LineInstance::new(center, (center.0, center.1 + len), w, yc));
    data.lines.push(LineInstance::new((center.0 - head * 0.5, center.1 + len - head), (center.0, center.1 + len), w, yc));
    data.lines.push(LineInstance::new((center.0 + head * 0.5, center.1 + len - head), (center.0, center.1 + len), w, yc));

    // Center handle
    let fc = if hovered == Some(GizmoHandle::MoveFree) { COLOR_HOVER } else { COLOR_FREE };
    data.rects.push(RectInstance::new(center, (cs / 2.0, cs / 2.0), 0.0, fc, Color::new(60, 60, 60, 255), 1.5 * scale));
}

fn generate_scale_handles(data: &mut GizmoRenderData, center: (f32, f32), transform: &ObjectTransform, scale: f32, hovered: Option<GizmoHandle>) {
    use constants::*;
    let hs = SCALE_HANDLE_SIZE * scale;
    let hw = (transform.size.0 / 2.0).max(30.0 * scale);
    let hh = (transform.size.1 / 2.0).max(30.0 * scale);
    let line_width = 2.0 * scale;
    let rot = transform.rotation;

    // Corners in local space, then rotated
    let corners = [
        rotate_point((-hw, -hh), center, rot), // 0: top-left
        rotate_point((hw, -hh), center, rot),  // 1: top-right
        rotate_point((hw, hh), center, rot),   // 2: bottom-right
        rotate_point((-hw, hh), center, rot),  // 3: bottom-left
    ];

    // Edge lines (draggable for axis-aligned scale) - rotated with object
    let edge_handles = [
        (corners[0], corners[1], GizmoHandle::ScaleTop),    // top edge
        (corners[1], corners[2], GizmoHandle::ScaleRight),  // right edge
        (corners[2], corners[3], GizmoHandle::ScaleBottom), // bottom edge
        (corners[3], corners[0], GizmoHandle::ScaleLeft),   // left edge
    ];
    let box_color = Color::new(120, 120, 120, 180);
    for (start, end, handle) in &edge_handles {
        let c = if hovered == Some(*handle) { COLOR_HOVER } else { box_color };
        data.lines.push(LineInstance::new(*start, *end, line_width, c));
    }

    // Corner handles (diagonal scale) - rotated with object
    let corner_handles = [
        (corners[0], GizmoHandle::ScaleTopLeft),
        (corners[1], GizmoHandle::ScaleTopRight),
        (corners[2], GizmoHandle::ScaleBottomRight),
        (corners[3], GizmoHandle::ScaleBottomLeft),
    ];
    for (pos, handle) in &corner_handles {
        let c = if hovered == Some(*handle) { COLOR_HOVER } else { COLOR_SCALE };
        data.rects.push(RectInstance::new(*pos, (hs / 2.0, hs / 2.0), rot, c, Color::new(80, 80, 80, 255), scale));
    }
}

fn generate_rotate_ring(data: &mut GizmoRenderData, center: (f32, f32), scale: f32, hovered: Option<GizmoHandle>) {
    use constants::*;
    let r = ROTATE_RING_RADIUS * scale;
    let w = ROTATE_RING_WIDTH * scale;
    let c = if hovered == Some(GizmoHandle::RotateRing) { COLOR_HOVER } else { COLOR_ROTATE };

    for i in 0..48 {
        let a1 = (i as f32 / 48.0) * std::f32::consts::TAU;
        let a2 = ((i + 1) as f32 / 48.0) * std::f32::consts::TAU;
        data.lines.push(LineInstance::new(
            (center.0 + r * a1.cos(), center.1 + r * a1.sin()),
            (center.0 + r * a2.cos(), center.1 + r * a2.sin()),
            w, c,
        ));
    }
}

pub fn hit_test_gizmo(transform: &ObjectTransform, mouse: (f32, f32), zoom: f32) -> Option<GizmoHandle> {
    let center = transform.center;
    let scale = 1.0 / zoom;
    let tol = constants::HIT_TOLERANCE * scale;

    // 1. Center handle
    let cs = constants::CENTER_HANDLE_SIZE * scale;
    if (mouse.0 - center.0).abs() <= cs / 2.0 && (mouse.1 - center.1).abs() <= cs / 2.0 {
        return Some(GizmoHandle::MoveFree);
    }

    // 2. Move axes
    let len = constants::ARROW_LENGTH * scale;
    if point_near_line((center.0 + cs / 2.0, center.1), (center.0 + len, center.1), mouse, tol) {
        return Some(GizmoHandle::MoveX);
    }
    if point_near_line((center.0, center.1 + cs / 2.0), (center.0, center.1 + len), mouse, tol) {
        return Some(GizmoHandle::MoveY);
    }

    // 3. Scale handles - corners (diagonal) - rotated with object
    let hw = (transform.size.0 / 2.0).max(30.0 * scale);
    let hh = (transform.size.1 / 2.0).max(30.0 * scale);
    let rot = transform.rotation;
    let corners = [
        rotate_point((-hw, -hh), center, rot), // top-left
        rotate_point((hw, -hh), center, rot),  // top-right
        rotate_point((hw, hh), center, rot),   // bottom-right
        rotate_point((-hw, hh), center, rot),  // bottom-left
    ];
    let corner_handles = [
        (corners[0], GizmoHandle::ScaleTopLeft),
        (corners[1], GizmoHandle::ScaleTopRight),
        (corners[2], GizmoHandle::ScaleBottomRight),
        (corners[3], GizmoHandle::ScaleBottomLeft),
    ];
    for (pos, handle) in &corner_handles {
        if dist(mouse, *pos) < tol { return Some(*handle); }
    }

    // 4. Scale handles - edges (lines) - rotated with object
    let edge_handles = [
        (corners[0], corners[1], GizmoHandle::ScaleTop),
        (corners[1], corners[2], GizmoHandle::ScaleRight),
        (corners[2], corners[3], GizmoHandle::ScaleBottom),
        (corners[3], corners[0], GizmoHandle::ScaleLeft),
    ];
    for (start, end, handle) in &edge_handles {
        if point_near_line(*start, *end, mouse, tol) { return Some(*handle); }
    }

    // 5. Rotate ring
    let ring_r = constants::ROTATE_RING_RADIUS * scale;
    if (dist(mouse, center) - ring_r).abs() < tol * 1.5 {
        return Some(GizmoHandle::RotateRing);
    }

    None
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Rotate a point around a center by angle (degrees).
fn rotate_point(offset: (f32, f32), center: (f32, f32), angle_deg: f32) -> (f32, f32) {
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    (
        center.0 + offset.0 * cos - offset.1 * sin,
        center.1 + offset.0 * sin + offset.1 * cos,
    )
}

fn point_near_line(start: (f32, f32), end: (f32, f32), p: (f32, f32), tol: f32) -> bool {
    let len_sq = (end.0 - start.0).powi(2) + (end.1 - start.1).powi(2);
    if len_sq < 0.0001 { return dist(p, start) < tol; }
    let t = ((p.0 - start.0) * (end.0 - start.0) + (p.1 - start.1) * (end.1 - start.1)) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj = (start.0 + t * (end.0 - start.0), start.1 + t * (end.1 - start.1));
    dist(p, proj) < tol
}

pub fn apply_move_transform(handle: GizmoHandle, orig: &ObjectTransform, delta: (f32, f32), snap: bool) -> ObjectTransform {
    let mut r = *orig;
    match handle {
        GizmoHandle::MoveX => { r.center.0 = orig.center.0 + delta.0; if snap { r.center.0 = (r.center.0 / 10.0).round() * 10.0; } }
        GizmoHandle::MoveY => { r.center.1 = orig.center.1 + delta.1; if snap { r.center.1 = (r.center.1 / 10.0).round() * 10.0; } }
        GizmoHandle::MoveFree => {
            r.center.0 = orig.center.0 + delta.0;
            r.center.1 = orig.center.1 + delta.1;
            if snap { r.center.0 = (r.center.0 / 10.0).round() * 10.0; r.center.1 = (r.center.1 / 10.0).round() * 10.0; }
        }
        _ => {}
    }
    r
}

pub fn apply_scale_transform(handle: GizmoHandle, orig: &ObjectTransform, delta: (f32, f32), snap: bool) -> ObjectTransform {
    let mut r = *orig;
    let min = 10.0;

    // Transform delta to object's local coordinate system
    let rad = -orig.rotation.to_radians();
    let local_dx = delta.0 * rad.cos() - delta.1 * rad.sin();
    let local_dy = delta.0 * rad.sin() + delta.1 * rad.cos();

    match handle {
        // Corner handles - diagonal scale (proportional)
        GizmoHandle::ScaleTopLeft => {
            r.size.0 = (orig.size.0 - local_dx * 2.0).max(min);
            r.size.1 = (orig.size.1 - local_dy * 2.0).max(min);
        }
        GizmoHandle::ScaleTopRight => {
            r.size.0 = (orig.size.0 + local_dx * 2.0).max(min);
            r.size.1 = (orig.size.1 - local_dy * 2.0).max(min);
        }
        GizmoHandle::ScaleBottomRight => {
            r.size.0 = (orig.size.0 + local_dx * 2.0).max(min);
            r.size.1 = (orig.size.1 + local_dy * 2.0).max(min);
        }
        GizmoHandle::ScaleBottomLeft => {
            r.size.0 = (orig.size.0 - local_dx * 2.0).max(min);
            r.size.1 = (orig.size.1 + local_dy * 2.0).max(min);
        }
        // Edge handles - single axis scale
        GizmoHandle::ScaleTop => {
            r.size.1 = (orig.size.1 - local_dy * 2.0).max(min);
        }
        GizmoHandle::ScaleBottom => {
            r.size.1 = (orig.size.1 + local_dy * 2.0).max(min);
        }
        GizmoHandle::ScaleLeft => {
            r.size.0 = (orig.size.0 - local_dx * 2.0).max(min);
        }
        GizmoHandle::ScaleRight => {
            r.size.0 = (orig.size.0 + local_dx * 2.0).max(min);
        }
        _ => {}
    }

    if snap {
        r.size.0 = (r.size.0 / 10.0).round() * 10.0;
        r.size.1 = (r.size.1 / 10.0).round() * 10.0;
    }
    r
}

pub fn apply_rotate_transform(orig: &ObjectTransform, start: (f32, f32), curr: (f32, f32), snap: bool) -> ObjectTransform {
    let mut r = *orig;
    let a1 = (start.1 - orig.center.1).atan2(start.0 - orig.center.0);
    let a2 = (curr.1 - orig.center.1).atan2(curr.0 - orig.center.0);
    let mut rot = orig.rotation + (a2 - a1).to_degrees();
    while rot < 0.0 { rot += 360.0; }
    while rot >= 360.0 { rot -= 360.0; }
    if snap { rot = (rot / 15.0).round() * 15.0; }
    r.rotation = rot;
    r
}

// ============================================================================
// Bezier Gizmo Functions
// ============================================================================

/// Generate bezier curve gizmo with 4 control point handles and center move handle.
pub fn generate_bezier_gizmo(
    transform: &BezierTransform,
    zoom: f32,
    hovered_handle: Option<GizmoHandle>,
) -> GizmoRenderData {
    use constants::*;
    let mut data = GizmoRenderData::default();
    let scale = 1.0 / zoom;

    let start = transform.start;
    let ctrl1 = transform.control1;
    let ctrl2 = transform.control2;
    let end = transform.end;
    let center = transform.center();

    // Tangent lines (start -> control1, end -> control2)
    let tw = BEZIER_TANGENT_WIDTH * scale;
    data.lines.push(LineInstance::new(start, ctrl1, tw, COLOR_BEZIER_TANGENT));
    data.lines.push(LineInstance::new(end, ctrl2, tw, COLOR_BEZIER_TANGENT));

    // Control point handles (circles)
    let ctrl_size = BEZIER_CONTROL_SIZE * scale;
    let c1_color = if hovered_handle == Some(GizmoHandle::BezierControl1) { COLOR_HOVER } else { COLOR_BEZIER_CONTROL };
    let c2_color = if hovered_handle == Some(GizmoHandle::BezierControl2) { COLOR_HOVER } else { COLOR_BEZIER_CONTROL };
    data.circles.push(CircleInstance::new(ctrl1, ctrl_size / 2.0, c1_color, Color::new(50, 50, 50, 255), scale));
    data.circles.push(CircleInstance::new(ctrl2, ctrl_size / 2.0, c2_color, Color::new(50, 50, 50, 255), scale));

    // Endpoint handles (squares)
    let ep_size = BEZIER_ENDPOINT_SIZE * scale;
    let start_color = if hovered_handle == Some(GizmoHandle::BezierStart) { COLOR_HOVER } else { COLOR_BEZIER_ENDPOINT };
    let end_color = if hovered_handle == Some(GizmoHandle::BezierEnd) { COLOR_HOVER } else { COLOR_BEZIER_ENDPOINT };
    data.rects.push(RectInstance::new(start, (ep_size / 2.0, ep_size / 2.0), 0.0, start_color, Color::new(50, 50, 50, 255), scale));
    data.rects.push(RectInstance::new(end, (ep_size / 2.0, ep_size / 2.0), 0.0, end_color, Color::new(50, 50, 50, 255), scale));

    // Center move handle (diamond - 45° rotated square)
    let center_size = BEZIER_CENTER_SIZE * scale;
    let center_color = if hovered_handle == Some(GizmoHandle::BezierMoveFree) { COLOR_HOVER } else { COLOR_BEZIER_CENTER };
    data.rects.push(RectInstance::new(center, (center_size / 2.0, center_size / 2.0), 45.0, center_color, Color::new(60, 60, 60, 255), 1.5 * scale));

    data
}

/// Hit test bezier gizmo handles.
pub fn hit_test_bezier_gizmo(transform: &BezierTransform, mouse: (f32, f32), zoom: f32) -> Option<GizmoHandle> {
    use constants::*;
    let scale = 1.0 / zoom;
    let tol = HIT_TOLERANCE * scale;

    let start = transform.start;
    let ctrl1 = transform.control1;
    let ctrl2 = transform.control2;
    let end = transform.end;
    let center = transform.center();

    // Center handle (highest priority for move)
    let center_size = BEZIER_CENTER_SIZE * scale;
    if dist(mouse, center) <= center_size / 2.0 + tol * 0.5 {
        return Some(GizmoHandle::BezierMoveFree);
    }

    // Start endpoint
    let ep_size = BEZIER_ENDPOINT_SIZE * scale;
    if (mouse.0 - start.0).abs() <= ep_size / 2.0 + tol * 0.5
        && (mouse.1 - start.1).abs() <= ep_size / 2.0 + tol * 0.5
    {
        return Some(GizmoHandle::BezierStart);
    }

    // End endpoint
    if (mouse.0 - end.0).abs() <= ep_size / 2.0 + tol * 0.5
        && (mouse.1 - end.1).abs() <= ep_size / 2.0 + tol * 0.5
    {
        return Some(GizmoHandle::BezierEnd);
    }

    // Control1
    let ctrl_size = BEZIER_CONTROL_SIZE * scale;
    if dist(mouse, ctrl1) <= ctrl_size / 2.0 + tol * 0.5 {
        return Some(GizmoHandle::BezierControl1);
    }

    // Control2
    if dist(mouse, ctrl2) <= ctrl_size / 2.0 + tol * 0.5 {
        return Some(GizmoHandle::BezierControl2);
    }

    None
}

/// Snap threshold for bezier points to snap to each other (in world units).
const BEZIER_POINT_SNAP_THRESHOLD: f32 = 15.0;

/// Threshold for considering two points as "merged" (same position).
const BEZIER_MERGED_THRESHOLD: f32 = 0.5;

/// Apply transform to bezier based on handle and drag delta.
/// When `alt_held` is true, points that are merged (same position) move together.
pub fn apply_bezier_transform(
    handle: GizmoHandle,
    orig: &BezierTransform,
    delta: (f32, f32),
    snap: bool,
    alt_held: bool,
) -> BezierTransform {
    let mut result = *orig;

    let snap_to_grid = |p: (f32, f32)| -> (f32, f32) {
        if snap {
            ((p.0 / 10.0).round() * 10.0, (p.1 / 10.0).round() * 10.0)
        } else {
            p
        }
    };

    // Snap point to nearby target points (magnetic snap)
    let snap_to_points = |p: (f32, f32), targets: &[(f32, f32)]| -> (f32, f32) {
        for &target in targets {
            let d = dist(p, target);
            if d < BEZIER_POINT_SNAP_THRESHOLD && d > 0.001 {
                return target;
            }
        }
        p
    };

    // Check if two points are merged (same position)
    let is_merged = |a: (f32, f32), b: (f32, f32)| -> bool {
        dist(a, b) < BEZIER_MERGED_THRESHOLD
    };

    match handle {
        GizmoHandle::BezierStart => {
            let new_pos = (orig.start.0 + delta.0, orig.start.1 + delta.1);
            let snapped = snap_to_points(new_pos, &[result.control1, result.control2, result.end]);
            result.start = snap_to_grid(snapped);

            // Alt: move merged points together
            if alt_held {
                if is_merged(orig.start, orig.control1) {
                    result.control1 = result.start;
                }
                if is_merged(orig.start, orig.control2) {
                    result.control2 = result.start;
                }
                if is_merged(orig.start, orig.end) {
                    result.end = result.start;
                }
            }
        }
        GizmoHandle::BezierControl1 => {
            let new_pos = (orig.control1.0 + delta.0, orig.control1.1 + delta.1);
            let snapped = snap_to_points(new_pos, &[result.start, result.control2, result.end]);
            result.control1 = snap_to_grid(snapped);

            // Alt: move merged points together
            if alt_held {
                if is_merged(orig.control1, orig.start) {
                    result.start = result.control1;
                }
                if is_merged(orig.control1, orig.control2) {
                    result.control2 = result.control1;
                }
                if is_merged(orig.control1, orig.end) {
                    result.end = result.control1;
                }
            }
        }
        GizmoHandle::BezierControl2 => {
            let new_pos = (orig.control2.0 + delta.0, orig.control2.1 + delta.1);
            let snapped = snap_to_points(new_pos, &[result.start, result.control1, result.end]);
            result.control2 = snap_to_grid(snapped);

            // Alt: move merged points together
            if alt_held {
                if is_merged(orig.control2, orig.start) {
                    result.start = result.control2;
                }
                if is_merged(orig.control2, orig.control1) {
                    result.control1 = result.control2;
                }
                if is_merged(orig.control2, orig.end) {
                    result.end = result.control2;
                }
            }
        }
        GizmoHandle::BezierEnd => {
            let new_pos = (orig.end.0 + delta.0, orig.end.1 + delta.1);
            let snapped = snap_to_points(new_pos, &[result.start, result.control1, result.control2]);
            result.end = snap_to_grid(snapped);

            // Alt: move merged points together
            if alt_held {
                if is_merged(orig.end, orig.start) {
                    result.start = result.end;
                }
                if is_merged(orig.end, orig.control1) {
                    result.control1 = result.end;
                }
                if is_merged(orig.end, orig.control2) {
                    result.control2 = result.end;
                }
            }
        }
        GizmoHandle::BezierMoveFree => {
            // Move all points together
            let new_start = (orig.start.0 + delta.0, orig.start.1 + delta.1);
            let new_ctrl1 = (orig.control1.0 + delta.0, orig.control1.1 + delta.1);
            let new_ctrl2 = (orig.control2.0 + delta.0, orig.control2.1 + delta.1);
            let new_end = (orig.end.0 + delta.0, orig.end.1 + delta.1);

            if snap {
                // Snap center, then move all points by the same delta
                let old_center = orig.center();
                let new_center = snap_to_grid((old_center.0 + delta.0, old_center.1 + delta.1));
                let snap_delta = (new_center.0 - old_center.0, new_center.1 - old_center.1);
                result.start = (orig.start.0 + snap_delta.0, orig.start.1 + snap_delta.1);
                result.control1 = (orig.control1.0 + snap_delta.0, orig.control1.1 + snap_delta.1);
                result.control2 = (orig.control2.0 + snap_delta.0, orig.control2.1 + snap_delta.1);
                result.end = (orig.end.0 + snap_delta.0, orig.end.1 + snap_delta.1);
            } else {
                result.start = new_start;
                result.control1 = new_ctrl1;
                result.control2 = new_ctrl2;
                result.end = new_end;
            }
        }
        _ => {}
    }

    result
}

// ============================================================================
// Line Gizmo Functions
// ============================================================================

/// Generate line gizmo with start/end point handles and center move handle.
pub fn generate_line_gizmo(
    transform: &LineTransform,
    zoom: f32,
    hovered_handle: Option<GizmoHandle>,
) -> GizmoRenderData {
    use constants::*;
    let mut data = GizmoRenderData::default();
    let scale = 1.0 / zoom;

    let start = transform.start;
    let end = transform.end;
    let center = transform.center();

    // Endpoint handles (squares)
    let ep_size = BEZIER_ENDPOINT_SIZE * scale;
    let start_color = if hovered_handle == Some(GizmoHandle::LineStart) { COLOR_HOVER } else { COLOR_BEZIER_ENDPOINT };
    let end_color = if hovered_handle == Some(GizmoHandle::LineEnd) { COLOR_HOVER } else { COLOR_BEZIER_ENDPOINT };
    data.rects.push(RectInstance::new(start, (ep_size / 2.0, ep_size / 2.0), 0.0, start_color, Color::new(50, 50, 50, 255), scale));
    data.rects.push(RectInstance::new(end, (ep_size / 2.0, ep_size / 2.0), 0.0, end_color, Color::new(50, 50, 50, 255), scale));

    // Center move handle (diamond - 45° rotated square)
    let center_size = BEZIER_CENTER_SIZE * scale;
    let center_color = if hovered_handle == Some(GizmoHandle::LineMoveFree) { COLOR_HOVER } else { COLOR_BEZIER_CENTER };
    data.rects.push(RectInstance::new(center, (center_size / 2.0, center_size / 2.0), 45.0, center_color, Color::new(60, 60, 60, 255), 1.5 * scale));

    data
}

/// Hit test line gizmo handles.
pub fn hit_test_line_gizmo(transform: &LineTransform, mouse: (f32, f32), zoom: f32) -> Option<GizmoHandle> {
    use constants::*;
    let scale = 1.0 / zoom;
    let tol = HIT_TOLERANCE * scale;

    let start = transform.start;
    let end = transform.end;
    let center = transform.center();

    // Center handle (highest priority for move)
    let center_size = BEZIER_CENTER_SIZE * scale;
    if dist(mouse, center) <= center_size / 2.0 + tol * 0.5 {
        return Some(GizmoHandle::LineMoveFree);
    }

    // Start endpoint
    let ep_size = BEZIER_ENDPOINT_SIZE * scale;
    if (mouse.0 - start.0).abs() <= ep_size / 2.0 + tol * 0.5
        && (mouse.1 - start.1).abs() <= ep_size / 2.0 + tol * 0.5
    {
        return Some(GizmoHandle::LineStart);
    }

    // End endpoint
    if (mouse.0 - end.0).abs() <= ep_size / 2.0 + tol * 0.5
        && (mouse.1 - end.1).abs() <= ep_size / 2.0 + tol * 0.5
    {
        return Some(GizmoHandle::LineEnd);
    }

    None
}

/// Apply transform to line based on handle and drag delta.
pub fn apply_line_transform(
    handle: GizmoHandle,
    orig: &LineTransform,
    delta: (f32, f32),
    snap: bool,
) -> LineTransform {
    let mut result = *orig;

    let snap_to_grid = |p: (f32, f32)| -> (f32, f32) {
        if snap {
            ((p.0 / 10.0).round() * 10.0, (p.1 / 10.0).round() * 10.0)
        } else {
            p
        }
    };

    match handle {
        GizmoHandle::LineStart => {
            let new_pos = (orig.start.0 + delta.0, orig.start.1 + delta.1);
            result.start = snap_to_grid(new_pos);
        }
        GizmoHandle::LineEnd => {
            let new_pos = (orig.end.0 + delta.0, orig.end.1 + delta.1);
            result.end = snap_to_grid(new_pos);
        }
        GizmoHandle::LineMoveFree => {
            // Move both points together
            let new_start = (orig.start.0 + delta.0, orig.start.1 + delta.1);
            let new_end = (orig.end.0 + delta.0, orig.end.1 + delta.1);

            if snap {
                // Snap center, then move both points by the same delta
                let old_center = orig.center();
                let new_center = snap_to_grid((old_center.0 + delta.0, old_center.1 + delta.1));
                let snap_delta = (new_center.0 - old_center.0, new_center.1 - old_center.1);
                result.start = (orig.start.0 + snap_delta.0, orig.start.1 + snap_delta.1);
                result.end = (orig.end.0 + snap_delta.0, orig.end.1 + snap_delta.1);
            } else {
                result.start = new_start;
                result.end = new_end;
            }
        }
        _ => {}
    }

    result
}
