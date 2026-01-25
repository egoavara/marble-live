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
    /// Map bounds ((min_x, min_y), (max_x, max_y)) in world units.
    pub map_bounds: ((f32, f32), (f32, f32)),
    /// Smoothing factor for camera movement (0.0 = instant, 1.0 = never moves).
    pub smoothing: f32,
    /// Current leader position being tracked (for hysteresis)
    current_leader_pos: Option<(f32, f32)>,
    /// Cooldown counter for leader switching (frames remaining)
    leader_switch_cooldown: u32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)))
    }
}

impl CameraState {
    /// Creates a new camera state.
    ///
    /// # Arguments
    /// * `viewport` - Viewport dimensions (width, height) in pixels
    /// * `map_bounds` - Map bounds ((min_x, min_y), (max_x, max_y)) in world units
    pub fn new(viewport: (f32, f32), map_bounds: ((f32, f32), (f32, f32))) -> Self {
        let ((min_x, min_y), (max_x, max_y)) = map_bounds;
        let center = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        Self {
            mode: CameraMode::Overview,
            previous_mode: CameraMode::FollowMe,
            center,
            target_center: center,
            zoom: 1.0,
            viewport,
            map_bounds,
            smoothing: 0.1, // 10% interpolation per frame
            current_leader_pos: None,
            leader_switch_cooldown: 0,
        }
    }

    /// Helper to get map size from bounds.
    fn map_size(&self) -> (f32, f32) {
        let ((min_x, min_y), (max_x, max_y)) = self.map_bounds;
        (max_x - min_x, max_y - min_y)
    }

    /// Helper to get map center from bounds.
    fn map_center(&self) -> (f32, f32) {
        let ((min_x, min_y), (max_x, max_y)) = self.map_bounds;
        ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
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
        let map_center = self.map_center();
        let map_size = self.map_size();

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
                let zoom_x = self.viewport.0 / (map_size.0 * 1.1);
                let zoom_y = self.viewport.1 / (map_size.1 * 1.1);
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

    /// Update map bounds.
    pub fn set_map_bounds(&mut self, bounds: ((f32, f32), (f32, f32))) {
        self.map_bounds = bounds;
    }

    /// Fit the camera to show the entire map (for editor use).
    ///
    /// Centers on the map and adjusts zoom to fit the map in the viewport.
    pub fn fit_to_map(&mut self) {
        let map_center = self.map_center();
        let map_size = self.map_size();

        // Center on map
        self.center = map_center;
        self.target_center = map_center;

        // Calculate zoom to fit map in viewport with padding
        let zoom_x = self.viewport.0 / (map_size.0 * 1.1);
        let zoom_y = self.viewport.1 / (map_size.1 * 1.1);
        self.zoom = zoom_x.min(zoom_y).max(0.1);
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

    // === Editor interaction methods ===

    /// Convert screen coordinates to world coordinates.
    ///
    /// # Arguments
    /// * `screen_x` - Screen X coordinate in pixels
    /// * `screen_y` - Screen Y coordinate in pixels
    ///
    /// # Returns
    /// World coordinates as (x, y)
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let half_width = self.viewport.0 / (2.0 * self.zoom);
        let half_height = self.viewport.1 / (2.0 * self.zoom);

        // Convert from screen space [0, viewport] to normalized [-0.5, 0.5]
        let norm_x = screen_x / self.viewport.0 - 0.5;
        let norm_y = screen_y / self.viewport.1 - 0.5;

        // Convert to world space
        let world_x = self.center.0 + norm_x * 2.0 * half_width;
        let world_y = self.center.1 + norm_y * 2.0 * half_height;

        (world_x, world_y)
    }

    /// Convert world coordinates to screen coordinates.
    ///
    /// # Arguments
    /// * `world_x` - World X coordinate
    /// * `world_y` - World Y coordinate
    ///
    /// # Returns
    /// Screen coordinates as (x, y) in pixels
    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let half_width = self.viewport.0 / (2.0 * self.zoom);
        let half_height = self.viewport.1 / (2.0 * self.zoom);

        // Convert from world space to normalized [-0.5, 0.5]
        let norm_x = (world_x - self.center.0) / (2.0 * half_width);
        let norm_y = (world_y - self.center.1) / (2.0 * half_height);

        // Convert to screen space [0, viewport]
        let screen_x = (norm_x + 0.5) * self.viewport.0;
        let screen_y = (norm_y + 0.5) * self.viewport.1;

        (screen_x, screen_y)
    }

