//! Snap system for guideline and axis snapping.
//!
//! Provides:
//! - SnapTarget trait for abstracting snap targets (guidelines, world axes)
//! - SnapConfig resource for global snap settings
//! - Utility functions for snap calculations

use bevy::prelude::*;

/// Snap target abstraction for guidelines, world axes, etc.
pub trait SnapTarget {
    /// Project a point onto this target, returning the closest point on the target.
    fn project_point(&self, point: Vec2) -> Vec2;

    /// Calculate perpendicular distance from a point to this target.
    fn perpendicular_distance(&self, point: Vec2) -> f32;

    /// Get the snap distance threshold.
    fn snap_distance(&self) -> f32;

    /// Get the ruler interval for shift-snap.
    fn ruler_interval(&self) -> f32;

    /// Check if this target is enabled.
    fn is_enabled(&self) -> bool;

    /// Get the direction vector of this target (for distance lines).
    fn direction(&self) -> Vec2;

    /// Get start point of the target line.
    fn start(&self) -> Vec2;

    /// Get end point of the target line.
    fn end(&self) -> Vec2;
}

/// Line-based snap target (for guidelines and world axes).
#[derive(Debug, Clone)]
pub struct LineSnapTarget {
    pub start: Vec2,
    pub end: Vec2,
    pub snap_distance: f32,
    pub ruler_interval: f32,
    pub enabled: bool,
    /// If true, treat as infinite line (perpendicular distance only).
    /// If false, treat as line segment (distance to nearest point on segment).
    pub is_infinite: bool,
}

impl LineSnapTarget {
    /// Create a new line snap target (defaults to infinite line for guidelines).
    pub fn new(start: Vec2, end: Vec2, snap_distance: f32, ruler_interval: f32) -> Self {
        Self {
            start,
            end,
            snap_distance,
            ruler_interval,
            enabled: true,
            is_infinite: true, // Default to infinite for backwards compatibility with guidelines
        }
    }

    /// Create a line segment snap target (finite, uses segment distance).
    pub fn new_segment(start: Vec2, end: Vec2, snap_distance: f32, ruler_interval: f32) -> Self {
        Self {
            start,
            end,
            snap_distance,
            ruler_interval,
            enabled: true,
            is_infinite: false,
        }
    }

    /// Create a horizontal guideline at the given Y position.
    pub fn horizontal(y: f32, extent: f32, snap_distance: f32, ruler_interval: f32) -> Self {
        Self::new(
            Vec2::new(-extent, y),
            Vec2::new(extent, y),
            snap_distance,
            ruler_interval,
        )
    }

    /// Create a vertical guideline at the given X position.
    pub fn vertical(x: f32, extent: f32, snap_distance: f32, ruler_interval: f32) -> Self {
        Self::new(
            Vec2::new(x, -extent),
            Vec2::new(x, extent),
            snap_distance,
            ruler_interval,
        )
    }
}

impl SnapTarget for LineSnapTarget {
    fn project_point(&self, point: Vec2) -> Vec2 {
        project_point_to_line(point, self.start, self.end)
    }

    fn perpendicular_distance(&self, point: Vec2) -> f32 {
        if self.is_infinite {
            perpendicular_distance_to_line(point, self.start, self.end)
        } else {
            distance_to_line_segment(point, self.start, self.end)
        }
    }

    fn snap_distance(&self) -> f32 {
        self.snap_distance
    }

    fn ruler_interval(&self) -> f32 {
        self.ruler_interval
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn direction(&self) -> Vec2 {
        (self.end - self.start).normalize_or_zero()
    }

    fn start(&self) -> Vec2 {
        self.start
    }

    fn end(&self) -> Vec2 {
        self.end
    }
}

/// Global snap configuration resource.
#[derive(Resource, Debug, Clone)]
pub struct SnapConfig {
    /// Global snap enable/disable toggle.
    pub global_snap_enabled: bool,
    /// Whether to show distance lines to nearby snap targets.
    pub show_distance_lines: bool,
    /// Distance threshold for showing distance lines (meters).
    pub distance_line_threshold: f32,
    /// Whether Shift+drag snaps to ruler intervals.
    pub shift_snap_to_ruler: bool,
    /// World X axis (Y=0) enabled.
    pub world_axis_x_enabled: bool,
    /// World Y axis (X=0) enabled.
    pub world_axis_y_enabled: bool,
    /// Ruler interval for world axes (meters).
    pub world_axis_ruler_interval: f32,
    /// Snap distance for world axes (meters).
    pub world_axis_snap_distance: f32,

