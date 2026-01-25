//! Camera system for game viewport management.
//!
//! Supports three camera modes:
//! - FollowMe: Track the local player's marble
//! - FollowLeader: Track the leading marble
//! - Overview: Show the entire map

use marble_core::{GameState, PlayerId};
use serde::{Deserialize, Serialize};

/// Camera viewing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraMode {
    /// Follow the local player's marble.
    FollowMe,
    /// Follow the leading player's marble.
    FollowLeader,
    /// Overview of the entire map (default).
    #[default]
    Overview,
}

/// Minimum y-distance margin to switch leader (prevents flickering)
const LEADER_SWITCH_MARGIN: f32 = 30.0;

/// Cooldown frames after leader switch (60 frames = 1 second at 60fps)
const LEADER_SWITCH_COOLDOWN: u32 = 60;

/// Camera state for viewport transformations.
#[derive(Debug, Clone)]
pub struct CameraState {
    /// Current camera mode.
    pub mode: CameraMode,
    /// Previous mode (for Overview toggle restoration).
    pub previous_mode: CameraMode,
    /// Current camera center position (world coordinates).
    pub center: (f32, f32),
    /// Target center position (for smooth interpolation).
    target_center: (f32, f32),
    /// Zoom level (1.0 = normal, >1.0 = zoomed in).
    pub zoom: f32,
    /// Viewport dimensions in pixels.
    pub viewport: (f32, f32),
    /// Map dimensions in world units.
    pub map_size: (f32, f32),
    /// Smoothing factor for camera movement (0.0 = instant, 1.0 = never moves).
    pub smoothing: f32,
    /// Current leader position being tracked (for hysteresis)
    current_leader_pos: Option<(f32, f32)>,
    /// Cooldown counter for leader switching (frames remaining)
    leader_switch_cooldown: u32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self::new((800.0, 600.0), (800.0, 600.0))
    }
}

impl CameraState {
    /// Creates a new camera state.
    ///
    /// # Arguments
    /// * `viewport` - Viewport dimensions (width, height) in pixels
    /// * `map_size` - Map dimensions (width, height) in world units
    pub fn new(viewport: (f32, f32), map_size: (f32, f32)) -> Self {
        let center = (map_size.0 / 2.0, map_size.1 / 2.0);
        Self {
            mode: CameraMode::Overview,
            previous_mode: CameraMode::FollowMe,
            center,
            target_center: center,
            zoom: 1.0,
            viewport,
            map_size,
            smoothing: 0.1, // 10% interpolation per frame
            current_leader_pos: None,
            leader_switch_cooldown: 0,
        }
    }

    /// Toggle between FollowMe and FollowLeader modes.
    ///
    /// Called when Tab key is pressed.
    /// - If in FollowMe: switch to FollowLeader
    /// - If in FollowLeader: switch to FollowMe
    /// - If in Overview: switch to FollowMe
    pub fn toggle_follow(&mut self) {
        self.previous_mode = self.mode;
        self.mode = match self.mode {
            CameraMode::FollowMe => CameraMode::FollowLeader,
            CameraMode::FollowLeader => CameraMode::FollowMe,
            CameraMode::Overview => CameraMode::FollowMe,
        };
    }

    /// Toggle Overview mode on/off.
    ///
    /// Called when backtick (`) key is pressed.
    /// - If in Overview: restore previous mode
    /// - Otherwise: save current mode and switch to Overview
    pub fn toggle_overview(&mut self) {
        if self.mode == CameraMode::Overview {
            // Restore previous mode
            self.mode = self.previous_mode;
        } else {
            // Save current mode and switch to Overview
            self.previous_mode = self.mode;
            self.mode = CameraMode::Overview;
        }
    }