    /// Pan camera by screen delta (pixels).
    ///
    /// Moves the camera in the opposite direction of the delta,
    /// so dragging right moves the view left (content moves right).
    ///
    /// # Arguments
    /// * `dx` - Screen X delta in pixels
    /// * `dy` - Screen Y delta in pixels
    pub fn pan_by_screen_delta(&mut self, dx: f32, dy: f32) {
        // Convert screen delta to world delta
        let world_dx = dx / self.zoom;
        let world_dy = dy / self.zoom;

        // Move camera (opposite direction for natural panning feel)
        self.center.0 -= world_dx;
        self.center.1 -= world_dy;
        self.target_center = self.center;
    }

    /// Zoom at a specific screen position.
    ///
    /// Zooms in/out while keeping the world point under the cursor fixed.
    ///
    /// # Arguments
    /// * `screen_x` - Screen X coordinate (zoom center)
    /// * `screen_y` - Screen Y coordinate (zoom center)
    /// * `zoom_delta` - Zoom multiplier delta (positive = zoom in, negative = zoom out)
    pub fn zoom_at_screen_pos(&mut self, screen_x: f32, screen_y: f32, zoom_delta: f32) {
        // Get world position before zoom
        let world_before = self.screen_to_world(screen_x, screen_y);

        // Apply zoom with clamping
        let new_zoom = (self.zoom * (1.0 + zoom_delta)).clamp(0.1, 10.0);
        self.zoom = new_zoom;

        // Get world position after zoom
        let world_after = self.screen_to_world(screen_x, screen_y);

        // Adjust center to keep the same world point under cursor
        self.center.0 += world_before.0 - world_after.0;
        self.center.1 += world_before.1 - world_after.1;
        self.target_center = self.center;
    }

    /// Set zoom level directly.
    ///
    /// # Arguments
    /// * `zoom` - New zoom level (clamped to 0.1..10.0)
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.1, 10.0);
    }

    /// Set camera center directly.
    ///
    /// # Arguments
    /// * `x` - World X coordinate
    /// * `y` - World Y coordinate
    pub fn set_center(&mut self, x: f32, y: f32) {
        self.center = (x, y);
        self.target_center = self.center;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_toggle_follow() {
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));

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
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));

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
        let camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        let matrix = camera.view_projection_matrix();

        // Matrix should be valid (non-zero diagonal elements)
        assert!(matrix[0][0] != 0.0);
        assert!(matrix[1][1] != 0.0);
        assert!(matrix[2][2] != 0.0);
        assert!(matrix[3][3] != 0.0);
    }

    #[test]
    fn test_map_center_calculation() {
        let camera = CameraState::new((800.0, 600.0), ((100.0, 200.0), (500.0, 800.0)));

        // Map center should be (300, 500)
        let center = camera.map_center();
        assert!((center.0 - 300.0).abs() < 0.001);
        assert!((center.1 - 500.0).abs() < 0.001);

        // Map size should be (400, 600)
        let size = camera.map_size();
        assert!((size.0 - 400.0).abs() < 0.001);
        assert!((size.1 - 600.0).abs() < 0.001);
    }

    #[test]
    fn test_screen_to_world_conversion() {
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        camera.center = (400.0, 300.0);
        camera.zoom = 1.0;

        // Center of screen should map to camera center
        let (wx, wy) = camera.screen_to_world(400.0, 300.0);
        assert!((wx - 400.0).abs() < 0.001);
        assert!((wy - 300.0).abs() < 0.001);

        // Top-left corner
        let (wx, wy) = camera.screen_to_world(0.0, 0.0);
        assert!((wx - 0.0).abs() < 0.001);
        assert!((wy - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_world_to_screen_conversion() {
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        camera.center = (400.0, 300.0);
        camera.zoom = 1.0;

        // Camera center should map to screen center
        let (sx, sy) = camera.world_to_screen(400.0, 300.0);
        assert!((sx - 400.0).abs() < 0.001);
        assert!((sy - 300.0).abs() < 0.001);
    }

    #[test]
    fn test_pan_by_screen_delta() {
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        camera.center = (400.0, 300.0);
        camera.zoom = 1.0;

        // Pan right by 100 pixels should move camera left
        camera.pan_by_screen_delta(100.0, 0.0);
        assert!((camera.center.0 - 300.0).abs() < 0.001);
        assert!((camera.center.1 - 300.0).abs() < 0.001);
    }

    #[test]
    fn test_zoom_at_screen_pos() {
        let mut camera = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        camera.center = (400.0, 300.0);
        camera.zoom = 1.0;

        // Zoom in at center should keep center fixed
        let world_before = camera.screen_to_world(400.0, 300.0);
        camera.zoom_at_screen_pos(400.0, 300.0, 0.5);
        let world_after = camera.screen_to_world(400.0, 300.0);

        assert!((world_before.0 - world_after.0).abs() < 0.01);
        assert!((world_before.1 - world_after.1).abs() < 0.01);
        assert!((camera.zoom - 1.5).abs() < 0.001);
    }
}
