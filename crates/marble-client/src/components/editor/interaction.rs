//! Editor interaction state and types.

/// Gizmo handle types for different transformations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoHandle {
    // Move handles
    MoveX,
    MoveY,
    MoveFree,
    // Scale handles - corners (diagonal)
    ScaleTopLeft,
    ScaleTopRight,
    ScaleBottomRight,
    ScaleBottomLeft,
    // Scale handles - edges (axis-aligned)
    ScaleTop,
    ScaleRight,
    ScaleBottom,
    ScaleLeft,
    // Rotate handle
    RotateRing,
    // Bezier handles
    BezierStart,
    BezierControl1,
    BezierControl2,
    BezierEnd,
    BezierMoveFree,
    // Line handles
    LineStart,
    LineEnd,
    LineMoveFree,
    // Pivot handle (for PivotRotate keyframe)
    PivotPoint,
    // Ghost handle (for ghost preview dragging)
    GhostMove,
}

impl GizmoHandle {
    pub fn is_move(&self) -> bool {
        matches!(self, GizmoHandle::MoveX | GizmoHandle::MoveY | GizmoHandle::MoveFree)
    }

    pub fn is_scale(&self) -> bool {
        matches!(self,
            GizmoHandle::ScaleTopLeft | GizmoHandle::ScaleTopRight |
            GizmoHandle::ScaleBottomRight | GizmoHandle::ScaleBottomLeft |
            GizmoHandle::ScaleTop | GizmoHandle::ScaleRight |
            GizmoHandle::ScaleBottom | GizmoHandle::ScaleLeft
        )
    }

    pub fn is_rotate(&self) -> bool {
        matches!(self, GizmoHandle::RotateRing)
    }

    pub fn is_bezier(&self) -> bool {
        matches!(self,
            GizmoHandle::BezierStart | GizmoHandle::BezierControl1 |
            GizmoHandle::BezierControl2 | GizmoHandle::BezierEnd |
            GizmoHandle::BezierMoveFree
        )
    }

    pub fn is_line(&self) -> bool {
        matches!(self,
            GizmoHandle::LineStart | GizmoHandle::LineEnd | GizmoHandle::LineMoveFree
        )
    }

    pub fn is_pivot(&self) -> bool {
        matches!(self, GizmoHandle::PivotPoint)
    }

    pub fn is_ghost(&self) -> bool {
        matches!(self, GizmoHandle::GhostMove)
    }
}

/// Object transform data.
#[derive(Debug, Clone, Copy)]
pub struct ObjectTransform {
    pub center: (f32, f32),
    pub size: (f32, f32),
    pub rotation: f32,
}

/// Bezier curve transform data (4 control points).
#[derive(Debug, Clone, Copy)]
pub struct BezierTransform {
    pub start: (f32, f32),
    pub control1: (f32, f32),
    pub control2: (f32, f32),
    pub end: (f32, f32),
}

impl BezierTransform {
    /// Calculate center point of the bezier curve.
    pub fn center(&self) -> (f32, f32) {
        (
            (self.start.0 + self.control1.0 + self.control2.0 + self.end.0) / 4.0,
            (self.start.1 + self.control1.1 + self.control2.1 + self.end.1) / 4.0,
        )
    }
}

/// Line transform data (2 endpoints).
#[derive(Debug, Clone, Copy)]
pub struct LineTransform {
    pub start: (f32, f32),
    pub end: (f32, f32),
}

impl LineTransform {
    /// Calculate center point of the line.
    pub fn center(&self) -> (f32, f32) {
        (
            (self.start.0 + self.end.0) / 2.0,
            (self.start.1 + self.end.1) / 2.0,
        )
    }
}

/// Pivot point transform data (single point for PivotRotate keyframe).
#[derive(Debug, Clone, Copy)]
pub struct PivotTransform {
    pub point: (f32, f32),
}

/// Ghost preview transform data (for dragging ghost destination).
#[derive(Debug, Clone, Copy)]
pub struct GhostTransform {
    pub center: (f32, f32),
    pub init_pos: [f32; 2],
    pub init_rot: f32,
}

/// Editor interaction state.
#[derive(Debug, Clone)]
pub struct EditorInteractionState {
    pub mouse_world: Option<(f32, f32)>,
    pub mouse_screen: Option<(f32, f32)>,

    pub is_panning: bool,
    pub pan_start_screen: Option<(f32, f32)>,
    pub pan_start_camera_center: Option<(f32, f32)>,

