//! Rendering systems for the marble game.
//!
//! Uses Bevy's Gizmos API for debug-style rendering of shapes.
//! This provides immediate feedback while a more sophisticated
//! renderer can be added later.

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};

use crate::bevy::{GameCamera, MainCamera, MapConfig, Marble, MarbleVisual};
use crate::map::{EvaluatedShape, ObjectRole};

/// Gizmo configuration resource for shape rendering.
#[derive(Resource)]
pub struct ShapeGizmoConfig {
    /// Color for obstacle shapes.
    pub obstacle_color: Color,
    /// Color for trigger zones.
    pub trigger_color: Color,
    /// Color for spawner zones.
    pub spawner_color: Color,
    /// Color for guideline (default cyan).
    pub guideline_color: Color,
    /// Color for ruler ticks.
    pub ruler_tick_color: Color,
    /// Line width for shapes.
    pub line_width: f32,
}

impl Default for ShapeGizmoConfig {
    fn default() -> Self {
        Self {
            obstacle_color: Color::srgb(0.8, 0.8, 0.8),
            trigger_color: Color::srgba(0.2, 0.8, 0.2, 0.5),
            spawner_color: Color::srgba(0.2, 0.2, 0.8, 0.5),
            guideline_color: Color::srgba(0.5, 0.8, 0.9, 0.2),
            ruler_tick_color: Color::srgba(0.8, 0.8, 0.8, 0.15),
            line_width: 2.0,
        }
    }
}

/// Grid rendering configuration.
#[derive(Resource)]
pub struct GridConfig {
    /// Whether to show the grid.
    pub enabled: bool,
    /// Grid cell size.
    pub cell_size: f32,
    /// Color for regular grid lines.
    pub line_color: Color,
    /// Color for major grid lines (every 10 units).
    pub major_line_color: Color,
    /// Color for X axis (at y=0).
    pub x_axis_color: Color,
    /// Color for Y axis (at x=0).
    pub y_axis_color: Color,
    /// How far to extend the grid from camera center.
    pub extent: f32,
    /// Tick mark length for regular ticks.
    pub tick_length_small: f32,
    /// Tick mark length for 5-unit ticks.
    pub tick_length_medium: f32,
    /// Tick mark length for 10-unit ticks.
    pub tick_length_large: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 1.0,
            line_color: Color::srgba(0.3, 0.3, 0.3, 0.2),
            major_line_color: Color::srgba(0.4, 0.4, 0.4, 0.4),
            x_axis_color: Color::srgba(0.7, 0.2, 0.2, 0.7),
            y_axis_color: Color::srgba(0.2, 0.7, 0.2, 0.7),
            extent: 50.0,
            tick_length_small: 0.03,
            tick_length_medium: 0.06,
            tick_length_large: 0.1,
        }
    }
}

/// Marker component for grid label entities.
#[derive(Component)]
pub struct GridLabel;

/// Resource to track grid label state.
#[derive(Resource, Default)]
pub struct GridLabelState {
    pub last_camera_pos: Vec2,
    pub last_extent: f32,
}

/// Marker component for grid mesh entity.
#[derive(Component)]
pub struct GridMesh;

/// Resource to track grid mesh state.
#[derive(Resource, Default)]
pub struct GridMeshState {
    pub last_camera_pos: Vec2,
    pub last_extent: f32,
    /// All mesh handles for the grid (to be removed before recreation)
    pub mesh_handles: Vec<Handle<Mesh>>,
    /// All material handles for the grid (to be removed before recreation)
    pub material_handles: Vec<Handle<ColorMaterial>>,
}


