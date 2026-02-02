//! Overview camera system.
//!
//! Automatically adjusts the camera to show the entire map.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy::{CameraMode, GameCamera, MainCamera};

/// System to update camera for Overview mode.
///
/// Calculates the appropriate zoom level and position to show the entire map
/// with a small padding around the edges.
pub fn update_overview_camera(
    mut cameras: Query<&mut GameCamera, With<MainCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    for mut game_camera in cameras.iter_mut() {
        if game_camera.mode != CameraMode::Overview {
            continue;
        }

        let (min, max) = game_camera.map_bounds;
        let map_size = max - min;

        // Center of the map
        let center = (min + max) / 2.0;
        game_camera.target_position = center;

        // Calculate zoom to fit the map in the viewport with 10% padding
        let padding_factor = 1.1;
        let padded_width = map_size.x * padding_factor;
        let padded_height = map_size.y * padding_factor;

        let window_width = window.width();
        let window_height = window.height();

        // Calculate zoom for both dimensions and use the smaller one (to fit both)
        let zoom_x = window_width / padded_width;
        let zoom_y = window_height / padded_height;

        // Use the smaller zoom to ensure the entire map fits
        let target_zoom = zoom_x.min(zoom_y);

        // Clamp zoom to reasonable range (10-500 pixels per meter)
        game_camera.target_zoom = target_zoom.clamp(10.0, 500.0);
    }
}