    /// Update camera position based on current mode and game state.
    ///
    /// # Arguments
    /// * `game_state` - Current game state
    /// * `my_player_id` - Local player's ID (for FollowMe mode)
    pub fn update(&mut self, game_state: &GameState, my_player_id: Option<PlayerId>) {
        let map_center = (self.map_size.0 / 2.0, self.map_size.1 / 2.0);

        // Calculate target position based on mode
        // Note: Don't change mode automatically - just fall back to map center if target not found
        self.target_center = match self.mode {
            CameraMode::Overview => {
                // Center on map
                map_center
            }
            CameraMode::FollowMe => {
                if let Some(player_id) = my_player_id {
                    self.get_player_marble_position(game_state, player_id)
                        .unwrap_or(map_center)
                } else {
                    map_center
                }
            }
            CameraMode::FollowLeader => {
                self.get_leader_position_with_hysteresis(game_state)
                    .unwrap_or(map_center)
            }
        };

        // Calculate zoom based on mode
        let target_zoom = match self.mode {
            CameraMode::Overview => {
                // Fit entire map in viewport with some padding
                let zoom_x = self.viewport.0 / (self.map_size.0 * 1.1);
                let zoom_y = self.viewport.1 / (self.map_size.1 * 1.1);
                zoom_x.min(zoom_y)
            }
            CameraMode::FollowMe | CameraMode::FollowLeader => {
                // Closer zoom when following
                1.5
            }
        };

        // Smooth interpolation (lerp)
        let t = self.smoothing;
        self.center.0 += (self.target_center.0 - self.center.0) * t;
        self.center.1 += (self.target_center.1 - self.center.1) * t;
        self.zoom += (target_zoom - self.zoom) * t;
    }

    /// Get the position of a player's marble.
    fn get_player_marble_position(
        &self,
        game_state: &GameState,
        player_id: PlayerId,
    ) -> Option<(f32, f32)> {
        let marble = game_state.marble_manager.get_marble_by_owner(player_id)?;

        // Don't follow eliminated marbles
        if marble.eliminated {
            return None;
        }

        game_state
            .marble_manager
            .get_marble_position(&game_state.physics_world, marble.id)
    }

    /// Get the position of the leading marble (highest y coordinate = furthest down).
    /// Uses hysteresis and cooldown to prevent camera flickering when marbles are close.
    fn get_leader_position_with_hysteresis(
        &mut self,
        game_state: &GameState,
    ) -> Option<(f32, f32)> {
        // Decrease cooldown
        if self.leader_switch_cooldown > 0 {
            self.leader_switch_cooldown -= 1;
        }

        let active_marbles = game_state.marble_manager.active_marbles();

        // Find the marble with the highest y coordinate (actual leader)
        let mut best_pos: Option<(f32, f32)> = None;
        let mut max_y = f32::NEG_INFINITY;

        // Also find the position of the marble closest to current tracked position
        let mut current_tracked_pos: Option<(f32, f32)> = None;
        let mut current_tracked_y = f32::NEG_INFINITY;

        for marble in active_marbles {
            if let Some(pos) = game_state
                .marble_manager
                .get_marble_position(&game_state.physics_world, marble.id)
            {
                // Track the actual leader
                if pos.1 > max_y {
                    max_y = pos.1;
                    best_pos = Some(pos);
                }

                // If we have a current leader, find the marble closest to it
                if let Some(current) = self.current_leader_pos {
                    let dist_sq = (pos.0 - current.0).powi(2) + (pos.1 - current.1).powi(2);
                    if dist_sq < 2500.0 {
                        // Within 50 units
                        if pos.1 > current_tracked_y {
                            current_tracked_y = pos.1;
                            current_tracked_pos = Some(pos);
                        }
                    }
                }
            }
        }

        // If on cooldown and we have a tracked position, keep it
        if self.leader_switch_cooldown > 0 && current_tracked_pos.is_some() {
            self.current_leader_pos = current_tracked_pos;
            return current_tracked_pos;
        }

        // Apply hysteresis: only switch leader if new leader is significantly ahead
        let result = if let (Some(best), Some(tracked)) = (best_pos, current_tracked_pos) {
            if best.1 > tracked.1 + LEADER_SWITCH_MARGIN {
                // New leader is significantly ahead, switch and start cooldown
                self.leader_switch_cooldown = LEADER_SWITCH_COOLDOWN;
                best_pos
            } else {
                // Keep tracking current leader
                current_tracked_pos
            }
        } else if best_pos.is_some() && self.current_leader_pos.is_none() {
            // First time tracking, start cooldown
            self.leader_switch_cooldown = LEADER_SWITCH_COOLDOWN;
            best_pos
        } else {
            // No current tracking or no marbles, use best
            best_pos
        };

        // Update current leader position
        self.current_leader_pos = result;
        result
    }