    pub active_handle: Option<GizmoHandle>,
    pub drag_start_world: Option<(f32, f32)>,
    pub original_transform: Option<ObjectTransform>,
    pub original_bezier_transform: Option<BezierTransform>,
    pub original_line_transform: Option<LineTransform>,
    pub original_pivot_transform: Option<PivotTransform>,
    pub original_ghost_transform: Option<GhostTransform>,

    pub shift_held: bool,
    pub ctrl_held: bool,
    pub alt_held: bool,
}

impl Default for EditorInteractionState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorInteractionState {
    pub fn new() -> Self {
        Self {
            mouse_world: None,
            mouse_screen: None,
            is_panning: false,
            pan_start_screen: None,
            pan_start_camera_center: None,
            active_handle: None,
            drag_start_world: None,
            original_transform: None,
            original_bezier_transform: None,
            original_line_transform: None,
            original_pivot_transform: None,
            original_ghost_transform: None,
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        }
    }

    pub fn start_panning(&mut self, screen_pos: (f32, f32), camera_center: (f32, f32)) {
        self.is_panning = true;
        self.pan_start_screen = Some(screen_pos);
        self.pan_start_camera_center = Some(camera_center);
    }

    pub fn end_panning(&mut self) {
        self.is_panning = false;
        self.pan_start_screen = None;
        self.pan_start_camera_center = None;
    }

    pub fn start_drag(&mut self, handle: GizmoHandle, world_pos: (f32, f32), transform: ObjectTransform) {
        self.active_handle = Some(handle);
        self.drag_start_world = Some(world_pos);
        self.original_transform = Some(transform);
        self.original_bezier_transform = None;
        self.original_line_transform = None;
        self.original_pivot_transform = None;
        self.original_ghost_transform = None;
    }

    pub fn start_bezier_drag(&mut self, handle: GizmoHandle, world_pos: (f32, f32), transform: BezierTransform) {
        self.active_handle = Some(handle);
        self.drag_start_world = Some(world_pos);
        self.original_transform = None;
        self.original_bezier_transform = Some(transform);
        self.original_line_transform = None;
        self.original_pivot_transform = None;
        self.original_ghost_transform = None;
    }

    pub fn start_line_drag(&mut self, handle: GizmoHandle, world_pos: (f32, f32), transform: LineTransform) {
        self.active_handle = Some(handle);
        self.drag_start_world = Some(world_pos);
        self.original_transform = None;
        self.original_bezier_transform = None;
        self.original_line_transform = Some(transform);
        self.original_pivot_transform = None;
        self.original_ghost_transform = None;
    }

    pub fn start_pivot_drag(&mut self, handle: GizmoHandle, world_pos: (f32, f32), transform: PivotTransform) {
        self.active_handle = Some(handle);
        self.drag_start_world = Some(world_pos);
        self.original_transform = None;
        self.original_bezier_transform = None;
        self.original_line_transform = None;
        self.original_pivot_transform = Some(transform);
        self.original_ghost_transform = None;
    }

    pub fn start_ghost_drag(&mut self, handle: GizmoHandle, world_pos: (f32, f32), transform: GhostTransform) {
        self.active_handle = Some(handle);
        self.drag_start_world = Some(world_pos);
        self.original_transform = None;
        self.original_bezier_transform = None;
        self.original_line_transform = None;
        self.original_pivot_transform = None;
        self.original_ghost_transform = Some(transform);
    }

    pub fn end_drag(&mut self) {
        self.active_handle = None;
        self.drag_start_world = None;
        self.original_transform = None;
        self.original_bezier_transform = None;
        self.original_line_transform = None;
        self.original_pivot_transform = None;
        self.original_ghost_transform = None;
    }

    pub fn cancel_drag(&mut self) {
        self.end_drag();
    }

    pub fn is_dragging(&self) -> bool {
        self.active_handle.is_some()
    }

    pub fn update_mouse(&mut self, screen: (f32, f32), world: (f32, f32)) {
        self.mouse_screen = Some(screen);
        self.mouse_world = Some(world);
    }

    pub fn update_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.shift_held = shift;
        self.ctrl_held = ctrl;
        self.alt_held = alt;
    }

    pub fn drag_delta(&self) -> Option<(f32, f32)> {
        match (self.drag_start_world, self.mouse_world) {
            (Some(start), Some(current)) => Some((current.0 - start.0, current.1 - start.1)),
            _ => None,
        }
    }

    pub fn pan_delta(&self) -> Option<(f32, f32)> {
        match (self.pan_start_screen, self.mouse_screen) {
            (Some(start), Some(current)) => Some((current.0 - start.0, current.1 - start.1)),
            _ => None,
        }
    }
}
