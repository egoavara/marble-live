//! Editor canvas with Blender-style unified gizmo.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use marble_core::map::{EvaluatedShape, MapObject, RouletteConfig, Shape};
use marble_core::{GameContext, GameState};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::gizmo::{self, generate_bezier_gizmo, generate_gizmo, hit_test_bezier_gizmo, hit_test_gizmo};
use super::interaction::{BezierTransform, EditorInteractionState, GizmoHandle, ObjectTransform};
use crate::camera::{CameraMode, CameraState};
use crate::renderer::WgpuRenderer;

/// Preview transform during drag (either standard or bezier).
#[derive(Debug, Clone, Copy)]
pub enum PreviewTransform {
    Standard(usize, ObjectTransform),
    Bezier(usize, BezierTransform),
}

#[derive(Properties, PartialEq)]
pub struct EditorCanvasProps {
    pub config: RouletteConfig,
    pub selected_index: Option<usize>,
    #[prop_or_default]
    pub on_object_update: Callback<(usize, MapObject)>,
    #[prop_or_default]
    pub on_select: Callback<Option<usize>>,
}

#[function_component(EditorCanvas)]
pub fn editor_canvas(props: &EditorCanvasProps) -> Html {
    let canvas_ref = use_node_ref();
    let renderer: UseStateHandle<Option<Rc<RefCell<WgpuRenderer>>>> = use_state(|| None);
    let camera = use_mut_ref(|| {
        let mut cam = CameraState::new((800.0, 600.0), ((0.0, 0.0), (800.0, 600.0)));
        cam.set_mode(CameraMode::Overview);
        cam
    });
    let interaction = use_mut_ref(EditorInteractionState::new);
    let hovered_handle = use_mut_ref(|| None::<GizmoHandle>);
    // Local preview transform during drag (doesn't trigger parent re-render)
    let preview_transform = use_mut_ref(|| None::<PreviewTransform>);
    let render_trigger = use_force_update();

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

    // Render helper
    let do_render = {
        let renderer = renderer.clone();
        let camera = camera.clone();
        let config = props.config.clone();
        let selected_index = props.selected_index;
        let hovered_handle = hovered_handle.clone();
        let preview_transform = preview_transform.clone();

        Rc::new(move || {
            if let Some(renderer) = &*renderer {
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
                    None => {}
                }

                let mut game_state = GameState::new(0);
                game_state.load_map(render_config);
                let cam = camera.borrow();
                let hovered = *hovered_handle.borrow();

                let (oc, ol, or) = if let Some(idx) = selected_index {
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
                        } else {
                            // Standard object
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

        Callback::from(move |e: MouseEvent| {
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
                    // Check gizmo hit first
                    if let Some(idx) = selected_index {
                        if idx < config.objects.len() {
                            // Check bezier gizmo first
                            if is_bezier_object(&config.objects[idx]) {
                                if let Some(transform) = get_bezier_transform(&config.objects[idx]) {
                                    if let Some(handle) = hit_test_bezier_gizmo(&transform, world, cam.zoom) {
                                        tracing::info!("bezier gizmo hit: {:?}", handle);
                                        inter.start_bezier_drag(handle, world, transform);
                                        return;
                                    }
                                }
                            } else {
                                // Standard gizmo
                                if let Some(transform) = get_object_transform(&config.objects[idx]) {
                                    if let Some(handle) = hit_test_gizmo(&transform, world, cam.zoom) {
                                        tracing::info!("gizmo hit: {:?}", handle);
                                        inter.start_drag(handle, world, transform);
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

        Callback::from(move |e: MouseEvent| {
            let (sx, sy) = get_mouse_pos(&e);
            let cam_ref = camera.borrow();
            let world = cam_ref.screen_to_world(sx, sy);
            let zoom = cam_ref.zoom;
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
                if let Some(handle) = inter.active_handle {
                    if let Some(idx) = selected_index {
                        if idx < config.objects.len() {
                            // Bezier handle drag
                            if handle.is_bezier() {
                                if let Some(orig) = inter.original_bezier_transform {
                                    if let Some(d) = inter.drag_delta() {
                                        let new_t = gizmo::apply_bezier_transform(handle, &orig, d, inter.shift_held, inter.alt_held);
                                        *preview_transform.borrow_mut() = Some(PreviewTransform::Bezier(idx, new_t));
                                    }
                                }
                            } else if let Some(orig) = inter.original_transform {
                                // Standard handle drag
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
            if let Some(idx) = selected_index {
                if idx < config.objects.len() {
                    let new_h = if is_bezier_object(&config.objects[idx]) {
                        if let Some(transform) = get_bezier_transform(&config.objects[idx]) {
                            hit_test_bezier_gizmo(&transform, world, zoom)
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

        Callback::from(move |_: MouseEvent| {
            let mut inter = interaction.borrow_mut();
            inter.end_panning();
            inter.end_drag();
            drop(inter);

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
                None => {}
            }
            // Don't clear preview here - it will be cleared when config updates
        })
    };

    // Mouse leave
    let onmouseleave = {
        let interaction = interaction.clone();
        let hovered_handle = hovered_handle.clone();
        let preview_transform = preview_transform.clone();
        let do_render = do_render.clone();

        Callback::from(move |_: MouseEvent| {
            let mut inter = interaction.borrow_mut();
            inter.end_panning();
            inter.end_drag();
            inter.mouse_screen = None;
            inter.mouse_world = None;
            drop(inter);
            *hovered_handle.borrow_mut() = None;
            *preview_transform.borrow_mut() = None;
            do_render();
        })
    };

    // Key handlers
    let onkeydown = {
        let interaction = interaction.clone();
        let render_trigger = render_trigger.clone();
        Callback::from(move |e: KeyboardEvent| {
            let mut inter = interaction.borrow_mut();
            inter.update_modifiers(e.shift_key(), e.ctrl_key(), e.alt_key());
            if e.key() == "Escape" { inter.cancel_drag(); }
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

    let oncontextmenu = Callback::from(|e: MouseEvent| e.prevent_default());

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
        EvaluatedShape::Line { start, end } => {
            let cx = (start[0] + end[0]) / 2.0;
            let cy = (start[1] + end[1]) / 2.0;
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            Some(ObjectTransform {
                center: (cx, cy),
                size: ((dx * dx + dy * dy).sqrt(), 4.0),
                rotation: dy.atan2(dx).to_degrees(),
            })
        }
        EvaluatedShape::Bezier { .. } => None,
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

fn apply_transform_to_object(obj: &mut MapObject, t: &ObjectTransform) {
    match &mut obj.shape {
        Shape::Circle { center, radius } => {
            *center = Vec2OrExpr::Static([t.center.0, t.center.1]);
            *radius = NumberOrExpr::Number(t.size.0 / 2.0);
        }
        Shape::Rect { center, size, rotation } => {
            *center = Vec2OrExpr::Static([t.center.0, t.center.1]);
            *size = Vec2OrExpr::Static([t.size.0, t.size.1]);
            *rotation = NumberOrExpr::Number(t.rotation);
        }
        Shape::Line { start, end } => {
            let hl = t.size.0 / 2.0;
            let r = t.rotation.to_radians();
            let dx = hl * r.cos();
            let dy = hl * r.sin();
            *start = Vec2OrExpr::Static([t.center.0 - dx, t.center.1 - dy]);
            *end = Vec2OrExpr::Static([t.center.0 + dx, t.center.1 + dy]);
        }
        Shape::Bezier { .. } => {}
    }
}

fn apply_bezier_transform_to_object(obj: &mut MapObject, t: &BezierTransform) {
    if let Shape::Bezier { start, control1, control2, end, .. } = &mut obj.shape {
        *start = Vec2OrExpr::Static([t.start.0, t.start.1]);
        *control1 = Vec2OrExpr::Static([t.control1.0, t.control1.1]);
        *control2 = Vec2OrExpr::Static([t.control2.0, t.control2.1]);
        *end = Vec2OrExpr::Static([t.end.0, t.end.1]);
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
