//! ECS Components for the marble game.
//!
//! These components are shared between game and editor modes.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dsl::{BoolOrExpr, NumberOrExpr, Vec2OrExpr};
use crate::map::{ObjectRole, Shape, VectorFieldFalloff};
use crate::marble::{Color as MarbleColor, PlayerId};

/// Camera mode for different viewing behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum CameraMode {
    /// Follow a specific player's marble.
    FollowTarget(PlayerId),
    /// Automatically follow the leading marble (highest Y position) with hysteresis.
    FollowLeader,
    /// Show the entire map with automatic zoom adjustment.
    #[default]
    Overview,
    /// Manual pan/zoom control (editor mode).
    Editor,
}

/// Marker component for marble entities.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Marble {
    /// The player who owns this marble.
    pub owner_id: PlayerId,
    /// Whether this marble has been eliminated (reached a trigger).
    pub eliminated: bool,
}

impl Marble {
    pub fn new(owner_id: PlayerId) -> Self {
        Self {
            owner_id,
            eliminated: false,
        }
    }
}

/// Marker component for map objects.
#[derive(Component, Debug, Clone)]
pub struct MapObjectMarker {
    /// The object's unique ID (if any).
    pub object_id: Option<String>,
    /// The role of this object in the map.
    pub role: ObjectRole,
}

/// Trigger zone component for goal detection.
#[derive(Component, Debug, Clone)]
pub struct TriggerZone {
    /// Action to perform when a marble enters this trigger.
    /// "gamerule" = remove marble from memory entirely.
    pub action: String,
    /// Index of this trigger in the trigger list.
    pub trigger_index: usize,
}

/// Spawner zone component for marble spawn areas.
#[derive(Component, Debug, Clone)]
pub struct SpawnerZone {
    /// Spawn mode (e.g., "random").
    pub mode: String,
    /// Initial force mode (e.g., "random").
    pub initial_force: String,
}

/// Vector field zone component for directional forces.
#[derive(Component, Debug, Clone)]
pub struct VectorFieldZone {
    /// The shape of this vector field (for area detection).
    pub shape: Shape,
    /// Force direction (can use CEL expressions).
    pub direction: Vec2OrExpr,
    /// Force magnitude (can be a CEL expression).
    pub magnitude: NumberOrExpr,
    /// Whether the field is enabled (can be a CEL expression).
    pub enabled: BoolOrExpr,
    /// Falloff mode.
    pub falloff: VectorFieldFalloff,
}

/// Marker for objects that can be animated (keyframes).
#[derive(Component, Debug, Clone)]
pub struct AnimatedObject {
    /// Initial position at map load time.
    pub initial_position: Vec2,
    /// Initial rotation (radians) at map load time.
    pub initial_rotation: f32,
}

/// Visual representation of a marble.
#[derive(Component, Debug, Clone)]
pub struct MarbleVisual {
    /// The marble's color.
    pub color: MarbleColor,
    /// The marble's radius.
    pub radius: f32,
}

/// Component for objects that are targets of keyframe animations.
#[derive(Component, Debug, Clone)]
pub struct KeyframeTarget {
    /// The object ID used to identify this target in keyframe sequences.
    pub object_id: String,
}

/// Camera controller for game view.
#[derive(Component, Debug, Clone)]
pub struct GameCamera {
    /// Current camera mode.
    pub mode: CameraMode,
    /// Current zoom level (pixels per meter, default 100.0 means 1m = 100px).
    pub zoom: f32,
    /// Current camera position in world coordinates.
    pub target: Vec2,
    /// Target zoom level for smooth interpolation.
    pub target_zoom: f32,
    /// Target position for smooth interpolation.
    pub target_position: Vec2,
    /// Smoothing factor (0.0-1.0, lower = smoother). Default 0.1 means 10% per frame.
    pub smoothing: f32,
    /// Minimum distance difference to switch leader (hysteresis margin in meters).
    pub leader_switch_margin: f32,
    /// Cooldown frames before allowing leader switch.
    pub leader_cooldown: u32,
    /// Current cooldown counter.
    pub current_cooldown: u32,
    /// Currently tracked leader player ID (for FollowLeader mode).
    pub current_leader: Option<PlayerId>,
    /// Map bounds (min, max) for Overview mode auto-zoom.
    pub map_bounds: (Vec2, Vec2),
}

impl Default for GameCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl GameCamera {
    pub fn new() -> Self {
        Self {
            mode: CameraMode::Overview,
            // 100x zoom to convert meters to ~pixels (1m = 100px)
            zoom: 100.0,
            // Center on the map (6x10m map, center at 3,5)
            target: Vec2::new(3.0, 5.0),
            target_zoom: 100.0,
            target_position: Vec2::new(3.0, 5.0),
            smoothing: 0.1,
            // Leader hysteresis settings
            leader_switch_margin: 0.3, // 0.3m = 30cm difference to switch
            leader_cooldown: 60,       // 60 frames = 1 second at 60fps
            current_cooldown: 0,
            current_leader: None,
            // Default map bounds (will be updated when map loads)
            map_bounds: (Vec2::ZERO, Vec2::new(6.0, 10.0)),
        }
    }

    /// Creates a camera configured for editor mode.
    pub fn editor() -> Self {
        Self {
            mode: CameraMode::Editor,
            ..Self::new()
        }
    }

    /// Creates a camera configured for game play with Overview mode.
    pub fn game() -> Self {
        Self {
            mode: CameraMode::Overview,
            ..Self::new()
        }
    }

    /// Set the camera mode.
    pub fn set_mode(&mut self, mode: CameraMode) {
        self.mode = mode;
        // Reset leader tracking when changing modes
        if !matches!(mode, CameraMode::FollowLeader) {
            self.current_leader = None;
            self.current_cooldown = 0;
        }
    }

    /// Update map bounds (typically called when a map is loaded).
    pub fn set_map_bounds(&mut self, min: Vec2, max: Vec2) {
        self.map_bounds = (min, max);
    }
}

/// Marker for the main game camera.
#[derive(Component, Debug, Clone, Default)]
pub struct MainCamera;

/// Marker component for guideline objects.
#[derive(Component, Debug, Clone)]
pub struct GuidelineMarker {
    /// Guideline color.
    pub color: Color,
    /// Whether to show ruler ticks.
    pub show_ruler: bool,
    /// Whether snap is enabled.
    pub snap_enabled: bool,
    /// Snap distance in meters.
    pub snap_distance: f32,
    /// Ruler tick interval in meters.
    pub ruler_interval: f32,
    /// Line start point (for snap calculations).
    pub start: Vec2,
    /// Line end point (for snap calculations).
    pub end: Vec2,
}

impl Default for GuidelineMarker {
    fn default() -> Self {
        Self {
            color: Color::srgba(0.0, 0.8, 0.8, 0.8), // Cyan
            show_ruler: true,
            snap_enabled: true,
            snap_distance: 0.15,
            ruler_interval: 0.5,
            start: Vec2::ZERO,
            end: Vec2::new(0.0, 10.0),
        }
    }
}
