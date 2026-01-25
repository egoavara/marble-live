//! Editor state management hook.

use std::rc::Rc;

use marble_core::map::{MapMeta, MapObject, ObjectRole, RouletteConfig, Shape};
use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use yew::prelude::*;

const STORAGE_KEY: &str = "marble-live-editor-state";

/// Editor state.
#[derive(Clone, PartialEq)]
pub struct EditorState {
    /// Current map configuration.
    pub config: RouletteConfig,
    /// Currently selected object index.
    pub selected_object: Option<usize>,
    /// Whether there are unsaved changes.
    pub is_dirty: bool,
    /// Clipboard for copy/paste operations.
    pub clipboard: Option<MapObject>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            config: RouletteConfig::default_classic(),
            selected_object: None,
            is_dirty: false,
            clipboard: None,
        }
    }
}

/// Editor actions for state updates.
pub enum EditorAction {
    /// Load a new configuration.
    LoadConfig(RouletteConfig),
    /// Select an object by index.
    SelectObject(Option<usize>),
    /// Update an object at the given index.
    UpdateObject { index: usize, object: MapObject },
    /// Add a new object.
    AddObject(MapObject),
    /// Delete an object by index.
    DeleteObject(usize),
    /// Update map metadata.
    UpdateMeta(MapMeta),
    /// Create a new empty map.
    NewMap,
    /// Mark as saved (clear dirty flag).
    MarkSaved,
    /// Copy an object to clipboard.
    CopyObject(usize),
    /// Paste clipboard object at given world position.
    PasteObject { x: f32, y: f32 },
    /// Mirror object X-axis (left-right flip).
    MirrorObjectX(usize),
    /// Mirror object Y-axis (top-bottom flip).
    MirrorObjectY(usize),
}

