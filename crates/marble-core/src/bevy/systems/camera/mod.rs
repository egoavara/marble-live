//! Camera systems for the marble game.
//!
//! Provides different camera modes:
//! - `FollowTarget`: Follow a specific player's marble
//! - `FollowLeader`: Automatically follow the leading marble (highest Y)
//! - `Overview`: Show the entire map with auto-zoom
//! - `Editor`: Manual pan/zoom control

pub mod editor;
pub mod follow;
pub mod overview;

pub use editor::*;
pub use follow::*;
pub use overview::*;

use bevy::prelude::*;

use crate::bevy::{GameCamera, MainCamera};

/// System to apply smooth interpolation to camera position and zoom.
///
/// This system should run after all camera mode systems to apply
/// the final smoothed values to the camera transform.
pub fn apply_camera_smoothing(
    mut cameras: Query<(&mut GameCamera, &mut Transform, &mut Projection), With<MainCamera>>,
) {
    for (mut game_camera, mut transform, mut projection) in cameras.iter_mut() {
        // Interpolate position
        game_camera.target = game_camera
            .target
            .lerp(game_camera.target_position, game_camera.smoothing);

        // Interpolate zoom
        game_camera.zoom =
            game_camera.zoom + (game_camera.target_zoom - game_camera.zoom) * game_camera.smoothing;

        // Apply to transform
        transform.translation.x = game_camera.target.x;
        transform.translation.y = game_camera.target.y;

        // Update projection scale if orthographic
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.scale = 1.0 / game_camera.zoom;
        }
    }
}

// Note: Camera mode dispatch is implicit - each system checks the mode before updating.