/// System to render coordinate grid using mesh.
pub fn render_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Option<Res<GridConfig>>,
    camera_query: Query<(&GameCamera, &GlobalTransform), With<MainCamera>>,
    existing_grid: Query<Entity, With<GridMesh>>,
    mut grid_state: ResMut<GridMeshState>,
) {
    let default_config = GridConfig::default();
    let grid_config = config.map(|c| c.into_inner()).unwrap_or(&default_config);

    // Remove grid if disabled
    if !grid_config.enabled {
        for entity in existing_grid.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Get camera position
    let camera_pos = camera_query
        .single()
        .map(|(_, transform)| transform.translation().truncate())
        .unwrap_or(Vec2::ZERO);

    let extent = grid_config.extent;

    // Only rebuild if camera moved significantly
    let moved = (camera_pos - grid_state.last_camera_pos).length() > 5.0
        || (extent - grid_state.last_extent).abs() > 1.0;

    if !moved && !existing_grid.is_empty() {
        return;
    }

    grid_state.last_camera_pos = camera_pos;
    grid_state.last_extent = extent;

    // Remove old grid entities
    for entity in existing_grid.iter() {
        commands.entity(entity).despawn();
    }

    // Remove old mesh and material assets to prevent memory leak
    for handle in grid_state.mesh_handles.drain(..) {
        meshes.remove(&handle);
    }
    for handle in grid_state.material_handles.drain(..) {
        materials.remove(&handle);
    }

    let cell_size = grid_config.cell_size;
    let line_width = 0.01;
    let axis_width = 0.02;
    let major_line_width = 0.015;

    // Calculate grid bounds
    let min_x = ((camera_pos.x - extent) / cell_size).floor() * cell_size;
    let max_x = ((camera_pos.x + extent) / cell_size).ceil() * cell_size;
    let min_y = ((camera_pos.y - extent) / cell_size).floor() * cell_size;
    let max_y = ((camera_pos.y + extent) / cell_size).ceil() * cell_size;

    let grid_z = -10.0; // Behind everything

    // Build mesh for regular grid lines
    let (grid_mesh, grid_color) = build_grid_mesh(
        min_x, max_x, min_y, max_y, cell_size, line_width, major_line_width,
        grid_config.line_color, grid_config.major_line_color,
    );

    let grid_mesh_handle = meshes.add(grid_mesh);
    let grid_material_handle = materials.add(ColorMaterial::from(grid_color));

    // Store handles for later cleanup
    grid_state.mesh_handles.push(grid_mesh_handle.clone());
    grid_state.material_handles.push(grid_material_handle.clone());

    commands.spawn((
        GridMesh,
        Mesh2d(grid_mesh_handle),
        MeshMaterial2d(grid_material_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, grid_z)),
    ));

    // Build X axis (y=0)
    let x_axis_mesh = build_line_mesh(min_x, max_x, 0.0, 0.0, axis_width, true);
    let x_axis_handle = meshes.add(x_axis_mesh);
    let x_axis_material = materials.add(ColorMaterial::from(grid_config.x_axis_color));

    grid_state.mesh_handles.push(x_axis_handle.clone());
    grid_state.material_handles.push(x_axis_material.clone());

    commands.spawn((
        GridMesh,
        Mesh2d(x_axis_handle),
        MeshMaterial2d(x_axis_material),
        Transform::from_translation(Vec3::new(0.0, 0.0, grid_z + 0.1)),
    ));

    // Build Y axis (x=0)
    let y_axis_mesh = build_line_mesh(0.0, 0.0, min_y, max_y, axis_width, false);
    let y_axis_handle = meshes.add(y_axis_mesh);
    let y_axis_material = materials.add(ColorMaterial::from(grid_config.y_axis_color));

    grid_state.mesh_handles.push(y_axis_handle.clone());
    grid_state.material_handles.push(y_axis_material.clone());

    commands.spawn((
        GridMesh,
        Mesh2d(y_axis_handle),
        MeshMaterial2d(y_axis_material),
        Transform::from_translation(Vec3::new(0.0, 0.0, grid_z + 0.1)),
    ));

    // Build tick marks
    let tick_interval = cell_size / 10.0;
    let (x_ticks_mesh, y_ticks_mesh) = build_tick_meshes(
        min_x, max_x, min_y, max_y, tick_interval,
        grid_config.tick_length_small,
        grid_config.tick_length_medium,
        grid_config.tick_length_large,
    );

    let x_ticks_handle = meshes.add(x_ticks_mesh);
    let x_ticks_material = materials.add(ColorMaterial::from(grid_config.x_axis_color));

    grid_state.mesh_handles.push(x_ticks_handle.clone());
    grid_state.material_handles.push(x_ticks_material.clone());

    commands.spawn((
        GridMesh,
        Mesh2d(x_ticks_handle),
        MeshMaterial2d(x_ticks_material),
        Transform::from_translation(Vec3::new(0.0, 0.0, grid_z + 0.2)),
    ));

    let y_ticks_handle = meshes.add(y_ticks_mesh);
    let y_ticks_material = materials.add(ColorMaterial::from(grid_config.y_axis_color));

    grid_state.mesh_handles.push(y_ticks_handle.clone());
    grid_state.material_handles.push(y_ticks_material.clone());

    commands.spawn((
        GridMesh,
        Mesh2d(y_ticks_handle),
        MeshMaterial2d(y_ticks_material),
        Transform::from_translation(Vec3::new(0.0, 0.0, grid_z + 0.2)),
    ));
}