    // Grid and angle snap settings
    /// Grid snap enable/disable toggle.
    pub grid_snap_enabled: bool,
    /// Grid snap interval (meters).
    pub grid_snap_interval: f32,
    /// Angle snap enable/disable toggle.
    pub angle_snap_enabled: bool,
    /// Angle snap interval (degrees).
    pub angle_snap_interval: f32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            global_snap_enabled: true,
            show_distance_lines: true,
            distance_line_threshold: 2.0,
            shift_snap_to_ruler: true,
            world_axis_x_enabled: false,
            world_axis_y_enabled: false,
            world_axis_ruler_interval: 1.0,
            world_axis_snap_distance: 0.15,
            // Grid and angle snap defaults
            grid_snap_enabled: true,
            grid_snap_interval: 0.05,
            angle_snap_enabled: true,
            angle_snap_interval: 0.5,
        }
    }
}

/// Information about a distance line to render.
#[derive(Debug, Clone)]
pub struct DistanceLine {
    /// Start point (object position).
    pub from: Vec2,
    /// End point (projected point on target).
    pub to: Vec2,
    /// Perpendicular distance.
    pub distance: f32,
    /// Index of the snap target.
    pub target_index: usize,
}

/// Project a point onto a line segment, returning the closest point.
pub fn project_point_to_line(point: Vec2, line_start: Vec2, line_end: Vec2) -> Vec2 {
    let line = line_end - line_start;
    let len_sq = line.length_squared();

    if len_sq < 0.0001 {
        return line_start;
    }

    // Project point onto infinite line, then clamp to segment
    let t = ((point - line_start).dot(line) / len_sq).clamp(0.0, 1.0);
    line_start + t * line
}

/// Calculate perpendicular distance from a point to a line (infinite line, not segment).
pub fn perpendicular_distance_to_line(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line = line_end - line_start;
    let len = line.length();

    if len < 0.0001 {
        return point.distance(line_start);
    }

    // Cross product gives signed area of parallelogram, divide by base for height
    let cross = (point.x - line_start.x) * (line_end.y - line_start.y)
        - (point.y - line_start.y) * (line_end.x - line_start.x);

    cross.abs() / len
}

/// Calculate shortest distance from a point to a line segment.
/// Returns perpendicular distance if within segment bounds,
/// otherwise returns distance to nearest endpoint.
pub fn distance_to_line_segment(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line = line_end - line_start;
    let len_sq = line.length_squared();

    if len_sq < 0.0001 {
        return point.distance(line_start);
    }

    // Project point onto line, get parameter t
    let t = (point - line_start).dot(line) / len_sq;

    if t <= 0.0 {
        // Point is before line_start
        point.distance(line_start)
    } else if t >= 1.0 {
        // Point is after line_end
        point.distance(line_end)
    } else {
        // Point is within segment bounds - use perpendicular distance
        let projected = line_start + t * line;
        point.distance(projected)
    }
}

/// Find the best snap point from a list of targets.
///
/// Returns (snapped_position, target_index) if a snap target is within range.
pub fn find_snap_point(point: Vec2, targets: &[&dyn SnapTarget]) -> Option<(Vec2, usize)> {
    let mut best_dist = f32::MAX;
    let mut best_snap = None;

    for (idx, target) in targets.iter().enumerate() {
        if !target.is_enabled() {
            continue;
        }

        let dist = target.perpendicular_distance(point);
        if dist < target.snap_distance() && dist < best_dist {
            best_dist = dist;
            best_snap = Some((target.project_point(point), idx));
        }
    }

    best_snap
}

/// Snap a point to the ruler interval of a target.
///
/// Projects the point onto the target and rounds to the nearest ruler tick.
pub fn snap_to_ruler_interval(point: Vec2, target: &dyn SnapTarget) -> Vec2 {
    let projected = target.project_point(point);
    let direction = target.direction();
    let start = target.start();
    let interval = target.ruler_interval();

    // Calculate distance along the line from start
    let distance_along = (projected - start).dot(direction);

    // Round to nearest interval
    let snapped_distance = (distance_along / interval).round() * interval;

    // Calculate final position
    start + direction * snapped_distance
}

/// Calculate distance lines from a point to all nearby snap targets.
///
/// Only includes targets within the threshold distance.
pub fn calculate_distance_lines(
    point: Vec2,
    targets: &[&dyn SnapTarget],
    threshold: f32,
) -> Vec<DistanceLine> {
    let mut lines = Vec::new();

    for (idx, target) in targets.iter().enumerate() {
        if !target.is_enabled() {
            continue;
        }

        let dist = target.perpendicular_distance(point);
        if dist <= threshold {
            let projected = target.project_point(point);
            lines.push(DistanceLine {
                from: point,
                to: projected,
                distance: dist,
                target_index: idx,
            });
        }
    }

    lines
}

/// Find the nearest enabled snap target to a point.
pub fn find_nearest_target<'a>(
    point: Vec2,
    targets: &[&'a dyn SnapTarget],
) -> Option<&'a dyn SnapTarget> {
    let mut best_dist = f32::MAX;
    let mut best_target = None;

    for target in targets.iter() {
        if !target.is_enabled() {
            continue;
        }

        let dist = target.perpendicular_distance(point);
        if dist < best_dist {
            best_dist = dist;
            best_target = Some(*target);
        }
    }

    best_target
}