    /// Calculate the view-projection matrix for rendering.
    ///
    /// Returns a 4x4 matrix in column-major order suitable for WGSL shaders.
    pub fn view_projection_matrix(&self) -> [[f32; 4]; 4] {
        // Calculate visible area in world coordinates
        let half_width = self.viewport.0 / (2.0 * self.zoom);
        let half_height = self.viewport.1 / (2.0 * self.zoom);

        // Orthographic projection
        // Maps world coordinates to clip space [-1, 1]
        let left = self.center.0 - half_width;
        let right = self.center.0 + half_width;
        let bottom = self.center.1 + half_height; // Y is flipped for screen coordinates
        let top = self.center.1 - half_height;

        // Standard orthographic projection matrix
        let sx = 2.0 / (right - left);
        let sy = 2.0 / (top - bottom);
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);

        // Column-major order for WGSL
        [
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [tx, ty, 0.0, 1.0],
        ]
    }

    /// Update viewport dimensions.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport = (width, height);
    }

    /// Update map dimensions.
    pub fn set_map_size(&mut self, width: f32, height: f32) {
        self.map_size = (width, height);
    }

    /// Get the current camera mode.
    pub fn mode(&self) -> CameraMode {
        self.mode
    }

    /// Set the camera mode directly.
    ///
    /// Used by Q/W/E keys and UI buttons.
    pub fn set_mode(&mut self, mode: CameraMode) {
        if self.mode != mode {
            self.previous_mode = self.mode;
            self.mode = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_toggle_follow() {
        let mut camera = CameraState::new((800.0, 600.0), (800.0, 600.0));

        // Start in Overview (default)
        assert_eq!(camera.mode, CameraMode::Overview);

        // Tab from Overview -> FollowMe
        camera.toggle_follow();
        assert_eq!(camera.mode, CameraMode::FollowMe);

        // Tab from FollowMe -> FollowLeader
        camera.toggle_follow();
        assert_eq!(camera.mode, CameraMode::FollowLeader);

        // Tab from FollowLeader -> FollowMe
        camera.toggle_follow();
        assert_eq!(camera.mode, CameraMode::FollowMe);
    }

    #[test]
    fn test_camera_toggle_overview() {
        let mut camera = CameraState::new((800.0, 600.0), (800.0, 600.0));

        // Start in Overview, toggle to follow first
        camera.toggle_follow();
        assert_eq!(camera.mode, CameraMode::FollowMe);

        // Toggle to Overview
        camera.toggle_overview();
        assert_eq!(camera.mode, CameraMode::Overview);
        assert_eq!(camera.previous_mode, CameraMode::FollowMe);

        // Toggle back - should restore FollowMe
        camera.toggle_overview();
        assert_eq!(camera.mode, CameraMode::FollowMe);
    }

    #[test]
    fn test_view_projection_matrix() {
        let camera = CameraState::new((800.0, 600.0), (800.0, 600.0));
        let matrix = camera.view_projection_matrix();

        // Matrix should be valid (non-zero diagonal elements)
        assert!(matrix[0][0] != 0.0);
        assert!(matrix[1][1] != 0.0);
        assert!(matrix[2][2] != 0.0);
        assert!(matrix[3][3] != 0.0);
    }
}
