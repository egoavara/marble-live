//! Editor-specific systems for the marble map editor.
//!
//! These systems handle:
//! - Object selection and highlighting
//! - Gizmo rendering (move, scale, rotate)
//! - Mouse input handling
//! - Editor state synchronization with Yew
//! - Snap to guidelines and axes

mod gizmo;
mod input;
mod selection;
mod snap;

pub use gizmo::*;
pub use input::*;
pub use selection::*;
pub use snap::*;

use bevy::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::map::MapObject;

/// Editor state resource - shared with Yew via state store.
#[derive(Resource, Default, Clone)]
pub struct EditorStateRes {
    /// Currently selected object index.
    pub selected_object: Option<usize>,
    /// Currently selected keyframe sequence index.
    pub selected_sequence: Option<usize>,
    /// Currently selected keyframe index within sequence.
    pub selected_keyframe: Option<usize>,
    /// Current mouse position in world coordinates.
    pub mouse_world: Vec2,
    /// Current mouse position in screen coordinates.
    pub mouse_screen: Vec2,
    /// Whether mouse is currently dragging.
    pub is_dragging: bool,
    /// Active gizmo handle being dragged.
    pub active_handle: Option<GizmoHandle>,
    /// Currently hovered gizmo handle (for visual feedback).
    pub hovered_handle: Option<GizmoHandle>,
    /// Mouse position when drag started (for relative movement).
    pub drag_start_mouse: Option<Vec2>,
    /// Object center when drag started (for relative movement).
    pub drag_start_object_center: Option<Vec2>,
    /// Object size when drag started (for scale operations).
    pub drag_start_size: Option<Vec2>,
    /// Object rotation when drag started (for rotate operations, in radians).
    pub drag_start_rotation: Option<f32>,
    /// Initial angle from object center to mouse when rotation drag started.
    pub drag_start_angle: Option<f32>,
    /// Whether simulation is running.
    pub is_simulating: bool,
    /// Whether preview is active.
    pub is_previewing: bool,
    // Keyframe drag state
    /// Keyframe pivot position when drag started (for PivotRotate).
    pub drag_start_keyframe_pivot: Option<[f32; 2]>,
    /// Keyframe angle when drag started (for PivotRotate/Apply rotation).
    pub drag_start_keyframe_angle: Option<f32>,
    /// Keyframe translation when drag started (for Apply).
    pub drag_start_keyframe_translation: Option<[f32; 2]>,
    /// Index of the currently snapped target (for visual feedback).
    pub snapped_target_index: Option<usize>,
}

/// Gizmo handle types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoHandle {
    // Move handles (world coordinate)
    MoveX,
    MoveY,
    MoveFree,
    // Move handles (local coordinate - for rotated objects)
    LocalMoveX,
    LocalMoveY,
    // Scale handles (for Rect)
    ScaleTopLeft,
    ScaleTopRight,
    ScaleBottomLeft,
    ScaleBottomRight,
    ScaleTop,
    ScaleBottom,
    ScaleLeft,
    ScaleRight,
    // Radius handles (for Circle) - 4 cardinal directions
    RadiusTop,
    RadiusBottom,
    RadiusLeft,
    RadiusRight,
    // Rotate handle
    Rotate,
    // Bezier handles
    BezierStart,
    BezierControl1,
    BezierControl2,
    BezierEnd,
    // Line handles
    LineStart,
    LineEnd,
    // Pivot handle
    Pivot,
    // Keyframe handles
    /// PivotRotate's pivot position handle
    KeyframePivot,
    /// PivotRotate/Apply rotation angle handle (arc)
    KeyframeAngle,
    /// Apply's X translation handle
    KeyframeTranslateX,
    /// Apply's Y translation handle
    KeyframeTranslateY,
    /// Apply's free translation handle
    KeyframeTranslateFree,
    // Guideline handles
    /// Guideline start point
    GuidelineStart,
    /// Guideline end point
    GuidelineEnd,
    /// Guideline move (whole line)
    GuidelineMove,
}

/// Transform data for dragging.
#[derive(Debug, Clone, Copy)]
pub struct ObjectTransform {
    pub center: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}

/// Editor state store for Yew synchronization.
#[derive(Resource, Clone, Default)]
pub struct EditorStateStore {
    inner: Arc<RwLock<EditorStateStoreInner>>,
}

#[derive(Default)]
struct EditorStateStoreInner {
    selected_object: Option<usize>,
    selected_sequence: Option<usize>,
    selected_keyframe: Option<usize>,
    mouse_world: [f32; 2],
    is_simulating: bool,
    is_previewing: bool,
    version: u64,
    /// Pending object updates from Yew.
    pending_object_updates: Vec<(usize, MapObject)>,
    /// Pending selection changes from Yew.
    pending_selection: Option<Option<usize>>,
}