/// Snap a position to a grid interval.
///
/// Rounds each coordinate to the nearest multiple of the interval.
pub fn snap_to_grid(pos: Vec2, interval: f32) -> Vec2 {
    if interval <= 0.0 {
        return pos;
    }
    Vec2::new(
        (pos.x / interval).round() * interval,
        (pos.y / interval).round() * interval,
    )
}

/// Snap an angle (in degrees) to the nearest multiple of the interval.
pub fn snap_angle(angle_deg: f32, interval: f32) -> f32 {
    if interval <= 0.0 {
        return angle_deg;
    }
    (angle_deg / interval).round() * interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_point_to_line() {
        let start = Vec2::new(0.0, 0.0);
        let end = Vec2::new(10.0, 0.0);

        // Point above the line
        let point = Vec2::new(5.0, 3.0);
        let projected = project_point_to_line(point, start, end);
        assert!((projected - Vec2::new(5.0, 0.0)).length() < 0.001);

        // Point at start
        let point = Vec2::new(-2.0, 1.0);
        let projected = project_point_to_line(point, start, end);
        assert!((projected - start).length() < 0.001);

        // Point at end
        let point = Vec2::new(12.0, 1.0);
        let projected = project_point_to_line(point, start, end);
        assert!((projected - end).length() < 0.001);
    }

    #[test]
    fn test_perpendicular_distance() {
        let start = Vec2::new(0.0, 0.0);
        let end = Vec2::new(10.0, 0.0);

        let point = Vec2::new(5.0, 3.0);
        let dist = perpendicular_distance_to_line(point, start, end);
        assert!((dist - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_snap_to_ruler_interval() {
        let target = LineSnapTarget::horizontal(0.0, 100.0, 0.15, 0.5);

        let point = Vec2::new(1.3, 0.2);
        let snapped = snap_to_ruler_interval(point, &target);
        assert!((snapped.x - 1.5).abs() < 0.001);
        assert!(snapped.y.abs() < 0.001);
    }

    #[test]
    fn test_distance_to_line_segment() {
        let start = Vec2::new(0.0, 0.0);
        let end = Vec2::new(10.0, 0.0);

        // Point above the line (within segment bounds)
        let point = Vec2::new(5.0, 3.0);
        let dist = distance_to_line_segment(point, start, end);
        assert!((dist - 3.0).abs() < 0.001);

        // Point beyond end (should use endpoint distance)
        let point = Vec2::new(13.0, 4.0);
        let dist = distance_to_line_segment(point, start, end);
        let expected = point.distance(end); // √(9 + 16) = 5
        assert!((dist - expected).abs() < 0.001);

        // Point before start (should use endpoint distance)
        let point = Vec2::new(-3.0, 4.0);
        let dist = distance_to_line_segment(point, start, end);
        let expected = point.distance(start); // √(9 + 16) = 5
        assert!((dist - expected).abs() < 0.001);
    }

    #[test]
    fn test_line_snap_target_segment_uses_segment_distance() {
        // Bug scenario: point P(10, 1) with two finite line segments
        // A: horizontal segment from (0,0) to (5,0)
        // B: vertical segment from (8,0) to (8,5)
        // P is beyond A's segment, so B should be closer

        let horizontal = LineSnapTarget::new_segment(
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 0.0),
            0.5,
            1.0,
        );
        let vertical = LineSnapTarget::new_segment(
            Vec2::new(8.0, 0.0),
            Vec2::new(8.0, 5.0),
            0.5,
            1.0,
        );

        let point = Vec2::new(10.0, 1.0);

        // Distance to horizontal: P is beyond the segment end (5,0)
        // Closest point on segment is (5,0), distance = √((10-5)² + (1-0)²) = √26 ≈ 5.1
        let dist_h = horizontal.perpendicular_distance(point);
        let expected_h = point.distance(Vec2::new(5.0, 0.0));
        assert!((dist_h - expected_h).abs() < 0.001);

        // Distance to vertical: P is within segment Y range (0 to 5), X distance = |10-8| = 2
        let dist_v = vertical.perpendicular_distance(point);
        assert!((dist_v - 2.0).abs() < 0.001);

        // Vertical should be closer than horizontal
        assert!(dist_v < dist_h, "vertical ({}) should be closer than horizontal ({})", dist_v, dist_h);
    }

    #[test]
    fn test_line_snap_target_infinite_uses_perpendicular_distance() {
        // Guidelines (infinite lines) should use perpendicular distance only
        // Even when point is beyond segment bounds

        let guideline = LineSnapTarget::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 10.0),  // Vertical guideline at X=0, from Y=0 to Y=10
            0.5,
            1.0,
        );

        // Point at (0.2, 17.5) - beyond the segment Y range
        let point = Vec2::new(0.2, 17.5);

        // For infinite line, distance should be perpendicular (X distance = 0.2)
        let dist = guideline.perpendicular_distance(point);
        assert!((dist - 0.2).abs() < 0.001, "Expected ~0.2, got {}", dist);
    }
}