/// Build a mesh for grid lines.
fn build_grid_mesh(
    min_x: f32, max_x: f32, min_y: f32, max_y: f32,
    cell_size: f32, line_width: f32, major_line_width: f32,
    line_color: Color, _major_color: Color,
) -> (Mesh, Color) {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let half_width = line_width / 2.0;
    let half_major = major_line_width / 2.0;

    // Vertical lines
    let mut x = min_x;
    while x <= max_x {
        let x_int = (x / cell_size).round() as i32;
        let is_axis = x.abs() < 0.001;

        if !is_axis {
            let hw = if x_int % 10 == 0 { half_major } else { half_width };
            add_line_quad(&mut positions, &mut indices, x - hw, min_y, x + hw, max_y);
        }
        x += cell_size;
    }

    // Horizontal lines
    let mut y = min_y;
    while y <= max_y {
        let y_int = (y / cell_size).round() as i32;
        let is_axis = y.abs() < 0.001;

        if !is_axis {
            let hw = if y_int % 10 == 0 { half_major } else { half_width };
            add_line_quad(&mut positions, &mut indices, min_x, y - hw, max_x, y + hw);
        }
        y += cell_size;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));

    (mesh, line_color)
}

/// Build a single line mesh.
fn build_line_mesh(x1: f32, x2: f32, y1: f32, y2: f32, width: f32, horizontal: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let hw = width / 2.0;
    if horizontal {
        add_line_quad(&mut positions, &mut indices, x1, y1 - hw, x2, y2 + hw);
    } else {
        add_line_quad(&mut positions, &mut indices, x1 - hw, y1, x2 + hw, y2);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Build tick mark meshes for both axes.
fn build_tick_meshes(
    min_x: f32, max_x: f32, min_y: f32, max_y: f32,
    tick_interval: f32,
    tick_small: f32, tick_medium: f32, tick_large: f32,
) -> (Mesh, Mesh) {
    let tick_width = 0.008;
    let hw = tick_width / 2.0;

    // X axis ticks (vertical lines on y=0)
    let mut x_positions: Vec<[f32; 3]> = Vec::new();
    let mut x_indices: Vec<u32> = Vec::new();

    let mut x = (min_x / tick_interval).floor() * tick_interval;
    while x <= max_x {
        let tick_index = (x / tick_interval).round() as i32;
        if x.abs() > 0.0001 {
            let tick_length = if tick_index % 10 == 0 {
                tick_large
            } else if tick_index % 5 == 0 {
                tick_medium
            } else {
                tick_small
            };
            add_line_quad(&mut x_positions, &mut x_indices, x - hw, -tick_length, x + hw, tick_length);
        }
        x += tick_interval;
    }

    let mut x_mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    x_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, x_positions);
    x_mesh.insert_indices(Indices::U32(x_indices));

    // Y axis ticks (horizontal lines on x=0)
    let mut y_positions: Vec<[f32; 3]> = Vec::new();
    let mut y_indices: Vec<u32> = Vec::new();

    let mut y = (min_y / tick_interval).floor() * tick_interval;
    while y <= max_y {
        let tick_index = (y / tick_interval).round() as i32;
        if y.abs() > 0.0001 {
            let tick_length = if tick_index % 10 == 0 {
                tick_large
            } else if tick_index % 5 == 0 {
                tick_medium
            } else {
                tick_small
            };
            add_line_quad(&mut y_positions, &mut y_indices, -tick_length, y - hw, tick_length, y + hw);
        }
        y += tick_interval;
    }

    let mut y_mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    y_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, y_positions);
    y_mesh.insert_indices(Indices::U32(y_indices));

    (x_mesh, y_mesh)
}