impl EditorStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get selected object index.
    pub fn get_selected_object(&self) -> Option<usize> {
        self.inner.read().selected_object
    }

    /// Set selected object index (from Yew).
    pub fn set_selected_object(&self, index: Option<usize>) {
        let mut inner = self.inner.write();
        inner.pending_selection = Some(index);
        inner.version += 1;
    }

    /// Get version for change detection.
    pub fn get_version(&self) -> u64 {
        self.inner.read().version
    }

    /// Queue an object update from Yew.
    pub fn queue_object_update(&self, index: usize, object: MapObject) {
        let mut inner = self.inner.write();
        inner.pending_object_updates.push((index, object));
        inner.version += 1;
    }

    /// Take pending updates (called by Bevy systems).
    pub fn take_pending_updates(&self) -> Vec<(usize, MapObject)> {
        let mut inner = self.inner.write();
        std::mem::take(&mut inner.pending_object_updates)
    }

    /// Take pending selection (called by Bevy systems).
    pub fn take_pending_selection(&self) -> Option<Option<usize>> {
        let mut inner = self.inner.write();
        inner.pending_selection.take()
    }

    /// Update state from Bevy.
    pub fn sync_from_bevy(&self, state: &EditorStateRes) {
        let mut inner = self.inner.write();
        inner.selected_object = state.selected_object;
        inner.selected_sequence = state.selected_sequence;
        inner.selected_keyframe = state.selected_keyframe;
        inner.mouse_world = [state.mouse_world.x, state.mouse_world.y];
        inner.is_simulating = state.is_simulating;
        inner.is_previewing = state.is_previewing;
        inner.version += 1;
    }

    /// Set simulation state.
    pub fn set_simulating(&self, simulating: bool) {
        let mut inner = self.inner.write();
        inner.is_simulating = simulating;
        inner.version += 1;
    }

    /// Get simulation state.
    pub fn is_simulating(&self) -> bool {
        self.inner.read().is_simulating
    }
}

/// Message sent when an object is selected.
#[derive(Message, Debug, Clone)]
pub struct SelectObjectEvent(pub Option<usize>);

/// Message sent when an object is updated.
#[derive(Message, Debug, Clone)]
pub struct UpdateObjectEvent {
    pub index: usize,
    pub object: MapObject,
}

/// Colors for gizmo rendering.
pub struct GizmoColors;

impl GizmoColors {
    pub const X_AXIS: Color = Color::srgb(0.9, 0.2, 0.2);
    pub const Y_AXIS: Color = Color::srgb(0.2, 0.9, 0.2);
    pub const FREE: Color = Color::srgb(0.9, 0.9, 0.9);
    pub const ROTATE: Color = Color::srgb(0.2, 0.5, 0.9);
    pub const SCALE: Color = Color::srgb(0.9, 0.6, 0.2);
    pub const SELECTED: Color = Color::srgba(0.2, 0.8, 0.9, 0.8);
    pub const HOVER: Color = Color::srgba(1.0, 1.0, 0.2, 0.9);
    pub const BEZIER_CONTROL: Color = Color::srgb(0.9, 0.4, 0.9);
    pub const PIVOT: Color = Color::srgb(0.9, 0.2, 0.9);
    // Local coordinate gizmo colors (slightly darker/more saturated)
    pub const LOCAL_X_AXIS: Color = Color::srgb(0.7, 0.1, 0.1);
    pub const LOCAL_Y_AXIS: Color = Color::srgb(0.1, 0.7, 0.1);
    // Keyframe gizmo colors
    pub const KEYFRAME_PIVOT: Color = Color::srgb(0.9, 0.4, 0.9); // Magenta
    pub const KEYFRAME_TRANSLATE: Color = Color::srgb(0.3, 0.8, 0.3); // Green
    pub const KEYFRAME_ROTATE: Color = Color::srgb(0.3, 0.5, 0.9); // Blue
    // Guideline gizmo colors
    pub const GUIDELINE: Color = Color::srgba(0.0, 0.8, 0.8, 0.9); // Cyan
    pub const GUIDELINE_ENDPOINT: Color = Color::srgb(0.0, 0.6, 0.6); // Darker cyan
    // Distance line color
    pub const DISTANCE_LINE: Color = Color::srgba(0.8, 0.8, 0.0, 0.5); // Yellow, semi-transparent
}