impl Reducible for EditorState {
    type Action = EditorAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            EditorAction::LoadConfig(config) => Rc::new(Self {
                config,
                selected_object: None,
                is_dirty: false,
                clipboard: None,
            }),
            EditorAction::SelectObject(index) => Rc::new(Self {
                selected_object: index,
                ..(*self).clone()
            }),
            EditorAction::UpdateObject { index, object } => {
                let mut config = self.config.clone();
                if index < config.objects.len() {
                    config.objects[index] = object;
                }
                Rc::new(Self {
                    config,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }
            EditorAction::AddObject(object) => {
                let mut config = self.config.clone();
                config.objects.push(object);
                let new_index = config.objects.len() - 1;
                Rc::new(Self {
                    config,
                    selected_object: Some(new_index),
                    is_dirty: true,
                    clipboard: self.clipboard.clone(),
                })
            }
            EditorAction::DeleteObject(index) => {
                let mut config = self.config.clone();
                if index < config.objects.len() {
                    config.objects.remove(index);
                }
                let selected = if config.objects.is_empty() {
                    None
                } else if let Some(sel) = self.selected_object {
                    if sel >= config.objects.len() {
                        Some(config.objects.len().saturating_sub(1))
                    } else if sel > index {
                        Some(sel - 1)
                    } else if sel == index {
                        if sel > 0 { Some(sel - 1) } else if config.objects.is_empty() { None } else { Some(0) }
                    } else {
                        Some(sel)
                    }
                } else {
                    None
                };
                Rc::new(Self {
                    config,
                    selected_object: selected,
                    is_dirty: true,
                    clipboard: self.clipboard.clone(),
                })
            }
            EditorAction::UpdateMeta(meta) => {
                let mut config = self.config.clone();
                config.meta = meta;
                Rc::new(Self {
                    config,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }
            EditorAction::NewMap => {
                let config = RouletteConfig {
                    meta: MapMeta {
                        name: "New Map".to_string(),
                        gamerule: vec![],
                        live_ranking: Default::default(),
                    },
                    objects: vec![],
                    keyframes: vec![],
                };
                Rc::new(Self {
                    config,
                    selected_object: None,
                    is_dirty: true,
                    clipboard: None,
                })
            }
            EditorAction::MarkSaved => Rc::new(Self {
                is_dirty: false,
                ..(*self).clone()
            }),
            EditorAction::CopyObject(index) => {
                if index < self.config.objects.len() {
                    Rc::new(Self {
                        clipboard: Some(self.config.objects[index].clone()),
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
            EditorAction::PasteObject { x, y } => {
                if let Some(obj) = &self.clipboard {
                    let mut new_obj = obj.clone();
                    // Move object center to paste position
                    move_object_center(&mut new_obj, x, y);
                    let mut config = self.config.clone();
                    config.objects.push(new_obj);
                    let new_index = config.objects.len() - 1;
                    Rc::new(Self {
                        config,
                        selected_object: Some(new_index),
                        is_dirty: true,
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
            EditorAction::MirrorObjectX(index) => {
                if index < self.config.objects.len() {
                    let mut config = self.config.clone();
                    mirror_object_x(&mut config.objects[index]);
                    Rc::new(Self {
                        config,
                        is_dirty: true,
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
            EditorAction::MirrorObjectY(index) => {
                if index < self.config.objects.len() {
                    let mut config = self.config.clone();
                    mirror_object_y(&mut config.objects[index]);
                    Rc::new(Self {
                        config,
                        is_dirty: true,
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
        }
    }
}

/// Editor state handle returned by `use_editor_state`.
#[derive(Clone)]
pub struct EditorStateHandle {
    pub config: RouletteConfig,
    pub selected_object: Option<usize>,
    pub is_dirty: bool,
    pub clipboard: Option<MapObject>,
    pub on_new: Callback<()>,
    pub on_load: Callback<RouletteConfig>,
    pub on_save: Callback<()>,
    pub on_select: Callback<Option<usize>>,
    pub on_add: Callback<MapObject>,
    pub on_delete: Callback<usize>,
    pub on_update_meta: Callback<MapMeta>,
    pub on_update_object: Callback<(usize, MapObject)>,
    pub on_copy: Callback<usize>,
    pub on_paste: Callback<(f32, f32)>,
    pub on_mirror_x: Callback<usize>,
    pub on_mirror_y: Callback<usize>,
}

/// Hook for managing editor state with localStorage persistence.
#[hook]
pub fn use_editor_state() -> EditorStateHandle {
    let state = use_reducer(|| {
        // Try to load from localStorage
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            if let Ok(Some(json)) = storage.get_item(STORAGE_KEY) {
                if let Ok(config) = serde_json::from_str::<RouletteConfig>(&json) {
                    return EditorState {
                        config,
                        selected_object: None,
                        is_dirty: false,
                        clipboard: None,
                    };
                }
            }
        }
        EditorState::default()
    });

    // Save to localStorage when config changes (use JSON hash as dependency)
    {
        let config = state.config.clone();
        let config_json = serde_json::to_string(&config).unwrap_or_default();
        use_effect_with(config_json.clone(), move |json| {
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                let _ = storage.set_item(STORAGE_KEY, json);
            }
        });
    }

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            state.dispatch(EditorAction::NewMap);
        })
    };

    let on_load = {
        let state = state.clone();
        Callback::from(move |config: RouletteConfig| {
            state.dispatch(EditorAction::LoadConfig(config));
        })
    };

    let on_save = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            state.dispatch(EditorAction::MarkSaved);
        })
    };

    let on_select = {
        let state = state.clone();
        Callback::from(move |index: Option<usize>| {
            state.dispatch(EditorAction::SelectObject(index));
        })
    };

    let on_add = {
        let state = state.clone();
        Callback::from(move |object: MapObject| {
            state.dispatch(EditorAction::AddObject(object));
        })
    };

    let on_delete = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            state.dispatch(EditorAction::DeleteObject(index));
        })
    };

    let on_update_meta = {
        let state = state.clone();
        Callback::from(move |meta: MapMeta| {
            state.dispatch(EditorAction::UpdateMeta(meta));
        })
    };

    let on_update_object = {
        let state = state.clone();
        Callback::from(move |(index, object): (usize, MapObject)| {
            state.dispatch(EditorAction::UpdateObject { index, object });
        })
    };

    let on_copy = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            state.dispatch(EditorAction::CopyObject(index));
        })
    };

    let on_paste = {
        let state = state.clone();
        Callback::from(move |(x, y): (f32, f32)| {
            state.dispatch(EditorAction::PasteObject { x, y });
        })
    };

    let on_mirror_x = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            state.dispatch(EditorAction::MirrorObjectX(index));
        })
    };

    let on_mirror_y = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            state.dispatch(EditorAction::MirrorObjectY(index));
        })
    };

    EditorStateHandle {
        config: state.config.clone(),
        selected_object: state.selected_object,
        is_dirty: state.is_dirty,
        clipboard: state.clipboard.clone(),
        on_new,
        on_load,
        on_save,
        on_select,
        on_add,
        on_delete,
        on_update_meta,
        on_update_object,
        on_copy,
        on_paste,
        on_mirror_x,
        on_mirror_y,
    }
}

