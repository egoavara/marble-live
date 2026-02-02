//! Editor camera system.
//!
//! Provides manual pan and zoom control for the map editor.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy::{CameraInputState, CameraMode, GameCamera, MainCamera};

/// System to handle editor camera input (pan and zoom).
///
/// - Middle mouse button drag: Pan the camera
/// - Mouse wheel scroll: Zoom in/out (centered on cursor position)
pub fn handle_editor_camera_input(
    mut cameras: Query<(&mut GameCamera, &Transform, &Projection), With<MainCamera>>,
    mut input_state: ResMut<CameraInputState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        // Cursor outside window, end any drag
        if input_state.is_dragging {
            input_state.is_dragging = false;
            input_state.drag_start_screen = None;
            input_state.drag_start_camera_pos = None;
        }
        return;
    };

    for (mut game_camera, transform, projection) in cameras.iter_mut() {
        if game_camera.mode != CameraMode::Editor {
            continue;
        }

        // Handle middle mouse button drag for panning
        handle_pan_input(
            &mut game_camera,
            &mut input_state,
            &mouse_button,
            cursor_position,
            window,
            projection,
        );

        // Handle scroll wheel for zooming
        handle_zoom_input(
            &mut game_camera,
            &mut scroll_events,
            cursor_position,
            window,
            transform,
            projection,
        );
    }
}

/// Handle middle mouse button drag for panning.
fn handle_pan_input(
    game_camera: &mut GameCamera,
    input_state: &mut CameraInputState,
    mouse_button: &ButtonInput<MouseButton>,
    cursor_position: Vec2,
    _window: &Window,
    projection: &Projection,
) {
    // Start drag
    if mouse_button.just_pressed(MouseButton::Middle) {
        input_state.is_dragging = true;
        input_state.drag_start_screen = Some(cursor_position);
        input_state.drag_start_camera_pos = Some(game_camera.target);
    }

    // Continue drag
    if mouse_button.pressed(MouseButton::Middle) && input_state.is_dragging {
        if let (Some(start_screen), Some(start_camera)) =
            (input_state.drag_start_screen, input_state.drag_start_camera_pos)
        {
            // Calculate delta in screen space
            let screen_delta = cursor_position - start_screen;

            // Convert screen delta to world delta
            // In orthographic projection, screen delta / zoom = world delta
            // Invert because dragging right should move camera left (to see more right)
            let scale = if let Projection::Orthographic(ortho) = projection {
                ortho.scale
            } else {
                1.0 / game_camera.zoom
            };

            let world_delta = Vec2::new(-screen_delta.x * scale, screen_delta.y * scale);

            // Update target position
            let new_pos = start_camera + world_delta;
            game_camera.target_position = new_pos;
            // For editor, update directly without smoothing
            game_camera.target = new_pos;
        }
    }

    // End drag
    if mouse_button.just_released(MouseButton::Middle) {
        input_state.is_dragging = false;
        input_state.drag_start_screen = None;
        input_state.drag_start_camera_pos = None;
    }
}

/// Handle scroll wheel for zooming.
fn handle_zoom_input(
    game_camera: &mut GameCamera,
    scroll_events: &mut MessageReader<MouseWheel>,
    cursor_position: Vec2,
    window: &Window,
    transform: &Transform,
    projection: &Projection,
) {
    for event in scroll_events.read() {
        // Scroll amount (positive = zoom in, negative = zoom out)
        let scroll_amount = event.y;
        if scroll_amount.abs() < 0.001 {
            continue;
        }

        // Zoom factor per scroll unit
        let zoom_factor = 1.1_f32;
        let multiplier = if scroll_amount > 0.0 {
            zoom_factor
        } else {
            1.0 / zoom_factor
        };

        let old_zoom = game_camera.zoom;
        let new_zoom = (old_zoom * multiplier).clamp(10.0, 500.0);

        // Calculate cursor position in world coordinates (before zoom)
        let scale = if let Projection::Orthographic(ortho) = projection {
            ortho.scale
        } else {
            1.0 / old_zoom
        };

        // Convert cursor screen position to world position
        // Screen origin is top-left, Y increases downward
        // World origin depends on camera position
        let screen_center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        let cursor_offset_screen = cursor_position - screen_center;
        let cursor_offset_world = Vec2::new(cursor_offset_screen.x * scale, -cursor_offset_screen.y * scale);
        let cursor_world_before = Vec2::new(transform.translation.x, transform.translation.y) + cursor_offset_world;

        // Calculate new scale
        let new_scale = 1.0 / new_zoom;

        // Calculate where cursor would be in world after zoom
        let cursor_offset_world_after =
            Vec2::new(cursor_offset_screen.x * new_scale, -cursor_offset_screen.y * new_scale);

        // Adjust camera position so cursor stays at the same world position
        let new_camera_pos = cursor_world_before - cursor_offset_world_after;

        // Apply changes
        game_camera.target_zoom = new_zoom;
        game_camera.zoom = new_zoom; // Immediate update for editor
        game_camera.target_position = new_camera_pos;
        game_camera.target = new_camera_pos; // Immediate update for editor
    }
}
