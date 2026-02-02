//! Editor canvas with Blender-style unified gizmo.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use marble_core::keyframe::KeyframeExecutor;
use marble_core::map::{EvaluatedShape, Keyframe, KeyframeSequence, MapObject, RouletteConfig, Shape};
use marble_core::{GameContext, GameState};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::context_menu::{ContextMenu, ContextMenuState};
use super::gizmo::{self, generate_bezier_gizmo, generate_gizmo, generate_line_gizmo, generate_pivot_gizmo, hit_test_bezier_gizmo, hit_test_ghost, hit_test_gizmo, hit_test_line_gizmo, hit_test_pivot_gizmo};
// Re-export instance types from gizmo module
use super::gizmo::{CircleInstance, LineInstance, RectInstance};
use super::interaction::{BezierTransform, EditorInteractionState, GhostTransform, GizmoHandle, LineTransform, ObjectTransform, PivotTransform};
use crate::camera::{CameraMode, CameraState};
// Use stub renderer during Bevy migration
use crate::renderer_stub::WgpuRenderer;

/// Preview transform during drag (standard, bezier, line, or pivot).
#[derive(Debug, Clone, Copy)]
pub enum PreviewTransform {
    Standard(usize, ObjectTransform),
    Bezier(usize, BezierTransform),
    Line(usize, LineTransform),
    /// Pivot preview: (sequence_index, keyframe_index, pivot_transform)
    Pivot(usize, usize, PivotTransform),
    /// Ghost preview drag: (sequence_index, keyframe_index, ghost_transform)
    Ghost(usize, usize, GhostTransform),
}

#[derive(Properties)]
pub struct EditorCanvasProps {
    pub config: RouletteConfig,
    pub selected_index: Option<usize>,
    #[prop_or_default]
    pub on_object_update: Callback<(usize, MapObject)>,
    #[prop_or_default]
    pub on_select: Callback<Option<usize>>,
    /// Simulation game state reference (for immediate access).
    #[prop_or_default]
    pub game_state_ref: Option<Rc<RefCell<Option<Rc<RefCell<GameState>>>>>>,
    /// Whether simulation is running.
    #[prop_or_default]
    pub is_simulating: bool,
    /// Version counter to trigger re-render when game_state changes.
    #[prop_or_default]
    pub game_state_version: u32,
    /// Whether clipboard has content.
    #[prop_or_default]
    pub has_clipboard: bool,
    /// Copy object callback.
    #[prop_or_default]
    pub on_copy: Callback<usize>,
    /// Paste object callback (x, y world position).
    #[prop_or_default]
    pub on_paste: Callback<(f32, f32)>,
    /// Delete object callback.
    #[prop_or_default]
    pub on_delete: Callback<usize>,
    /// Mirror X callback.
    #[prop_or_default]
    pub on_mirror_x: Callback<usize>,
    /// Mirror Y callback.
    #[prop_or_default]
    pub on_mirror_y: Callback<usize>,
    /// Target object IDs from selected keyframe sequence (for highlighting).
    #[prop_or_default]
    pub sequence_target_ids: Vec<String>,
    /// Preview keyframe sequence (single keyframe to preview).
    #[prop_or_default]
    pub preview_sequence: Option<KeyframeSequence>,
    /// Callback when preview animation completes.
    #[prop_or_default]
    pub on_preview_complete: Callback<()>,
    /// Callback when current preview keyframe index changes.
    #[prop_or_default]
    pub on_preview_keyframe_change: Callback<Option<usize>>,
    /// Currently selected keyframe sequence (for pivot gizmo).
    #[prop_or_default]
    pub selected_sequence: Option<KeyframeSequence>,
    /// Currently selected sequence index (for pivot gizmo updates).
    #[prop_or_default]
    pub selected_sequence_index: Option<usize>,
    /// Currently selected keyframe index within the sequence (for pivot gizmo).
    #[prop_or_default]
    pub selected_keyframe: Option<usize>,
    /// Callback when a keyframe is updated via gizmo drag.
    #[prop_or_default]
    pub on_update_keyframe: Callback<(usize, Keyframe)>,
}