/// Creates a default obstacle object.
pub fn create_default_obstacle() -> MapObject {
    MapObject {
        id: None,
        role: ObjectRole::Obstacle,
        shape: Shape::Circle {
            center: Vec2OrExpr::Static([400.0, 300.0]),
            radius: NumberOrExpr::Number(30.0),
        },
        properties: Default::default(),
    }
}

/// Creates a default spawner object.
pub fn create_default_spawner() -> MapObject {
    MapObject {
        id: None,
        role: ObjectRole::Spawner,
        shape: Shape::Rect {
            center: Vec2OrExpr::Static([400.0, 100.0]),
            size: Vec2OrExpr::Static([200.0, 50.0]),
            rotation: Default::default(),
        },
        properties: Default::default(),
    }
}

/// Creates a default trigger object.
pub fn create_default_trigger() -> MapObject {
    MapObject {
        id: None,
        role: ObjectRole::Trigger,
        shape: Shape::Circle {
            center: Vec2OrExpr::Static([400.0, 500.0]),
            radius: NumberOrExpr::Number(40.0),
        },
        properties: Default::default(),
    }
}

/// Round to integer.
fn snap(v: f32) -> f32 {
    v.round()
}

/// Move an object's center to a new position (snapped to integer).
fn move_object_center(obj: &mut MapObject, x: f32, y: f32) {
    let x = snap(x);
    let y = snap(y);
    match &mut obj.shape {
        Shape::Circle { center, .. } => {
            *center = Vec2OrExpr::Static([x, y]);
        }
        Shape::Rect { center, .. } => {
            *center = Vec2OrExpr::Static([x, y]);
        }
        Shape::Line { start, end } => {
            // Calculate current center and move both endpoints
            let (sx, sy) = match start {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => (0.0, 0.0),
            };
            let (ex, ey) = match end {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => (0.0, 0.0),
            };
            let cx = (sx + ex) / 2.0;
            let cy = (sy + ey) / 2.0;
            let dx = x - cx;
            let dy = y - cy;
            *start = Vec2OrExpr::Static([snap(sx + dx), snap(sy + dy)]);
            *end = Vec2OrExpr::Static([snap(ex + dx), snap(ey + dy)]);
        }
        Shape::Bezier { start, control1, control2, end, .. } => {
            // Get current points
            let sv = match &*start { Vec2OrExpr::Static(v) => *v, _ => return };
            let c1v = match &*control1 { Vec2OrExpr::Static(v) => *v, _ => return };
            let c2v = match &*control2 { Vec2OrExpr::Static(v) => *v, _ => return };
            let ev = match &*end { Vec2OrExpr::Static(v) => *v, _ => return };

            // Calculate center
            let cx = (sv[0] + c1v[0] + c2v[0] + ev[0]) / 4.0;
            let cy = (sv[1] + c1v[1] + c2v[1] + ev[1]) / 4.0;
            let dx = x - cx;
            let dy = y - cy;

            // Move all points
            *start = Vec2OrExpr::Static([snap(sv[0] + dx), snap(sv[1] + dy)]);
            *control1 = Vec2OrExpr::Static([snap(c1v[0] + dx), snap(c1v[1] + dy)]);
            *control2 = Vec2OrExpr::Static([snap(c2v[0] + dx), snap(c2v[1] + dy)]);
            *end = Vec2OrExpr::Static([snap(ev[0] + dx), snap(ev[1] + dy)]);
        }
    }
}