/// Add a quad (rectangle) to the mesh.
fn add_line_quad(positions: &mut Vec<[f32; 3]>, indices: &mut Vec<u32>, x1: f32, y1: f32, x2: f32, y2: f32) {
    let base = positions.len() as u32;

    positions.push([x1, y1, 0.0]);
    positions.push([x2, y1, 0.0]);
    positions.push([x2, y2, 0.0]);
    positions.push([x1, y2, 0.0]);

    indices.push(base);
    indices.push(base + 1);
    indices.push(base + 2);
    indices.push(base);
    indices.push(base + 2);
    indices.push(base + 3);
}

/// System to manage grid labels (numbers at each grid line).
pub fn manage_grid_labels(
    mut commands: Commands,
    config: Option<Res<GridConfig>>,
    camera_query: Query<(&GameCamera, &GlobalTransform), With<MainCamera>>,
    existing_labels: Query<Entity, With<GridLabel>>,
    mut label_state: ResMut<GridLabelState>,
) {
    let default_config = GridConfig::default();
    let grid_config = config.map(|c| c.into_inner()).unwrap_or(&default_config);

    if !grid_config.enabled {
        for entity in existing_labels.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let camera_pos = camera_query
        .single()
        .map(|(_, transform)| transform.translation().truncate())
        .unwrap_or(Vec2::ZERO);

    let extent = grid_config.extent;

    // Only update labels if camera moved significantly
    let moved = (camera_pos - label_state.last_camera_pos).length() > 5.0
        || (extent - label_state.last_extent).abs() > 1.0;

    if !moved && !existing_labels.is_empty() {
        return;
    }

    label_state.last_camera_pos = camera_pos;
    label_state.last_extent = extent;

    // Remove old labels
    for entity in existing_labels.iter() {
        commands.entity(entity).despawn();
    }

    let cell_size = grid_config.cell_size;
    let min_x = ((camera_pos.x - extent) / cell_size).floor() as i32;
    let max_x = ((camera_pos.x + extent) / cell_size).ceil() as i32;
    let min_y = ((camera_pos.y - extent) / cell_size).floor() as i32;
    let max_y = ((camera_pos.y + extent) / cell_size).ceil() as i32;

    let label_color = Color::srgba(0.6, 0.6, 0.6, 0.9);
    let label_offset = 0.2;
    let text_scale = 0.005; // Scale down text to world units
    let label_z = -100.0; // Behind everything

    // Spawn X axis labels (every grid line)
    for x in min_x..=max_x {
        if x == 0 {
            continue;
        }
        let world_x = x as f32 * cell_size;
        commands.spawn((
            GridLabel,
            Text2d::new(format!("{}", x)),
            TextFont {
                font_size: 32.0,
                ..default()
            },
            TextColor(label_color),
            Transform::from_translation(Vec3::new(world_x, -label_offset, label_z))
                .with_scale(Vec3::splat(text_scale)),
        ));
    }

    // Spawn Y axis labels (every grid line)
    for y in min_y..=max_y {
        if y == 0 {
            continue;
        }
        let world_y = y as f32 * cell_size;
        commands.spawn((
            GridLabel,
            Text2d::new(format!("{}", y)),
            TextFont {
                font_size: 32.0,
                ..default()
            },
            TextColor(label_color),
            Transform::from_translation(Vec3::new(-label_offset, world_y, label_z))
                .with_scale(Vec3::splat(text_scale)),
        ));
    }

    // Origin label
    commands.spawn((
        GridLabel,
        Text2d::new("0"),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(label_color),
        Transform::from_translation(Vec3::new(-label_offset, -label_offset, label_z))
            .with_scale(Vec3::splat(text_scale)),
    ));
}

/// System to render map objects using gizmos.
///
/// Renders all objects directly from MapConfig to ensure nothing is missed.
pub fn render_map_objects(
    mut gizmos: Gizmos,
    config: Option<Res<ShapeGizmoConfig>>,
    map_config: Option<Res<MapConfig>>,
    animated_objects: Query<(&crate::bevy::MapObjectMarker, &Transform)>,
) {
    let default_config = ShapeGizmoConfig::default();
    let gizmo_config = config.map(|c| c.into_inner()).unwrap_or(&default_config);

    let Some(map_config) = map_config else {
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);

    // Collect animated object transforms by ID
    let mut animated_transforms: std::collections::HashMap<String, &Transform> =
        std::collections::HashMap::new();
    for (marker, transform) in animated_objects.iter() {
        if let Some(ref id) = marker.object_id {
            animated_transforms.insert(id.clone(), transform);
        }
    }

    // Render all objects
    for obj in &map_config.0.objects {
        let color = match obj.role {
            ObjectRole::Obstacle => gizmo_config.obstacle_color,
            ObjectRole::Trigger => gizmo_config.trigger_color,
            ObjectRole::Spawner => gizmo_config.spawner_color,
            ObjectRole::Guideline => {
                // Guidelines get custom color from properties or default
                obj.properties
                    .guideline
                    .as_ref()
                    .and_then(|p| p.color)
                    .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
                    .unwrap_or(gizmo_config.guideline_color)
            }
            ObjectRole::VectorField => {
                // Vector fields rendered with a distinct purple color
                Color::srgba(0.7, 0.3, 0.9, 0.5)
            }
        };

        // Check if this object has an animated transform
        if let Some(id) = &obj.id {
            if let Some(transform) = animated_transforms.get(id) {
                // Use entity's transform for animated objects
                let shape = obj.shape.evaluate(&ctx);
                draw_shape_with_transform(&mut gizmos, &shape, transform, color);
                continue;
            }
        }

        // Static objects: use config directly
        let shape = obj.shape.evaluate(&ctx);
        draw_shape(&mut gizmos, &shape, color);
    }
}

/// Helper function to draw a shape using entity's transform (for animated objects).
fn draw_shape_with_transform(gizmos: &mut Gizmos, shape: &EvaluatedShape, transform: &Transform, color: Color) {
    let pos = transform.translation.truncate();
    let rot = transform.rotation.to_euler(EulerRot::ZYX).0;

    match shape {
        EvaluatedShape::Circle { radius, .. } => {
            gizmos.circle_2d(Isometry2d::from_translation(pos), *radius, color);
        }
        EvaluatedShape::Rect { size, .. } => {
            let rot2d = Rot2::radians(rot);
            let isometry = Isometry2d::new(pos, rot2d);
            gizmos.rect_2d(isometry, Vec2::new(size[0], size[1]), color);
        }
        EvaluatedShape::Line { start, end } => {
            // For lines, rotate around center
            let half_len = Vec2::new(end[0] - start[0], end[1] - start[1]) / 2.0;

            let (sin, cos) = rot.sin_cos();
            let rotated_half = Vec2::new(
                half_len.x * cos - half_len.y * sin,
                half_len.x * sin + half_len.y * cos,
            );

            let new_start = pos - rotated_half;
            let new_end = pos + rotated_half;
            gizmos.line_2d(new_start, new_end, color);
        }
        EvaluatedShape::Bezier { .. } => {
            // For bezier, just draw at transformed position (simplified)
            if let Some(points) = shape.bezier_to_points() {
                for i in 0..points.len().saturating_sub(1) {
                    let p1 = Vec2::new(points[i][0], points[i][1]);
                    let p2 = Vec2::new(points[i + 1][0], points[i + 1][1]);
                    gizmos.line_2d(p1, p2, color);
                }
            }
        }
    }
}

/// System to render marbles using gizmos.
pub fn render_marbles(mut gizmos: Gizmos, marbles: Query<(&Marble, &MarbleVisual, &Transform)>) {
    let count = marbles.iter().count();
    if count > 0 {
        tracing::debug!("[render_marbles] Rendering {} marbles", count);
    }
    for (marble, visual, transform) in marbles.iter() {
        if marble.eliminated {
            continue;
        }

        let color = Color::srgba(
            visual.color.r as f32 / 255.0,
            visual.color.g as f32 / 255.0,
            visual.color.b as f32 / 255.0,
            visual.color.a as f32 / 255.0,
        );

        let pos = transform.translation.truncate();

        // Draw filled circle for marble
        gizmos.circle_2d(Isometry2d::from_translation(pos), visual.radius, color);

        // Draw outline
        gizmos.circle_2d(
            Isometry2d::from_translation(pos),
            visual.radius,
            Color::srgb(0.0, 0.0, 0.0),
        );
    }
}

// Note: Camera update moved to systems/camera/mod.rs (apply_camera_smoothing)

/// Helper function to draw a shape using gizmos.
fn draw_shape(gizmos: &mut Gizmos, shape: &EvaluatedShape, color: Color) {
    match shape {
        EvaluatedShape::Circle { center, radius } => {
            let pos = Vec2::new(center[0], center[1]);
            gizmos.circle_2d(Isometry2d::from_translation(pos), *radius, color);
        }
        EvaluatedShape::Rect {
            center,
            size,
            rotation,
        } => {
            let pos = Vec2::new(center[0], center[1]);
            let rot = Rot2::radians(rotation.to_radians());
            let isometry = Isometry2d::new(pos, rot);
            gizmos.rect_2d(isometry, Vec2::new(size[0], size[1]), color);
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
            // Draw bezier as a series of line segments
            let points = bezier_to_points(start, control1, control2, end, 20);
            for i in 0..points.len() - 1 {
                gizmos.line_2d(points[i], points[i + 1], color);
            }
        }
    }
}

/// Convert bezier curve to line segments.
fn bezier_to_points(
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

/// System to render guidelines with dashed lines and ruler ticks.
pub fn render_guidelines(
    mut gizmos: Gizmos,
    config: Option<Res<ShapeGizmoConfig>>,
    map_config: Option<Res<MapConfig>>,
) {
    let default_config = ShapeGizmoConfig::default();
    let gizmo_config = config.map(|c| c.into_inner()).unwrap_or(&default_config);

    let Some(map_config) = map_config else {
        return;
    };

    let ctx = crate::dsl::GameContext::new(0.0, 0);

    for obj in &map_config.0.objects {
        if obj.role != ObjectRole::Guideline {
            continue;
        }

        let shape = obj.shape.evaluate(&ctx);
        let guideline_props = obj.properties.guideline.as_ref();

        // Get color
        let color = guideline_props
            .and_then(|p| p.color)
            .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
            .unwrap_or(gizmo_config.guideline_color);

        let show_ruler = guideline_props.map(|p| p.show_ruler).unwrap_or(true);
        let ruler_interval = guideline_props.map(|p| p.ruler_interval).unwrap_or(0.5);

        // Draw guideline as dashed line
        match &shape {
            EvaluatedShape::Line { start, end } => {
                let start_pos = Vec2::new(start[0], start[1]);
                let end_pos = Vec2::new(end[0], end[1]);

                // Draw dashed line
                draw_dashed_line(&mut gizmos, start_pos, end_pos, color, 0.1);

                // Draw ruler ticks if enabled
                if show_ruler {
                    draw_ruler_ticks(
                        &mut gizmos,
                        start_pos,
                        end_pos,
                        ruler_interval,
                        gizmo_config.ruler_tick_color,
                    );
                }
            }
            // Other shapes are drawn as solid lines (not typical for guidelines)
            _ => {
                draw_shape(&mut gizmos, &shape, color);
            }
        }
    }
}

/// Draw a dashed line between two points.
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

/// Draw ruler ticks along a guideline.
fn draw_ruler_ticks(
    gizmos: &mut Gizmos,
    start: Vec2,
    end: Vec2,
    interval: f32,
    color: Color,
) {
    let dir = end - start;
    let length = dir.length();

    if length < 0.001 || interval < 0.001 {
        return;
    }

    let dir_normalized = dir / length;
    // Perpendicular direction for tick marks
    let perp = Vec2::new(-dir_normalized.y, dir_normalized.x);

    // Calculate how many ticks to draw
    let num_ticks = (length / interval).floor() as i32 + 1;

    for i in 0..num_ticks {
        let dist = i as f32 * interval;
        if dist > length {
            break;
        }

        let pos = start + dir_normalized * dist;

        // Tick sizes: larger for every 2nd interval (like a ruler)
        let tick_size = if i % 2 == 0 { 0.08 } else { 0.04 };

        // Draw tick mark perpendicular to the guideline
        let tick_start = pos - perp * tick_size;
        let tick_end = pos + perp * tick_size;
        gizmos.line_2d(tick_start, tick_end, color);
    }
}