impl PartialEq for EditorCanvasProps {
    fn eq(&self, other: &Self) -> bool {
        self.config == other.config
            && self.selected_index == other.selected_index
            && self.on_object_update == other.on_object_update
            && self.on_select == other.on_select
            && self.is_simulating == other.is_simulating
            && self.game_state_version == other.game_state_version
            && self.has_clipboard == other.has_clipboard
            && self.on_copy == other.on_copy
            && self.on_paste == other.on_paste
            && self.on_delete == other.on_delete
            && self.on_mirror_x == other.on_mirror_x
            && self.on_mirror_y == other.on_mirror_y
            && self.sequence_target_ids == other.sequence_target_ids
            && self.preview_sequence == other.preview_sequence
            && self.on_preview_complete == other.on_preview_complete
            && self.on_preview_keyframe_change == other.on_preview_keyframe_change
            && self.selected_sequence == other.selected_sequence
            && self.selected_sequence_index == other.selected_sequence_index
            && self.selected_keyframe == other.selected_keyframe
            && self.on_update_keyframe == other.on_update_keyframe
            // Compare game_state_ref by Rc pointer equality
            && match (&self.game_state_ref, &other.game_state_ref) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

#[function_component(EditorCanvas)]
pub fn editor_canvas(props: &EditorCanvasProps) -> Html {
    let canvas_ref = use_node_ref();
    let renderer: UseStateHandle<Option<Rc<RefCell<WgpuRenderer>>>> = use_state(|| None);
    let camera = use_mut_ref(|| {
        let mut cam = CameraState::new((800.0, 600.0), ((0.0, 0.0), (6.0, 10.0)));
        cam.set_mode(CameraMode::Overview);
        cam
    });
    let interaction = use_mut_ref(EditorInteractionState::new);
    let hovered_handle = use_mut_ref(|| None::<GizmoHandle>);
    // Local preview transform during drag (doesn't trigger parent re-render)
    let preview_transform = use_mut_ref(|| None::<PreviewTransform>);
    let render_trigger = use_force_update();
    // Cached keyframe selection (RefCell for dynamic access in render closure)
    let cached_selected_sequence: Rc<RefCell<Option<KeyframeSequence>>> = use_mut_ref(|| None);
    let cached_selected_keyframe: Rc<RefCell<Option<usize>>> = use_mut_ref(|| None);
    // Context menu state
    let context_menu_state = use_state(ContextMenuState::new);
    // Track if dragging is active (for document-level event listeners)
    let is_dragging_state = use_state(|| false);

    // Keyframe preview state
    let preview_game_state: Rc<RefCell<Option<GameState>>> = use_mut_ref(|| None);
    let preview_executor: Rc<RefCell<Option<KeyframeExecutor>>> = use_mut_ref(|| None);
    let preview_initial_transforms: Rc<RefCell<std::collections::HashMap<String, ([f32; 2], f32)>>> =
        use_mut_ref(std::collections::HashMap::new);
    // Track current positions during preview (updated by animation)
    let preview_current_positions: Rc<RefCell<std::collections::HashMap<String, ([f32; 2], f32)>>> =
        use_mut_ref(std::collections::HashMap::new);
    let is_preview_active = use_state(|| false);

    // Initialize renderer
    {
        let canvas_ref = canvas_ref.clone();
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        use_effect_with(canvas_ref.clone(), move |canvas_ref| {
            if renderer.is_some() { return; }
            let canvas_ref = canvas_ref.clone();
            let renderer = renderer.clone();
            let camera = camera.clone();
            let config = config.clone();
            spawn_local(async move {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                    // Get actual canvas/window size before creating renderer
                    let (w, h) = if let Some(window) = web_sys::window() {
                        let w = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(800.0) as u32;
                        let h = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(600.0) as u32;
                        canvas.set_width(w);
                        canvas.set_height(h);
                        (w, h)
                    } else {
                        (800, 600)
                    };

                    match WgpuRenderer::new(canvas).await {
                        Ok(r) => {
                            tracing::info!("Renderer initialized with size {}x{}", w, h);
                            // Update camera viewport and fit to map immediately
                            {
                                let mut cam = camera.borrow_mut();
                                cam.set_viewport(w as f32, h as f32);
                                cam.set_map_bounds(config.calculate_bounds());
                                cam.fit_to_map();
                                tracing::info!("Camera fitted: center=({:.1},{:.1}) zoom={:.3} viewport=({},{})",
                                    cam.center.0, cam.center.1, cam.zoom, cam.viewport.0, cam.viewport.1);
                            }
                            renderer.set(Some(Rc::new(RefCell::new(r))));
                        }
                        Err(e) => tracing::error!("Failed to create renderer: {}", e),
                    }
                }
            });
        });
    }

    // Start keyframe preview when preview_sequence changes
    {
        let preview_sequence = props.preview_sequence.clone();
        let config = props.config.clone();
        let preview_game_state = preview_game_state.clone();
        let preview_executor = preview_executor.clone();
        let preview_initial_transforms = preview_initial_transforms.clone();
        let preview_current_positions = preview_current_positions.clone();
        let is_preview_active = is_preview_active.clone();

        use_effect_with(preview_sequence.clone(), move |seq| {
            if let Some(seq) = seq.clone() {
                // Create preview game state from config
                let mut gs = GameState::new(0);
                gs.load_map(config.clone());

                // Store initial transforms for target objects
                let mut initials = std::collections::HashMap::new();
                let ctx = GameContext::new(0.0, 0);
                for target_id in &seq.target_ids {
                    for obj in &config.objects {
                        if obj.id.as_ref() == Some(target_id) {
                            let shape = obj.shape.evaluate(&ctx);
                            let (pos, rot) = match shape {
                                EvaluatedShape::Circle { center, .. } => (center, 0.0),
                                EvaluatedShape::Rect { center, rotation, .. } => (center, rotation.to_radians()),
                                EvaluatedShape::Line { start, end } => {
                                    ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                                }
                                EvaluatedShape::Bezier { start, end, .. } => {
                                    ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                                }
                            };
                            initials.insert(target_id.clone(), (pos, rot));
                            break;
                        }
                    }
                }

                // Initialize current positions to initial transforms
                *preview_current_positions.borrow_mut() = initials.clone();
                *preview_initial_transforms.borrow_mut() = initials;
                *preview_game_state.borrow_mut() = Some(gs);
                *preview_executor.borrow_mut() = Some(KeyframeExecutor::new("__preview__".to_string()));
                is_preview_active.set(true);
            } else {
                // Clear preview state
                *preview_game_state.borrow_mut() = None;
                *preview_executor.borrow_mut() = None;
                preview_initial_transforms.borrow_mut().clear();
                preview_current_positions.borrow_mut().clear();
                is_preview_active.set(false);
            }
        });
    }

    // Render helper
    let do_render = {
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        let selected_index = props.selected_index;
        let hovered_handle = hovered_handle.clone();
        let preview_transform = preview_transform.clone();
        let game_state_ref = props.game_state_ref.clone();
        let is_simulating = props.is_simulating;
        let sequence_target_ids = props.sequence_target_ids.clone();
        let preview_game_state_render = preview_game_state.clone();
        let is_preview_active_render = *is_preview_active;
        let cached_selected_sequence = cached_selected_sequence.clone();
        let cached_selected_keyframe = cached_selected_keyframe.clone();

        Rc::new(move || {
            if let Some(renderer) = &*renderer {
                let cam = camera.borrow();
                let hovered = *hovered_handle.borrow();

                // Use preview game state if previewing
                if is_preview_active_render {
                    if let Some(gs) = &*preview_game_state_render.borrow() {
                        renderer.borrow_mut().render_with_overlay(gs, &cam, &[], &[], &[]);
                        return;
                    }
                }

                // Use simulation game state if simulating, otherwise create from config
                if is_simulating {
                    if let Some(gs_ref) = &game_state_ref {
                        if let Some(gs) = &*gs_ref.borrow() {
                            // Render simulation state (no gizmo overlays during simulation)
                            renderer.borrow_mut().render_with_overlay(&gs.borrow(), &cam, &[], &[], &[]);
                            return;
                        }
                    }
                }

                // Editor mode: render config with gizmo overlays
                let mut render_config = config.clone();
                let preview = preview_transform.borrow();

                // Apply preview transform if dragging
                match *preview {
                    Some(PreviewTransform::Standard(idx, transform)) => {
                        if idx < render_config.objects.len() {
                            apply_transform_to_object(&mut render_config.objects[idx], &transform);
                        }
                    }
                    Some(PreviewTransform::Bezier(idx, transform)) => {
                        if idx < render_config.objects.len() {
                            apply_bezier_transform_to_object(&mut render_config.objects[idx], &transform);
                        }
                    }
                    Some(PreviewTransform::Line(idx, transform)) => {
                        if idx < render_config.objects.len() {
                            apply_line_transform_to_object(&mut render_config.objects[idx], &transform);
                        }
                    }
                    Some(PreviewTransform::Pivot(_, _, _)) => {
                        // Pivot preview doesn't modify render_config, just shows gizmo
                    }
                    Some(PreviewTransform::Ghost(_, _, _)) => {
                        // Ghost preview doesn't modify render_config, just shows gizmo
                    }
                    None => {}
                }

                let mut game_state = GameState::new(0);
                game_state.load_map(render_config);

                // Generate gizmo overlays for selected object
                let (mut oc, mut ol, mut or) = if let Some(idx) = selected_index {
                    if idx < config.objects.len() {
                        // Check if it's a bezier object
                        if is_bezier_object(&config.objects[idx]) {
                            // Use preview bezier transform if available
                            let bezier_t = match *preview {
                                Some(PreviewTransform::Bezier(preview_idx, t)) if preview_idx == idx => Some(t),
                                _ => get_bezier_transform(&config.objects[idx]),
                            };
                            if let Some(t) = bezier_t {
                                let gizmo = generate_bezier_gizmo(&t, cam.zoom, hovered);
                                (gizmo.circles, gizmo.lines, gizmo.rects)
                            } else {
                                (vec![], vec![], vec![])
                            }
                        } else if is_line_object(&config.objects[idx]) {
                            // Use preview line transform if available
                            let line_t = match *preview {
                                Some(PreviewTransform::Line(preview_idx, t)) if preview_idx == idx => Some(t),
                                _ => get_line_transform(&config.objects[idx]),
                            };
                            if let Some(t) = line_t {
                                let gizmo = generate_line_gizmo(&t, cam.zoom, hovered);
                                (gizmo.circles, gizmo.lines, gizmo.rects)
                            } else {
                                (vec![], vec![], vec![])
                            }
                        } else {
                            // Standard object (Circle, Rect)
                            let transform = match *preview {
                                Some(PreviewTransform::Standard(preview_idx, t)) if preview_idx == idx => Some(t),
                                _ => get_object_transform(&config.objects[idx]),
                            };
                            if let Some(t) = transform {
                                let gizmo = generate_gizmo(&t, cam.zoom, hovered);
                                (gizmo.circles, gizmo.lines, gizmo.rects)
                            } else {
                                (vec![], vec![], vec![])
                            }
                        }
                    } else {
                        (vec![], vec![], vec![])
                    }
                } else {
                    (vec![], vec![], vec![])
                };

                // Generate pivot gizmo for selected PivotRotate keyframe
                let selected_keyframe = *cached_selected_keyframe.borrow();
                let selected_sequence = cached_selected_sequence.borrow();
                if let Some(kf_idx) = selected_keyframe {
                    if let Some(seq) = &*selected_sequence {
                        if let Some(kf) = seq.keyframes.get(kf_idx) {
                            if let Keyframe::PivotRotate { pivot, .. } = kf {
                                // Use preview pivot transform if dragging, otherwise use keyframe's pivot
                                let pivot_t = match *preview {
                                    Some(PreviewTransform::Pivot(_, preview_kf_idx, t)) if preview_kf_idx == kf_idx => t,
                                    _ => PivotTransform { point: (pivot[0], pivot[1]) },
                                };
                                let pivot_gizmo = generate_pivot_gizmo(&pivot_t, cam.zoom, hovered);
                                oc.extend(pivot_gizmo.circles);
                                ol.extend(pivot_gizmo.lines);
                                or.extend(pivot_gizmo.rects);
                            }
                        }
                    }
                }

                // Generate ghost preview for Apply/PivotRotate keyframe
                if let Some(kf_idx) = selected_keyframe {
                    if let Some(seq) = &*selected_sequence {
                        if let Some(kf) = seq.keyframes.get(kf_idx) {
                            if matches!(kf, Keyframe::Apply { .. } | Keyframe::PivotRotate { .. }) {
                                // During ghost drag, create a virtual keyframe from the dragged position
                                let effective_kf = match *preview {
                                    Some(PreviewTransform::Ghost(_, preview_kf_idx, ghost_t)) if preview_kf_idx == kf_idx => {
                                        match kf {
                                            Keyframe::Apply { rotation, duration, easing, .. } => {
                                                Some(Keyframe::Apply {
                                                    translation: Some([
                                                        ghost_t.center.0 - ghost_t.init_pos[0],
                                                        ghost_t.center.1 - ghost_t.init_pos[1],
                                                    ]),
                                                    rotation: *rotation,
                                                    duration: *duration,
                                                    easing: easing.clone(),
                                                })
                                            }
                                            Keyframe::PivotRotate { pivot, pivot_mode, duration, easing, .. } => {
                                                let from_angle = (ghost_t.init_pos[1] - pivot[1])
                                                    .atan2(ghost_t.init_pos[0] - pivot[0]);
                                                let to_angle = (ghost_t.center.1 - pivot[1])
                                                    .atan2(ghost_t.center.0 - pivot[0]);
                                                Some(Keyframe::PivotRotate {
                                                    pivot: *pivot,
                                                    pivot_mode: *pivot_mode,
                                                    angle: (to_angle - from_angle).to_degrees(),
                                                    duration: *duration,
                                                    easing: easing.clone(),
                                                })
                                            }
                                            _ => None,
                                        }
                                    }
                                    _ => None,
                                };
                                let render_kf = effective_kf.as_ref().unwrap_or(kf);

                                let ctx = GameContext::new(0.0, 0);
                                let mut target_shapes = Vec::new();
                                for target_id in &seq.target_ids {
                                    for obj in &config.objects {
                                        if obj.id.as_ref() == Some(target_id) {
                                            let shape = obj.shape.evaluate(&ctx);
                                            let (pos, rot) = match &shape {
                                                EvaluatedShape::Circle { center, .. } => (*center, 0.0),
                                                EvaluatedShape::Rect { center, rotation, .. } => (*center, rotation.to_radians()),
                                                EvaluatedShape::Line { start, end } => {
                                                    ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                                                }
                                                EvaluatedShape::Bezier { start, end, .. } => {
                                                    ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                                                }
                                            };
                                            target_shapes.push((shape, pos, rot));
                                            break;
                                        }
                                    }
                                }
                                if !target_shapes.is_empty() {
                                    let ghost = gizmo::generate_ghost_preview(render_kf, &target_shapes, cam.zoom, hovered);
                                    oc.extend(ghost.circles);
                                    ol.extend(ghost.lines);
                                    or.extend(ghost.rects);
                                }
                            }
                        }
                    }
                }
                drop(selected_sequence);

                // Generate highlight overlays for sequence target objects
                if !sequence_target_ids.is_empty() {
                    let ctx = GameContext::new(0.0, 0);
                    // Orange highlight color
                    let highlight_color = marble_core::Color::new(255, 152, 0, 200); // #ff9800 with alpha

                    for (idx, obj) in config.objects.iter().enumerate() {
                        // Skip if this object is selected (gizmo takes priority)
                        if selected_index == Some(idx) {
                            continue;
                        }

                        // Check if object is a sequence target
                        let is_target = obj.id.as_ref()
                            .map(|id| sequence_target_ids.contains(id))
                            .unwrap_or(false);

                        if is_target {
                            // Generate highlight overlay based on object shape
                            let highlight_width = 3.0 / cam.zoom; // Scale-independent border width
                            match obj.shape.evaluate(&ctx) {
                                EvaluatedShape::Circle { center, radius } => {
                                    // Draw a slightly larger circle as highlight
                                    oc.push(CircleInstance::new(
                                        (center[0], center[1]),
                                        radius + highlight_width,
                                        marble_core::Color::new(0, 0, 0, 0), // Transparent fill
                                        highlight_color,
                                        highlight_width,
                                    ));
                                }
                                EvaluatedShape::Rect { center, size, rotation } => {
                                    // Draw a slightly larger rect as highlight
                                    or.push(RectInstance::new(
                                        (center[0], center[1]),
                                        (size[0] / 2.0 + highlight_width, size[1] / 2.0 + highlight_width),
                                        rotation,
                                        marble_core::Color::new(0, 0, 0, 0), // Transparent fill
                                        highlight_color,
                                        highlight_width,
                                    ));
                                }
                                EvaluatedShape::Line { start, end } => {
                                    // Draw parallel lines as highlight
                                    let dx = end[0] - start[0];
                                    let dy = end[1] - start[1];
                                    let len = (dx * dx + dy * dy).sqrt();
                                    if len > 0.001 {
                                        let nx = -dy / len * highlight_width;
                                        let ny = dx / len * highlight_width;
                                        // Two parallel lines
                                        ol.push(LineInstance::new(
                                            (start[0] + nx, start[1] + ny),
                                            (end[0] + nx, end[1] + ny),
                                            2.0 / cam.zoom,
                                            highlight_color,
                                        ));
                                        ol.push(LineInstance::new(
                                            (start[0] - nx, start[1] - ny),
                                            (end[0] - nx, end[1] - ny),
                                            2.0 / cam.zoom,
                                            highlight_color,
                                        ));
                                    }
                                }
                                EvaluatedShape::Bezier { start, control1, control2, end, .. } => {
                                    // Draw bezier curve approximation as highlight
                                    const SEGMENTS: usize = 20;
                                    for i in 0..SEGMENTS {
                                        let t0 = i as f32 / SEGMENTS as f32;
                                        let t1 = (i + 1) as f32 / SEGMENTS as f32;
                                        let p0 = evaluate_bezier(t0, start, control1, control2, end);
                                        let p1 = evaluate_bezier(t1, start, control1, control2, end);
                                        ol.push(LineInstance::new(
                                            p0,
                                            p1,
                                            4.0 / cam.zoom,
                                            highlight_color,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                renderer.borrow_mut().render_with_overlay(&game_state, &cam, &oc, &ol, &or);
            }
        })
    };

    // Clear preview and re-render when config changes from parent
    {
        let preview_transform = preview_transform.clone();
        let do_render = do_render.clone();
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        let has_renderer = renderer.is_some();

        // Use a simple version counter based on selected object's position/size
        let config_version = props.selected_index.map(|idx| {
            if idx < config.objects.len() {
                // Create a simple hash of the selected object's shape
                format!("{:?}", config.objects[idx].shape)
            } else {
                String::new()
            }
        }).unwrap_or_default();

        use_effect_with((config_version, has_renderer), move |(_, has_renderer)| {
            if !*has_renderer { return; }
            // Clear preview when config updates from parent
            *preview_transform.borrow_mut() = None;
            {
                let mut cam = camera.borrow_mut();
                let bounds = config.calculate_bounds();
                cam.set_map_bounds(bounds);
            }
            do_render();
        });
    }

    // Render editor mode when simulation stops or game_state is reset
    {
        let do_render = do_render.clone();
        let renderer = renderer.clone();
        let is_simulating = props.is_simulating;
        let game_state_version = props.game_state_version;
        let has_renderer = renderer.is_some();

        use_effect_with((is_simulating, game_state_version, has_renderer), move |(is_sim, _version, has_renderer)| {
            // When simulation stops or game_state changes, render editor mode immediately
            if !*is_sim && *has_renderer {
                do_render();
            }
        });
    }

    // Re-render when sequence_target_ids changes (for highlighting target objects)
    {
        let do_render = do_render.clone();
        let renderer = renderer.clone();
        let is_simulating = props.is_simulating;
        let sequence_target_ids = props.sequence_target_ids.clone();
        let has_renderer = renderer.is_some();

        use_effect_with((sequence_target_ids, has_renderer, is_simulating), move |(_, has_renderer, is_sim)| {
            // Re-render immediately when sequence targets change (not during simulation)
            if *has_renderer && !*is_sim {
                do_render();
            }
        });
    }

    // Update cached keyframe selection and re-render when it changes
    {
        let do_render = do_render.clone();
        let renderer = renderer.clone();
        let is_simulating = props.is_simulating;
        let selected_sequence = props.selected_sequence.clone();
        let selected_keyframe = props.selected_keyframe;
        let cached_selected_sequence = cached_selected_sequence.clone();
        let cached_selected_keyframe = cached_selected_keyframe.clone();
        let has_renderer = renderer.is_some();

        use_effect_with((selected_sequence.clone(), selected_keyframe, has_renderer, is_simulating), move |(seq, kf, has_renderer, is_sim)| {
            // Update cached values
            *cached_selected_sequence.borrow_mut() = seq.clone();
            *cached_selected_keyframe.borrow_mut() = *kf;
            // Re-render immediately when keyframe selection changes (not during simulation)
            if *has_renderer && !*is_sim {
                do_render();
            }
        });
    }

    // Resize handler
    {
        let canvas_ref = canvas_ref.clone();
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        use_effect_with((), move |_| {
            let canvas_ref = canvas_ref.clone();
            let renderer = renderer.clone();
            let camera = camera.clone();
            let config = config.clone();

            let resize_cb = Closure::wrap(Box::new(move || {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                    if let Some(window) = web_sys::window() {
                        let w = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(800.0) as u32;
                        let h = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(600.0) as u32;
                        if w > 0 && h > 0 {
                            canvas.set_width(w);
                            canvas.set_height(h);
                            if let Some(renderer) = &*renderer {
                                renderer.borrow_mut().resize(w, h);
                                camera.borrow_mut().set_viewport(w as f32, h as f32);
                                let mut gs = GameState::new(0);
                                gs.load_map(config.clone());
                                renderer.borrow_mut().render(&gs, &camera.borrow());
                            }
                        }
                    }
                }
            }) as Box<dyn Fn()>);

            if let Some(window) = web_sys::window() {
                let _ = window.add_event_listener_with_callback("resize", resize_cb.as_ref().unchecked_ref());
                resize_cb.as_ref().unchecked_ref::<js_sys::Function>().call0(&JsValue::NULL).ok();
            }
            resize_cb.forget();
        });
    }

    // Keyframe preview loop
    {
        let renderer = renderer.clone();
        let camera = camera.clone();
        let preview_sequence = props.preview_sequence.clone();
        let on_preview_complete = props.on_preview_complete.clone();
        let on_preview_keyframe_change = props.on_preview_keyframe_change.clone();
        let preview_game_state = preview_game_state.clone();
        let preview_executor = preview_executor.clone();
        let preview_initial_transforms = preview_initial_transforms.clone();
        let preview_current_positions = preview_current_positions.clone();
        let is_preview_active = is_preview_active.clone();
        let preview_frame_id = use_mut_ref(|| None::<i32>);
        let preview_last_time = use_mut_ref(|| None::<f64>);
        let preview_loop_running = use_mut_ref(|| false);
        let last_keyframe_index: Rc<RefCell<Option<usize>>> = use_mut_ref(|| None);

        use_effect_with((*is_preview_active, preview_sequence.clone()), move |(is_active, seq)| {
            let preview_frame_id_cleanup = preview_frame_id.clone();
            let preview_loop_running_cleanup = preview_loop_running.clone();

            // Stop any existing loop
            *preview_loop_running.borrow_mut() = false;

            if *is_active && seq.is_some() {
                let seq = seq.clone().unwrap();
                // Start preview loop
                *preview_loop_running.borrow_mut() = true;
                *preview_last_time.borrow_mut() = None;

                let renderer = renderer.clone();
                let camera = camera.clone();
                let preview_game_state = preview_game_state.clone();
                let preview_executor = preview_executor.clone();
                let preview_initial_transforms = preview_initial_transforms.clone();
                let preview_current_positions = preview_current_positions.clone();
                let preview_frame_id = preview_frame_id.clone();
                let preview_last_time = preview_last_time.clone();
                let preview_loop_running = preview_loop_running.clone();
                let is_preview_active = is_preview_active.clone();
                let on_preview_complete = on_preview_complete.clone();
                let on_preview_keyframe_change = on_preview_keyframe_change.clone();
                let last_keyframe_index = last_keyframe_index.clone();

                // Reset last keyframe index and notify start
                *last_keyframe_index.borrow_mut() = None;
                on_preview_keyframe_change.emit(Some(0));

                let closure: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
                let closure_clone = closure.clone();

                *closure.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
                    if !*preview_loop_running.borrow() {
                        return;
                    }

                    // Calculate delta time
                    let last = *preview_last_time.borrow();
                    let dt = match last {
                        Some(last_ts) => ((timestamp - last_ts) / 1000.0).min(0.1) as f32,
                        None => 1.0 / 60.0,
                    };
                    *preview_last_time.borrow_mut() = Some(timestamp);

                    // Update keyframe executor
                    let mut finished = false;
                    {
                        let mut executor_opt = preview_executor.borrow_mut();
                        let initials = preview_initial_transforms.borrow();
                        let current_positions = preview_current_positions.borrow().clone();

                        if let Some(executor) = executor_opt.as_mut() {
                            // Create temporary sequence for preview
                            let preview_sequences = vec![seq.clone()];

                            // Get game context from preview game state for random() support
                            let mut curr_pos = preview_current_positions.borrow_mut();
                            if let Some(gs) = &mut *preview_game_state.borrow_mut() {
                                let updates = executor.update(
                                    dt,
                                    &preview_sequences,
                                    &current_positions,
                                    &initials,
                                    gs.game_context_mut(),
                                );

                                // Apply updates to game state and track current positions
                                for (id, pos, rot) in &updates {
                                    gs.set_kinematic_position(id, *pos, *rot);
                                    // Update tracked positions for next frame
                                    curr_pos.insert(id.clone(), (*pos, *rot));
                                }
                                // Step physics to apply kinematic targets to actual positions
                                gs.physics_world.step();

                                finished = executor.is_finished();

                                // Notify keyframe index change
                                let current_idx = executor.current_index();
                                let last_idx = *last_keyframe_index.borrow();
                                if last_idx != Some(current_idx) {
                                    *last_keyframe_index.borrow_mut() = Some(current_idx);
                                    on_preview_keyframe_change.emit(Some(current_idx));
                                }
                            }
                        }
                    }

                    // Render preview state
                    if let Some(renderer) = &*renderer {
                        if let Some(gs) = &*preview_game_state.borrow() {
                            let cam = camera.borrow();
                            renderer.borrow_mut().render_with_overlay(gs, &cam, &[], &[], &[]);
                        }
                    }

                    // Check if finished
                    if finished {
                        *preview_loop_running.borrow_mut() = false;
                        is_preview_active.set(false);
                        on_preview_keyframe_change.emit(None);
                        on_preview_complete.emit(());
                        return;
                    }

                    // Request next frame
                    if *preview_loop_running.borrow() {
                        if let Some(window) = web_sys::window() {
                            if let Some(ref cb) = *closure_clone.borrow() {
                                let id = window
                                    .request_animation_frame(cb.as_ref().unchecked_ref())
                                    .ok();
                                *preview_frame_id.borrow_mut() = id;
                            }
                        }
                    }
                }));

                // Start the loop
                if let Some(window) = web_sys::window() {
                    if let Some(ref cb) = *closure.borrow() {
                        let id = window
                            .request_animation_frame(cb.as_ref().unchecked_ref())
                            .ok();
                        *preview_frame_id_cleanup.borrow_mut() = id;
                    }
                }
            }

            // Cleanup
            move || {
                *preview_loop_running_cleanup.borrow_mut() = false;
                if let Some(id) = *preview_frame_id_cleanup.borrow() {
                    if let Some(window) = web_sys::window() {
                        let _ = window.cancel_animation_frame(id);
                    }
                }
                *preview_frame_id_cleanup.borrow_mut() = None;
            }
        });
    }

    // Simulation loop (physics update + render)
    {
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        let game_state_ref = props.game_state_ref.clone();
        let is_simulating = props.is_simulating;
        let sim_frame_id = use_mut_ref(|| None::<i32>);
        let accumulated_time = use_mut_ref(|| 0.0f64);
        let last_time = use_mut_ref(|| None::<f64>);
        // Flag to signal loop to stop immediately
        let loop_running = use_mut_ref(|| false);

        use_effect_with(is_simulating, move |is_running| {
            let sim_frame_id_cleanup = sim_frame_id.clone();
            let loop_running_cleanup = loop_running.clone();

            // Stop any existing loop immediately
            *loop_running.borrow_mut() = false;

            if *is_running {
                // Start new loop
                *loop_running.borrow_mut() = true;
                *accumulated_time.borrow_mut() = 0.0;
                *last_time.borrow_mut() = None;

                let renderer = renderer.clone();
                let camera = camera.clone();
                let config = config.clone();
                let game_state_ref = game_state_ref.clone();
                let sim_frame_id = sim_frame_id.clone();
                let accumulated_time = accumulated_time.clone();
                let last_time = last_time.clone();
                let loop_running = loop_running.clone();

                let closure: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
                let closure_clone = closure.clone();

                *closure.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
                    // Check if loop should stop
                    if !*loop_running.borrow() {
                        return;
                    }

                    // Get game state from ref - if None, render editor mode and stop loop
                    let gs_opt = game_state_ref.as_ref().and_then(|r| r.borrow().clone());
                    let Some(gs) = gs_opt else {
                        // Game state was reset - render editor mode and stop loop
                        if let Some(renderer) = &*renderer {
                            let mut editor_gs = GameState::new(0);
                            editor_gs.load_map(config.clone());
                            let cam = camera.borrow();
                            renderer.borrow_mut().render_with_overlay(&editor_gs, &cam, &[], &[], &[]);
                        }
                        *loop_running.borrow_mut() = false;
                        return;
                    };

                    // Calculate delta time
                    let last = *last_time.borrow();
                    let dt = match last {
                        Some(last_ts) => (timestamp - last_ts).min(100.0),
                        None => 1000.0 / 60.0,
                    };
                    *last_time.borrow_mut() = Some(timestamp);

                    // Fixed timestep: 60Hz (16.67ms)
                    const FIXED_DT: f64 = 1000.0 / 60.0;
                    *accumulated_time.borrow_mut() += dt;

                    // Update physics and keyframes
                    while *accumulated_time.borrow() >= FIXED_DT {
                        gs.borrow_mut().update();
                        *accumulated_time.borrow_mut() -= FIXED_DT;
                    }

                    // Render current simulation state
                    if let Some(renderer) = &*renderer {
                        let cam = camera.borrow();
                        renderer.borrow_mut().render_with_overlay(&gs.borrow(), &cam, &[], &[], &[]);
                    }

                    // Request next frame only if still running
                    if *loop_running.borrow() {
                        if let Some(window) = web_sys::window() {
                            if let Some(ref cb) = *closure_clone.borrow() {
                                let id = window
                                    .request_animation_frame(cb.as_ref().unchecked_ref())
                                    .ok();
                                *sim_frame_id.borrow_mut() = id;
                            }
                        }
                    }
                }));

                // Start the loop
                if let Some(window) = web_sys::window() {
                    if let Some(ref cb) = *closure.borrow() {
                        let id = window
                            .request_animation_frame(cb.as_ref().unchecked_ref())
                            .ok();
                        *sim_frame_id_cleanup.borrow_mut() = id;
                    }
                }
            }

            // Cleanup
            move || {
                // Signal loop to stop
                *loop_running_cleanup.borrow_mut() = false;
                // Cancel pending animation frame
                if let Some(id) = *sim_frame_id_cleanup.borrow() {
                    if let Some(window) = web_sys::window() {
                        let _ = window.cancel_animation_frame(id);
                    }
                }
                *sim_frame_id_cleanup.borrow_mut() = None;
            }
        });
    }

    // Document-level mouse events for drag outside canvas
    {
        let camera = camera.clone();
        let interaction = interaction.clone();
        let config = props.config.clone();
        let selected_index = props.selected_index;
        let preview_transform = preview_transform.clone();
        let on_object_update = props.on_object_update.clone();
        let do_render = do_render.clone();
        let is_dragging = *is_dragging_state;
        let is_dragging_state = is_dragging_state.clone();
        let canvas_ref = canvas_ref.clone();
        let selected_sequence = props.selected_sequence.clone();
        let selected_sequence_index = props.selected_sequence_index;
        let selected_keyframe = props.selected_keyframe;
        let on_update_keyframe = props.on_update_keyframe.clone();

        use_effect_with(is_dragging, move |is_dragging| {
            // 클린업에 필요한 데이터를 Option으로 감싸서 동일한 클로저 타입 반환
            type ClosureType = Closure<dyn FnMut(web_sys::MouseEvent)>;
            let cleanup_data: Option<(Rc<ClosureType>, Rc<ClosureType>)> = if *is_dragging {
                let camera = camera.clone();
                let interaction = interaction.clone();
                let config = config.clone();
                let selected_index = selected_index;
                let preview_transform = preview_transform.clone();
                let on_object_update = on_object_update.clone();
                let do_render = do_render.clone();
                let is_dragging_state = is_dragging_state.clone();
                let canvas_ref = canvas_ref.clone();

                // Document mousemove handler
                let mousemove_cb = {
                    let camera = camera.clone();
                    let interaction = interaction.clone();
                    let config = config.clone();
                    let preview_transform = preview_transform.clone();
                    let do_render = do_render.clone();
                    let canvas_ref = canvas_ref.clone();
                    let selected_sequence_index = selected_sequence_index;
                    let selected_keyframe = selected_keyframe;

                    Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
                        let mut inter = interaction.borrow_mut();
                        if !inter.is_dragging() {
                            return;
                        }

                        // Get canvas-relative position
                        let (sx, sy) = if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
                            let rect = canvas.get_bounding_client_rect();
                            ((e.client_x() as f64 - rect.left()) as f32, (e.client_y() as f64 - rect.top()) as f32)
                        } else {
                            (e.client_x() as f32, e.client_y() as f32)
                        };

                        let cam_ref = camera.borrow();
                        let world = cam_ref.screen_to_world(sx, sy);
                        let viewport = cam_ref.viewport;
                        drop(cam_ref);

                        inter.update_mouse((sx, sy), world);
                        inter.update_modifiers(e.shift_key(), e.ctrl_key(), e.alt_key());

                        // Auto-pan when near screen edges during drag
                        // pan_by_screen_delta moves camera opposite to delta (natural panning)
                        // So positive delta moves camera left/up, negative moves right/down
                        const EDGE_MARGIN: f32 = 40.0;
                        const PAN_SPEED: f32 = 8.0;
                        let mut pan_x = 0.0f32;
                        let mut pan_y = 0.0f32;

                        // Left edge: move camera left (positive delta)
                        if sx < EDGE_MARGIN { pan_x = PAN_SPEED; }
                        // Right edge: move camera right (negative delta)
                        else if sx > viewport.0 - EDGE_MARGIN { pan_x = -PAN_SPEED; }
                        // Top edge: move camera up (positive delta)
                        if sy < EDGE_MARGIN { pan_y = PAN_SPEED; }
                        // Bottom edge: move camera down (negative delta)
                        else if sy > viewport.1 - EDGE_MARGIN { pan_y = -PAN_SPEED; }

                        if pan_x != 0.0 || pan_y != 0.0 {
                            let mut cam = camera.borrow_mut();
                            let z = cam.zoom;
                            cam.pan_by_screen_delta(pan_x, pan_y);
                            // Camera moves by -pan/zoom in world coords, so drag start moves same amount
                            if let Some(start) = inter.drag_start_world {
                                inter.drag_start_world = Some((start.0 - pan_x / z, start.1 - pan_y / z));
                            }
                        }

                        if let Some(handle) = inter.active_handle {
                            // Pivot handle drag (for PivotRotate keyframe)
                            if handle.is_pivot() {
                                if let Some(orig) = inter.original_pivot_transform {
                                    if let Some(d) = inter.drag_delta() {
                                        if let (Some(seq_idx), Some(kf_idx)) = (selected_sequence_index, selected_keyframe) {
                                            let new_t = gizmo::apply_pivot_transform(&orig, d, inter.shift_held);
                                            *preview_transform.borrow_mut() = Some(PreviewTransform::Pivot(seq_idx, kf_idx, new_t));
                                        }
                                    }
                                }
                            } else if let Some(idx) = selected_index {
                                if idx < config.objects.len() {
                                    if handle.is_bezier() {
                                        if let Some(orig) = inter.original_bezier_transform {
                                            if let Some(d) = inter.drag_delta() {
                                                let new_t = gizmo::apply_bezier_transform(handle, &orig, d, inter.shift_held, inter.alt_held);
                                                *preview_transform.borrow_mut() = Some(PreviewTransform::Bezier(idx, new_t));
                                            }
                                        }
                                    } else if handle.is_line() {
                                        if let Some(orig) = inter.original_line_transform {
                                            if let Some(d) = inter.drag_delta() {
                                                let new_t = gizmo::apply_line_transform(handle, &orig, d, inter.shift_held);
                                                *preview_transform.borrow_mut() = Some(PreviewTransform::Line(idx, new_t));
                                            }
                                        }
                                    } else if let Some(orig) = inter.original_transform {
                                        let new_t = if handle.is_rotate() {
                                            if let (Some(start), Some(curr)) = (inter.drag_start_world, inter.mouse_world) {
                                                gizmo::apply_rotate_transform(&orig, start, curr, inter.shift_held)
                                            } else { orig }
                                        } else if handle.is_scale() {
                                            if let Some(d) = inter.drag_delta() {
                                                gizmo::apply_scale_transform(handle, &orig, d, inter.shift_held)
                                            } else { orig }
                                        } else {
                                            if let Some(d) = inter.drag_delta() {
                                                gizmo::apply_move_transform(handle, &orig, d, inter.shift_held)
                                            } else { orig }
                                        };
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Standard(idx, new_t));
                                    }
                                }
                            }
                        }
                        drop(inter);
                        do_render();
                    }) as Box<dyn FnMut(web_sys::MouseEvent)>)
                };

                // Document mouseup handler
                let mouseup_cb = {
                    let interaction = interaction.clone();
                    let preview_transform = preview_transform.clone();
                    let config = config.clone();
                    let on_object_update = on_object_update.clone();
                    let is_dragging_state = is_dragging_state.clone();
                    let selected_sequence = selected_sequence.clone();
                    let on_update_keyframe = on_update_keyframe.clone();

                    Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                        let mut inter = interaction.borrow_mut();
                        inter.end_drag();
                        drop(inter);
                        is_dragging_state.set(false);

                        let preview = *preview_transform.borrow();
                        match preview {
                            Some(PreviewTransform::Standard(idx, transform)) => {
                                if idx < config.objects.len() {
                                    let mut obj = config.objects[idx].clone();
                                    apply_transform_to_object(&mut obj, &transform);
                                    on_object_update.emit((idx, obj));
                                }
                            }
                            Some(PreviewTransform::Bezier(idx, transform)) => {
                                if idx < config.objects.len() {
                                    let mut obj = config.objects[idx].clone();
                                    apply_bezier_transform_to_object(&mut obj, &transform);
                                    on_object_update.emit((idx, obj));
                                }
                            }
                            Some(PreviewTransform::Line(idx, transform)) => {
                                if idx < config.objects.len() {
                                    let mut obj = config.objects[idx].clone();
                                    apply_line_transform_to_object(&mut obj, &transform);
                                    on_object_update.emit((idx, obj));
                                }
                            }
                            Some(PreviewTransform::Pivot(_seq_idx, kf_idx, pivot_t)) => {
                                // Update keyframe with new pivot position
                                if let Some(seq) = &selected_sequence {
                                    if let Some(kf) = seq.keyframes.get(kf_idx) {
                                        if let Keyframe::PivotRotate { pivot_mode, angle, duration, easing, .. } = kf {
                                            let updated_kf = Keyframe::PivotRotate {
                                                pivot: [pivot_t.point.0, pivot_t.point.1],
                                                pivot_mode: *pivot_mode,
                                                angle: *angle,
                                                duration: *duration,
                                                easing: easing.clone(),
                                            };
                                            on_update_keyframe.emit((kf_idx, updated_kf));
                                        }
                                    }
                                }
                            }
                            Some(PreviewTransform::Ghost(_seq_idx, kf_idx, ghost_t)) => {
                                // Update keyframe with new translation or angle from ghost position
                                if let Some(seq) = &selected_sequence {
                                    if let Some(kf) = seq.keyframes.get(kf_idx) {
                                        let updated_kf = match kf {
                                            Keyframe::Apply { rotation, duration, easing, .. } => {
                                                Keyframe::Apply {
                                                    translation: Some([
                                                        ghost_t.center.0 - ghost_t.init_pos[0],
                                                        ghost_t.center.1 - ghost_t.init_pos[1],
                                                    ]),
                                                    rotation: *rotation,
                                                    duration: *duration,
                                                    easing: easing.clone(),
                                                }
                                            }
                                            Keyframe::PivotRotate { pivot, pivot_mode, duration, easing, .. } => {
                                                let from_angle = (ghost_t.init_pos[1] - pivot[1])
                                                    .atan2(ghost_t.init_pos[0] - pivot[0]);
                                                let to_angle = (ghost_t.center.1 - pivot[1])
                                                    .atan2(ghost_t.center.0 - pivot[0]);
                                                Keyframe::PivotRotate {
                                                    pivot: *pivot,
                                                    pivot_mode: *pivot_mode,
                                                    angle: (to_angle - from_angle).to_degrees(),
                                                    duration: *duration,
                                                    easing: easing.clone(),
                                                }
                                            }
                                            _ => { return; }
                                        };
                                        on_update_keyframe.emit((kf_idx, updated_kf));
                                    }
                                }
                            }
                            None => {}
                        }
                    }) as Box<dyn FnMut(web_sys::MouseEvent)>)
                };

                // Add document listeners
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let _ = document.add_event_listener_with_callback("mousemove", mousemove_cb.as_ref().unchecked_ref());
                        let _ = document.add_event_listener_with_callback("mouseup", mouseup_cb.as_ref().unchecked_ref());
                    }
                }

                Some((Rc::new(mousemove_cb), Rc::new(mouseup_cb)))
            } else {
                None
            };

            // Return cleanup closure
            move || {
                if let Some((mousemove_cleanup, mouseup_cleanup)) = cleanup_data.as_ref() {
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            let _ = document.remove_event_listener_with_callback("mousemove", mousemove_cleanup.as_ref().as_ref().unchecked_ref());
                            let _ = document.remove_event_listener_with_callback("mouseup", mouseup_cleanup.as_ref().as_ref().unchecked_ref());
                        }
                    }
                }
            }
        });
    }

    // Mouse wheel
    let onwheel = {
        let camera = camera.clone();
        let interaction = interaction.clone();
        let do_render = do_render.clone();

        Callback::from(move |e: WheelEvent| {
            e.prevent_default();
            let inter = interaction.borrow();
            let dy = e.delta_y() as f32;
            let (sx, sy) = get_mouse_pos(&e);
            let mut cam = camera.borrow_mut();

            if inter.ctrl_held {
                cam.zoom_at_screen_pos(sx, sy, if dy < 0.0 { 0.1 } else { -0.1 });
            } else if inter.shift_held {
                cam.pan_by_screen_delta(dy, 0.0);
            } else {
                cam.pan_by_screen_delta(0.0, dy);
            }
            drop(cam);
            do_render();
        })
    };

    // Mouse down
    let onmousedown = {
        let camera = camera.clone();
        let interaction = interaction.clone();
        let config = props.config.clone();
        let selected_index = props.selected_index;
        let on_select = props.on_select.clone();
        let context_menu_state = context_menu_state.clone();
        let is_dragging_state = is_dragging_state.clone();
        let canvas_ref = canvas_ref.clone();
        let selected_sequence = props.selected_sequence.clone();
        let selected_sequence_index = props.selected_sequence_index;
        let selected_keyframe = props.selected_keyframe;

        Callback::from(move |e: MouseEvent| {
            // 캔버스에 포커스를 주어 키보드 이벤트 수신
            if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                let _ = canvas.focus();
            }

            // Close context menu on any click
            if context_menu_state.visible {
                context_menu_state.set(ContextMenuState::hide());
            }
            let (sx, sy) = get_mouse_pos(&e);
            let cam = camera.borrow();
            let world = cam.screen_to_world(sx, sy);
            tracing::info!(
                "mousedown btn={} screen=({:.0},{:.0}) world=({:.1},{:.1}) cam_center=({:.1},{:.1}) zoom={:.3} viewport=({:.0},{:.0})",
                e.button(), sx, sy, world.0, world.1, cam.center.0, cam.center.1, cam.zoom, cam.viewport.0, cam.viewport.1
            );

            // Log first few objects for debugging
            if e.button() == 0 {
                let ctx = GameContext::new(0.0, 0);
                for (i, obj) in config.objects.iter().take(3).enumerate() {
                    let shape = obj.shape.evaluate(&ctx);
                    match shape {
                        EvaluatedShape::Circle { center, radius } => {
                            tracing::info!("  obj[{}] Circle center=({:.1},{:.1}) r={:.1}", i, center[0], center[1], radius);
                        }
                        EvaluatedShape::Rect { center, size, .. } => {
                            tracing::info!("  obj[{}] Rect center=({:.1},{:.1}) size=({:.1},{:.1})", i, center[0], center[1], size[0], size[1]);
                        }
                        EvaluatedShape::Line { start, end } => {
                            tracing::info!("  obj[{}] Line ({:.1},{:.1})->({:.1},{:.1})", i, start[0], start[1], end[0], end[1]);
                        }
                        _ => {}
                    }
                }
            }

            let mut inter = interaction.borrow_mut();
            inter.update_mouse((sx, sy), world);

            match e.button() {
                0 => {
                    // Check pivot gizmo first (for PivotRotate keyframe editing)
                    if let Some(kf_idx) = selected_keyframe {
                        if let Some(seq_idx) = selected_sequence_index {
                            if let Some(seq) = &selected_sequence {
                                if let Some(kf) = seq.keyframes.get(kf_idx) {
                                    if let Keyframe::PivotRotate { pivot, .. } = kf {
                                        let pivot_t = PivotTransform { point: (pivot[0], pivot[1]) };
                                        if let Some(handle) = hit_test_pivot_gizmo(&pivot_t, world, cam.zoom) {
                                            tracing::info!("pivot gizmo hit: {:?}", handle);
                                            inter.start_pivot_drag(handle, world, pivot_t);
                                            is_dragging_state.set(true);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check ghost preview hit (for Apply/PivotRotate keyframe dragging)
                    if let Some(kf_idx) = selected_keyframe {
                        if let Some(seq_idx) = selected_sequence_index {
                            if let Some(seq) = &selected_sequence {
                                if let Some(kf) = seq.keyframes.get(kf_idx) {
                                    if matches!(kf, Keyframe::Apply { .. } | Keyframe::PivotRotate { .. }) {
                                        let targets = compute_ghost_targets(kf, seq, &config);
                                        if let Some(ghost_t) = hit_test_ghost(&targets, world, cam.zoom) {
                                            tracing::info!("ghost hit: center=({:.1},{:.1})", ghost_t.center.0, ghost_t.center.1);
                                            inter.start_ghost_drag(GizmoHandle::GhostMove, world, ghost_t);
                                            is_dragging_state.set(true);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check object gizmo hit
                    if let Some(idx) = selected_index {
                        if idx < config.objects.len() {
                            // Check bezier gizmo first
                            if is_bezier_object(&config.objects[idx]) {
                                if let Some(transform) = get_bezier_transform(&config.objects[idx]) {
                                    if let Some(handle) = hit_test_bezier_gizmo(&transform, world, cam.zoom) {
                                        tracing::info!("bezier gizmo hit: {:?}", handle);
                                        inter.start_bezier_drag(handle, world, transform);
                                        is_dragging_state.set(true);
                                        return;
                                    }
                                }
                            } else if is_line_object(&config.objects[idx]) {
                                // Line gizmo
                                if let Some(transform) = get_line_transform(&config.objects[idx]) {
                                    if let Some(handle) = hit_test_line_gizmo(&transform, world, cam.zoom) {
                                        tracing::info!("line gizmo hit: {:?}", handle);
                                        inter.start_line_drag(handle, world, transform);
                                        is_dragging_state.set(true);
                                        return;
                                    }
                                }
                            } else {
                                // Standard gizmo (Circle, Rect)
                                if let Some(transform) = get_object_transform(&config.objects[idx]) {
                                    if let Some(handle) = hit_test_gizmo(&transform, world, cam.zoom) {
                                        tracing::info!("gizmo hit: {:?}", handle);
                                        inter.start_drag(handle, world, transform);
                                        is_dragging_state.set(true);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    // Object selection
                    if let Some(idx) = hit_test_objects(&config, world) {
                        tracing::info!("object hit: {}", idx);
                        on_select.emit(Some(idx));
                    } else {
                        tracing::info!("no object hit, deselecting");
                        on_select.emit(None);
                    }
                }
                1 => {
                    inter.start_panning((sx, sy), cam.center);
                }
                _ => {}
            }
        })
    };

    // Mouse move
    let onmousemove = {
        let camera = camera.clone();
        let interaction = interaction.clone();
        let config = props.config.clone();
        let selected_index = props.selected_index;
        let hovered_handle = hovered_handle.clone();
        let preview_transform = preview_transform.clone();
        let do_render = do_render.clone();
        let selected_sequence = props.selected_sequence.clone();
        let selected_sequence_index = props.selected_sequence_index;
        let selected_keyframe = props.selected_keyframe;

        Callback::from(move |e: MouseEvent| {
            let (sx, sy) = get_mouse_pos(&e);
            let cam_ref = camera.borrow();
            let world = cam_ref.screen_to_world(sx, sy);
            let zoom = cam_ref.zoom;
            let viewport = cam_ref.viewport;
            drop(cam_ref);

            let mut inter = interaction.borrow_mut();
            inter.update_mouse((sx, sy), world);
            inter.update_modifiers(e.shift_key(), e.ctrl_key(), e.alt_key());

            // Panning
            if inter.is_panning {
                if let (Some(delta), Some(start_center)) = (inter.pan_delta(), inter.pan_start_camera_center) {
                    let mut cam = camera.borrow_mut();
                    let z = cam.zoom;
                    cam.set_center(start_center.0 - delta.0 / z, start_center.1 - delta.1 / z);
                }
                drop(inter);
                do_render();
                return;
            }

            // Gizmo drag - update preview only (no parent re-render)
            if inter.is_dragging() {
                // Auto-pan when near screen edges during drag
                // pan_by_screen_delta moves camera opposite to delta (natural panning)
                const EDGE_MARGIN: f32 = 40.0;
                const PAN_SPEED: f32 = 8.0;
                let mut pan_x = 0.0f32;
                let mut pan_y = 0.0f32;

                // Left edge: move camera left (positive delta)
                if sx < EDGE_MARGIN { pan_x = PAN_SPEED; }
                // Right edge: move camera right (negative delta)
                else if sx > viewport.0 - EDGE_MARGIN { pan_x = -PAN_SPEED; }
                // Top edge: move camera up (positive delta)
                if sy < EDGE_MARGIN { pan_y = PAN_SPEED; }
                // Bottom edge: move camera down (negative delta)
                else if sy > viewport.1 - EDGE_MARGIN { pan_y = -PAN_SPEED; }

                if pan_x != 0.0 || pan_y != 0.0 {
                    let mut cam = camera.borrow_mut();
                    let z = cam.zoom;
                    cam.pan_by_screen_delta(pan_x, pan_y);
                    // Camera moves by -pan/zoom in world coords, so drag start moves same amount
                    if let Some(start) = inter.drag_start_world {
                        inter.drag_start_world = Some((start.0 - pan_x / z, start.1 - pan_y / z));
                    }
                }

                if let Some(handle) = inter.active_handle {
                    // Pivot handle drag (for PivotRotate keyframe)
                    if handle.is_pivot() {
                        if let Some(orig) = inter.original_pivot_transform {
                            if let Some(d) = inter.drag_delta() {
                                if let Some(seq_idx) = selected_sequence_index {
                                    if let Some(kf_idx) = selected_keyframe {
                                        let new_t = gizmo::apply_pivot_transform(&orig, d, inter.shift_held);
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Pivot(seq_idx, kf_idx, new_t));
                                    }
                                }
                            }
                        }
                    } else if handle.is_ghost() {
                        // Ghost handle drag (for Apply/PivotRotate destination)
                        if let Some(orig) = inter.original_ghost_transform {
                            if let Some(d) = inter.drag_delta() {
                                if let Some(seq_idx) = selected_sequence_index {
                                    if let Some(kf_idx) = selected_keyframe {
                                        let new_t = gizmo::apply_ghost_transform(&orig, d, inter.shift_held);
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Ghost(seq_idx, kf_idx, new_t));
                                    }
                                }
                            }
                        }
                    } else if let Some(idx) = selected_index {
                        if idx < config.objects.len() {
                            // Bezier handle drag
                            if handle.is_bezier() {
                                if let Some(orig) = inter.original_bezier_transform {
                                    if let Some(d) = inter.drag_delta() {
                                        let new_t = gizmo::apply_bezier_transform(handle, &orig, d, inter.shift_held, inter.alt_held);
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Bezier(idx, new_t));
                                    }
                                }
                            } else if handle.is_line() {
                                // Line handle drag
                                if let Some(orig) = inter.original_line_transform {
                                    if let Some(d) = inter.drag_delta() {
                                        let new_t = gizmo::apply_line_transform(handle, &orig, d, inter.shift_held);
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Line(idx, new_t));
                                    }
                                }
                            } else if let Some(orig) = inter.original_transform {
                                // Standard handle drag (Circle, Rect)
                                let new_t = if handle.is_rotate() {
                                    if let (Some(start), Some(curr)) = (inter.drag_start_world, inter.mouse_world) {
                                        gizmo::apply_rotate_transform(&orig, start, curr, inter.shift_held)
                                    } else { orig }
                                } else if handle.is_scale() {
                                    if let Some(d) = inter.drag_delta() {
                                        gizmo::apply_scale_transform(handle, &orig, d, inter.shift_held)
                                    } else { orig }
                                } else {
                                    if let Some(d) = inter.drag_delta() {
                                        gizmo::apply_move_transform(handle, &orig, d, inter.shift_held)
                                    } else { orig }
                                };
                                *preview_transform.borrow_mut() = Some(PreviewTransform::Standard(idx, new_t));
                            }
                        }
                    }
                }
                drop(inter);
                do_render();
                return;
            }

            // Hover detection (no state trigger, just RefCell update)
            // Check pivot gizmo hover first
            if let Some(kf_idx) = selected_keyframe {
                if let Some(seq) = &selected_sequence {
                    if let Some(kf) = seq.keyframes.get(kf_idx) {
                        if let Keyframe::PivotRotate { pivot, .. } = kf {
                            let pivot_t = PivotTransform { point: (pivot[0], pivot[1]) };
                            if let Some(handle) = hit_test_pivot_gizmo(&pivot_t, world, zoom) {
                                let current = *hovered_handle.borrow();
                                if Some(handle) != current {
                                    *hovered_handle.borrow_mut() = Some(handle);
                                    drop(inter);
                                    do_render();
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // Check ghost preview hover
            if let Some(kf_idx) = selected_keyframe {
                if let Some(seq) = &selected_sequence {
                    if let Some(kf) = seq.keyframes.get(kf_idx) {
                        if matches!(kf, Keyframe::Apply { .. } | Keyframe::PivotRotate { .. }) {
                            let targets = compute_ghost_targets(kf, seq, &config);
                            if hit_test_ghost(&targets, world, zoom).is_some() {
                                let current = *hovered_handle.borrow();
                                if Some(GizmoHandle::GhostMove) != current {
                                    *hovered_handle.borrow_mut() = Some(GizmoHandle::GhostMove);
                                    drop(inter);
                                    do_render();
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // Check object gizmo hover
            if let Some(idx) = selected_index {
                if idx < config.objects.len() {
                    let new_h = if is_bezier_object(&config.objects[idx]) {
                        if let Some(transform) = get_bezier_transform(&config.objects[idx]) {
                            hit_test_bezier_gizmo(&transform, world, zoom)
                        } else {
                            None
                        }
                    } else if is_line_object(&config.objects[idx]) {
                        if let Some(transform) = get_line_transform(&config.objects[idx]) {
                            hit_test_line_gizmo(&transform, world, zoom)
                        } else {
                            None
                        }
                    } else {
                        if let Some(transform) = get_object_transform(&config.objects[idx]) {
                            hit_test_gizmo(&transform, world, zoom)
                        } else {
                            None
                        }
                    };
                    let current = *hovered_handle.borrow();
                    if new_h != current {
                        *hovered_handle.borrow_mut() = new_h;
                        drop(inter);
                        do_render();
                        return;
                    }
                }
            }
        })
    };

    // Mouse up - commit preview transform
    let onmouseup = {
        let interaction = interaction.clone();
        let preview_transform = preview_transform.clone();
        let config = props.config.clone();
        let on_object_update = props.on_object_update.clone();
        let is_dragging_state = is_dragging_state.clone();
        let selected_sequence = props.selected_sequence.clone();
        let on_update_keyframe = props.on_update_keyframe.clone();

        Callback::from(move |_: MouseEvent| {
            let mut inter = interaction.borrow_mut();
            inter.end_panning();
            inter.end_drag();
            drop(inter);
            is_dragging_state.set(false);

            // Commit preview transform to parent (keep preview for smooth transition)
            let preview = *preview_transform.borrow();
            match preview {
                Some(PreviewTransform::Standard(idx, transform)) => {
                    if idx < config.objects.len() {
                        let mut obj = config.objects[idx].clone();
                        apply_transform_to_object(&mut obj, &transform);
                        on_object_update.emit((idx, obj));
                    }
                }
                Some(PreviewTransform::Bezier(idx, transform)) => {
                    if idx < config.objects.len() {
                        let mut obj = config.objects[idx].clone();
                        apply_bezier_transform_to_object(&mut obj, &transform);
                        on_object_update.emit((idx, obj));
                    }
                }
                Some(PreviewTransform::Line(idx, transform)) => {
                    if idx < config.objects.len() {
                        let mut obj = config.objects[idx].clone();
                        apply_line_transform_to_object(&mut obj, &transform);
                        on_object_update.emit((idx, obj));
                    }
                }
                Some(PreviewTransform::Pivot(_seq_idx, kf_idx, pivot_t)) => {
                    // Update keyframe with new pivot position
                    if let Some(seq) = &selected_sequence {
                        if let Some(kf) = seq.keyframes.get(kf_idx) {
                            if let Keyframe::PivotRotate { pivot_mode, angle, duration, easing, .. } = kf {
                                let updated_kf = Keyframe::PivotRotate {
                                    pivot: [pivot_t.point.0, pivot_t.point.1],
                                    pivot_mode: *pivot_mode,
                                    angle: *angle,
                                    duration: *duration,
                                    easing: easing.clone(),
                                };
                                on_update_keyframe.emit((kf_idx, updated_kf));
                            }
                        }
                    }
                }
                Some(PreviewTransform::Ghost(_seq_idx, kf_idx, ghost_t)) => {
                    // Update keyframe with new translation or angle from ghost position
                    if let Some(seq) = &selected_sequence {
                        if let Some(kf) = seq.keyframes.get(kf_idx) {
                            let updated_kf = match kf {
                                Keyframe::Apply { rotation, duration, easing, .. } => {
                                    Keyframe::Apply {
                                        translation: Some([
                                            ghost_t.center.0 - ghost_t.init_pos[0],
                                            ghost_t.center.1 - ghost_t.init_pos[1],
                                        ]),
                                        rotation: *rotation,
                                        duration: *duration,
                                        easing: easing.clone(),
                                    }
                                }
                                Keyframe::PivotRotate { pivot, pivot_mode, duration, easing, .. } => {
                                    let from_angle = (ghost_t.init_pos[1] - pivot[1])
                                        .atan2(ghost_t.init_pos[0] - pivot[0]);
                                    let to_angle = (ghost_t.center.1 - pivot[1])
                                        .atan2(ghost_t.center.0 - pivot[0]);
                                    Keyframe::PivotRotate {
                                        pivot: *pivot,
                                        pivot_mode: *pivot_mode,
                                        angle: (to_angle - from_angle).to_degrees(),
                                        duration: *duration,
                                        easing: easing.clone(),
                                    }
                                }
                                _ => { return; }
                            };
                            on_update_keyframe.emit((kf_idx, updated_kf));
                        }
                    }
                }
                None => {}
            }
            // Don't clear preview here - it will be cleared when config updates
        })
    };

    // Mouse leave - don't cancel drag, just clear hover
    let onmouseleave = {
        let interaction = interaction.clone();
        let hovered_handle = hovered_handle.clone();
        let do_render = do_render.clone();

        Callback::from(move |_: MouseEvent| {
            let inter = interaction.borrow();
            // If dragging, don't do anything - let the drag continue
            if inter.is_dragging() || inter.is_panning {
                return;
            }
            drop(inter);
            *hovered_handle.borrow_mut() = None;
            do_render();
        })
    };

    // Key handlers
    let onkeydown = {
        let interaction = interaction.clone();
        let render_trigger = render_trigger.clone();
        let selected_index = props.selected_index;
        let on_copy = props.on_copy.clone();
        let on_paste = props.on_paste.clone();
        let on_delete = props.on_delete.clone();
        let camera = camera.clone();

        Callback::from(move |e: KeyboardEvent| {
            let mut inter = interaction.borrow_mut();
            inter.update_modifiers(e.shift_key(), e.ctrl_key(), e.alt_key());

            // Escape - 드래그 취소
            if e.key() == "Escape" {
                inter.cancel_drag();
            }

            // Ctrl+C - 복사
            if e.ctrl_key() && e.key() == "c" {
                if let Some(idx) = selected_index {
                    e.prevent_default();
                    on_copy.emit(idx);
                }
            }

            // Ctrl+V - 붙여넣기
            if e.ctrl_key() && e.key() == "v" {
                e.prevent_default();
                // 마우스 월드 좌표 사용, 없으면 카메라 중심
                let pos = inter.mouse_world.unwrap_or_else(|| {
                    let cam = camera.borrow();
                    cam.center
                });
                on_paste.emit(pos);
            }

            // Delete - 삭제
            if e.key() == "Delete" {
                if let Some(idx) = selected_index {
                    e.prevent_default();
                    on_delete.emit(idx);
                }
            }

            drop(inter);
            render_trigger.force_update();
        })
    };

    let onkeyup = {
        let interaction = interaction.clone();
        Callback::from(move |e: KeyboardEvent| {
            interaction.borrow_mut().update_modifiers(e.shift_key(), e.ctrl_key(), e.alt_key());
        })
    };

    let oncontextmenu = {
        let camera = camera.clone();
        let config = props.config.clone();
        let context_menu_state = context_menu_state.clone();
        let selected_index = props.selected_index;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // Canvas-relative position for world coordinate calculation
            let (canvas_x, canvas_y) = get_mouse_pos(&e);
            // Client position for fixed-position menu
            let screen_x = e.client_x() as f32;
            let screen_y = e.client_y() as f32;
            let cam = camera.borrow();
            let world = cam.screen_to_world(canvas_x, canvas_y);
            drop(cam);

            // Check if clicking on an object or selected gizmo
            let target_index = if let Some(idx) = selected_index {
                // If we have a selection, check if clicking on that object
                if idx < config.objects.len() {
                    let ctx = marble_core::GameContext::new(0.0, 0);
                    let hit = match config.objects[idx].shape.evaluate(&ctx) {
                        marble_core::map::EvaluatedShape::Circle { center, radius } => {
                            let d = ((world.0 - center[0]).powi(2) + (world.1 - center[1]).powi(2)).sqrt();
                            d <= radius
                        }
                        marble_core::map::EvaluatedShape::Rect { center, size, rotation } => {
                            let r = -rotation.to_radians();
                            let dx = world.0 - center[0];
                            let dy = world.1 - center[1];
                            let lx = dx * r.cos() - dy * r.sin();
                            let ly = dx * r.sin() + dy * r.cos();
                            lx.abs() <= size[0] / 2.0 && ly.abs() <= size[1] / 2.0
                        }
                        _ => false,
                    };
                    if hit { Some(idx) } else { hit_test_objects(&config, world) }
                } else {
                    hit_test_objects(&config, world)
                }
            } else {
                hit_test_objects(&config, world)
            };

            context_menu_state.set(ContextMenuState::show((screen_x, screen_y), world, target_index));
        })
    };

    // Context menu callbacks
    let on_context_close = {
        let context_menu_state = context_menu_state.clone();
        Callback::from(move |_: ()| {
            context_menu_state.set(ContextMenuState::hide());
        })
    };

    html! {
        <>
            <canvas
                ref={canvas_ref}
                class="editor-canvas-fullscreen"
                tabindex="0"
                {onwheel}
                {onmousedown}
                {onmousemove}
                {onmouseup}
                {onmouseleave}
                {onkeydown}
                {onkeyup}
                {oncontextmenu}
            />
            if renderer.is_none() {
                <div class="editor-canvas-loading">
                    <div class="loading-spinner" />
                    <span>{"Initializing renderer..."}</span>
                </div>
            }
            <ContextMenu
                state={(*context_menu_state).clone()}
                has_clipboard={props.has_clipboard}
                on_close={on_context_close}
                on_copy={props.on_copy.clone()}
                on_paste={props.on_paste.clone()}
                on_delete={props.on_delete.clone()}
                on_mirror_x={props.on_mirror_x.clone()}
                on_mirror_y={props.on_mirror_y.clone()}
            />
        </>
    }
}

fn get_mouse_pos(e: &MouseEvent) -> (f32, f32) {
    if let Some(el) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
        let rect = el.get_bounding_client_rect();
        ((e.client_x() as f64 - rect.left()) as f32, (e.client_y() as f64 - rect.top()) as f32)
    } else {
        (e.offset_x() as f32, e.offset_y() as f32)
    }
}

fn get_object_transform(obj: &MapObject) -> Option<ObjectTransform> {
    let ctx = GameContext::new(0.0, 0);
    match obj.shape.evaluate(&ctx) {
        EvaluatedShape::Circle { center, radius } => Some(ObjectTransform {
            center: (center[0], center[1]),
            size: (radius * 2.0, radius * 2.0),
            rotation: 0.0,
        }),
        EvaluatedShape::Rect { center, size, rotation } => Some(ObjectTransform {
            center: (center[0], center[1]),
            size: (size[0], size[1]),
            rotation,
        }),
        // Line and Bezier use their own transform types
        EvaluatedShape::Line { .. } | EvaluatedShape::Bezier { .. } => None,
    }
}

fn get_bezier_transform(obj: &MapObject) -> Option<BezierTransform> {
    let ctx = GameContext::new(0.0, 0);
    match obj.shape.evaluate(&ctx) {
        EvaluatedShape::Bezier { start, control1, control2, end, .. } => Some(BezierTransform {
            start: (start[0], start[1]),
            control1: (control1[0], control1[1]),
            control2: (control2[0], control2[1]),
            end: (end[0], end[1]),
        }),
        _ => None,
    }
}

fn is_bezier_object(obj: &MapObject) -> bool {
    matches!(obj.shape, Shape::Bezier { .. })
}

fn is_line_object(obj: &MapObject) -> bool {
    matches!(obj.shape, Shape::Line { .. })
}

fn get_line_transform(obj: &MapObject) -> Option<LineTransform> {
    let ctx = GameContext::new(0.0, 0);
    match obj.shape.evaluate(&ctx) {
        EvaluatedShape::Line { start, end } => Some(LineTransform {
            start: (start[0], start[1]),
            end: (end[0], end[1]),
        }),
        _ => None,
    }
}

/// Round coordinate to integer.
fn snap(v: f32) -> f32 {
    v.round()
}

fn apply_transform_to_object(obj: &mut MapObject, t: &ObjectTransform) {
    match &mut obj.shape {
        Shape::Circle { center, radius } => {
            *center = Vec2OrExpr::Static([snap(t.center.0), snap(t.center.1)]);
            *radius = NumberOrExpr::Number(snap(t.size.0 / 2.0));
        }
        Shape::Rect { center, size, rotation } => {
            *center = Vec2OrExpr::Static([snap(t.center.0), snap(t.center.1)]);
            *size = Vec2OrExpr::Static([snap(t.size.0), snap(t.size.1)]);
            *rotation = NumberOrExpr::Number(t.rotation.round());
        }
        // Line and Bezier use their own transform functions
        Shape::Line { .. } | Shape::Bezier { .. } => {}
    }
}

fn apply_bezier_transform_to_object(obj: &mut MapObject, t: &BezierTransform) {
    if let Shape::Bezier { start, control1, control2, end, .. } = &mut obj.shape {
        *start = Vec2OrExpr::Static([snap(t.start.0), snap(t.start.1)]);
        *control1 = Vec2OrExpr::Static([snap(t.control1.0), snap(t.control1.1)]);
        *control2 = Vec2OrExpr::Static([snap(t.control2.0), snap(t.control2.1)]);
        *end = Vec2OrExpr::Static([snap(t.end.0), snap(t.end.1)]);
    }
}

fn apply_line_transform_to_object(obj: &mut MapObject, t: &LineTransform) {
    if let Shape::Line { start, end } = &mut obj.shape {
        *start = Vec2OrExpr::Static([snap(t.start.0), snap(t.start.1)]);
        *end = Vec2OrExpr::Static([snap(t.end.0), snap(t.end.1)]);
    }
}

fn hit_test_objects(config: &RouletteConfig, world: (f32, f32)) -> Option<usize> {
    let ctx = GameContext::new(0.0, 0);
    for (idx, obj) in config.objects.iter().enumerate().rev() {
        let hit = match obj.shape.evaluate(&ctx) {
            EvaluatedShape::Circle { center, radius } => {
                let d = ((world.0 - center[0]).powi(2) + (world.1 - center[1]).powi(2)).sqrt();
                d <= radius
            }
            EvaluatedShape::Rect { center, size, rotation } => {
                let r = -rotation.to_radians();
                let dx = world.0 - center[0];
                let dy = world.1 - center[1];
                let lx = dx * r.cos() - dy * r.sin();
                let ly = dx * r.sin() + dy * r.cos();
                lx.abs() <= size[0] / 2.0 && ly.abs() <= size[1] / 2.0
            }
            EvaluatedShape::Line { start, end } => {
                let tol = 8.0;
                let len_sq = (end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2);
                if len_sq < 0.0001 {
                    ((world.0 - start[0]).powi(2) + (world.1 - start[1]).powi(2)).sqrt() < tol
                } else {
                    let t = ((world.0 - start[0]) * (end[0] - start[0]) + (world.1 - start[1]) * (end[1] - start[1])) / len_sq;
                    let t = t.clamp(0.0, 1.0);
                    let px = start[0] + t * (end[0] - start[0]);
                    let py = start[1] + t * (end[1] - start[1]);
                    ((world.0 - px).powi(2) + (world.1 - py).powi(2)).sqrt() < tol
                }
            }
            EvaluatedShape::Bezier { start, control1, control2, end, .. } => {
                hit_test_bezier_curve(world, start, control1, control2, end, 8.0)
            }
        };
        if hit { return Some(idx); }
    }
    None
}

/// Hit test for bezier curve using polyline approximation.
fn hit_test_bezier_curve(
    point: (f32, f32),
    start: [f32; 2],
    control1: [f32; 2],
    control2: [f32; 2],
    end: [f32; 2],
    tolerance: f32,
) -> bool {
    // Sample the bezier curve into segments and check distance to each
    const SEGMENTS: usize = 20;

    for i in 0..SEGMENTS {
        let t0 = i as f32 / SEGMENTS as f32;
        let t1 = (i + 1) as f32 / SEGMENTS as f32;

        let p0 = evaluate_bezier(t0, start, control1, control2, end);
        let p1 = evaluate_bezier(t1, start, control1, control2, end);

        // Point-to-segment distance
        let seg_len_sq = (p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2);
        if seg_len_sq < 0.0001 {
            let d = ((point.0 - p0.0).powi(2) + (point.1 - p0.1).powi(2)).sqrt();
            if d < tolerance { return true; }
        } else {
            let t = ((point.0 - p0.0) * (p1.0 - p0.0) + (point.1 - p0.1) * (p1.1 - p0.1)) / seg_len_sq;
            let t = t.clamp(0.0, 1.0);
            let proj_x = p0.0 + t * (p1.0 - p0.0);
            let proj_y = p0.1 + t * (p1.1 - p0.1);
            let d = ((point.0 - proj_x).powi(2) + (point.1 - proj_y).powi(2)).sqrt();
            if d < tolerance { return true; }
        }
    }
    false
}

/// Compute ghost target centers for hit testing and rendering.
/// Returns: Vec<(dest_center, init_pos, init_rot)>
fn compute_ghost_targets(
    keyframe: &Keyframe,
    seq: &KeyframeSequence,
    config: &RouletteConfig,
) -> Vec<((f32, f32), [f32; 2], f32)> {
    let ctx = GameContext::new(0.0, 0);
    let mut targets = Vec::new();

    for target_id in &seq.target_ids {
        for obj in &config.objects {
            if obj.id.as_ref() == Some(target_id) {
                let shape = obj.shape.evaluate(&ctx);
                let (init_pos, init_rot) = match &shape {
                    EvaluatedShape::Circle { center, .. } => (*center, 0.0),
                    EvaluatedShape::Rect { center, rotation, .. } => (*center, rotation.to_radians()),
                    EvaluatedShape::Line { start, end } => {
                        ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                    }
                    EvaluatedShape::Bezier { start, end, .. } => {
                        ([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], 0.0)
                    }
                };

                let dest_pos = match keyframe {
                    Keyframe::Apply { translation, .. } => {
                        let t = translation.unwrap_or([0.0, 0.0]);
                        (init_pos[0] + t[0], init_pos[1] + t[1])
                    }
                    Keyframe::PivotRotate { pivot, angle, .. } => {
                        let offset = [init_pos[0] - pivot[0], init_pos[1] - pivot[1]];
                        let angle_rad = angle.to_radians();
                        let cos = angle_rad.cos();
                        let sin = angle_rad.sin();
                        (
                            pivot[0] + offset[0] * cos - offset[1] * sin,
                            pivot[1] + offset[0] * sin + offset[1] * cos,
                        )
                    }
                    _ => continue,
                };

                targets.push((dest_pos, init_pos, init_rot));
                break;
            }
        }
    }
    targets
}

/// Evaluate cubic bezier curve at parameter t.
fn evaluate_bezier(t: f32, start: [f32; 2], ctrl1: [f32; 2], ctrl2: [f32; 2], end: [f32; 2]) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    let x = mt3 * start[0] + 3.0 * mt2 * t * ctrl1[0] + 3.0 * mt * t2 * ctrl2[0] + t3 * end[0];
    let y = mt3 * start[1] + 3.0 * mt2 * t * ctrl1[1] + 3.0 * mt * t2 * ctrl2[1] + t3 * end[1];

    (x, y)
}