/// Mirror object on X-axis (left-right flip, in-place around object center).
fn mirror_object_x(obj: &mut MapObject) {
    match &mut obj.shape {
        Shape::Circle { .. } => {
            // Circle is symmetric, no change needed
        }
        Shape::Rect { rotation, .. } => {
            // Flip rotation sign
            if let NumberOrExpr::Number(r) = rotation {
                *rotation = NumberOrExpr::Number((-*r).round());
            }
        }
        Shape::Line { start, end } => {
            // Get center and flip x-offset of start/end
            let (sx, sy) = match start {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => return,
            };
            let (ex, ey) = match end {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => return,
            };
            let cx = (sx + ex) / 2.0;
            // Reflect around center x
            *start = Vec2OrExpr::Static([snap(2.0 * cx - sx), snap(sy)]);
            *end = Vec2OrExpr::Static([snap(2.0 * cx - ex), snap(ey)]);
        }
        Shape::Bezier { start, control1, control2, end, .. } => {
            // Get current points
            let sv = match &*start { Vec2OrExpr::Static(v) => *v, _ => return };
            let c1v = match &*control1 { Vec2OrExpr::Static(v) => *v, _ => return };
            let c2v = match &*control2 { Vec2OrExpr::Static(v) => *v, _ => return };
            let ev = match &*end { Vec2OrExpr::Static(v) => *v, _ => return };

            // Calculate center x
            let cx = (sv[0] + c1v[0] + c2v[0] + ev[0]) / 4.0;

            // Flip all points around center x
            *start = Vec2OrExpr::Static([snap(2.0 * cx - sv[0]), snap(sv[1])]);
            *control1 = Vec2OrExpr::Static([snap(2.0 * cx - c1v[0]), snap(c1v[1])]);
            *control2 = Vec2OrExpr::Static([snap(2.0 * cx - c2v[0]), snap(c2v[1])]);
            *end = Vec2OrExpr::Static([snap(2.0 * cx - ev[0]), snap(ev[1])]);
        }
    }
}

/// Mirror object on Y-axis (top-bottom flip, in-place around object center).
fn mirror_object_y(obj: &mut MapObject) {
    match &mut obj.shape {
        Shape::Circle { .. } => {
            // Circle is symmetric, no change needed
        }
        Shape::Rect { rotation, .. } => {
            // Flip rotation sign
            if let NumberOrExpr::Number(r) = rotation {
                *rotation = NumberOrExpr::Number((-*r).round());
            }
        }
        Shape::Line { start, end } => {
            // Get center and flip y-offset of start/end
            let (sx, sy) = match start {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => return,
            };
            let (ex, ey) = match end {
                Vec2OrExpr::Static(v) => (v[0], v[1]),
                _ => return,
            };
            let cy = (sy + ey) / 2.0;
            // Reflect around center y
            *start = Vec2OrExpr::Static([snap(sx), snap(2.0 * cy - sy)]);
            *end = Vec2OrExpr::Static([snap(ex), snap(2.0 * cy - ey)]);
        }
        Shape::Bezier { start, control1, control2, end, .. } => {
            // Get current points
            let sv = match &*start { Vec2OrExpr::Static(v) => *v, _ => return };
            let c1v = match &*control1 { Vec2OrExpr::Static(v) => *v, _ => return };
            let c2v = match &*control2 { Vec2OrExpr::Static(v) => *v, _ => return };
            let ev = match &*end { Vec2OrExpr::Static(v) => *v, _ => return };

            // Calculate center y
            let cy = (sv[1] + c1v[1] + c2v[1] + ev[1]) / 4.0;

            // Flip all points around center y
            *start = Vec2OrExpr::Static([snap(sv[0]), snap(2.0 * cy - sv[1])]);
            *control1 = Vec2OrExpr::Static([snap(c1v[0]), snap(2.0 * cy - c1v[1])]);
            *control2 = Vec2OrExpr::Static([snap(c2v[0]), snap(2.0 * cy - c2v[1])]);
            *end = Vec2OrExpr::Static([snap(ev[0]), snap(2.0 * cy - ev[1])]);
        }
    }
}
