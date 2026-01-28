//! Editor state management hook.

use std::rc::Rc;

use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use marble_core::map::{
    Keyframe, KeyframeSequence, MapMeta, MapObject, ObjectRole, RouletteConfig, Shape,
};
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
    /// Currently selected keyframe sequence index.
    pub selected_sequence: Option<usize>,
    /// Currently selected keyframe index within the selected sequence.
    pub selected_keyframe: Option<usize>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            config: RouletteConfig::default_classic(),
            selected_object: None,
            is_dirty: false,
            clipboard: None,
            selected_sequence: None,
            selected_keyframe: None,
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

    // Sequence actions
    /// Select a keyframe sequence by index.
    SelectSequence(Option<usize>),
    /// Add a new keyframe sequence.
    AddSequence(KeyframeSequence),
    /// Update a keyframe sequence at the given index.
    UpdateSequence {
        index: usize,
        sequence: KeyframeSequence,
    },
    /// Delete a keyframe sequence by index.
    DeleteSequence(usize),

    // Keyframe actions (within selected sequence)
    /// Select a keyframe within the selected sequence.
    SelectKeyframe(Option<usize>),
    /// Add a keyframe to a sequence.
    AddKeyframe {
        sequence_index: usize,
        keyframe: Keyframe,
    },
    /// Update a keyframe within a sequence.
    UpdateKeyframe {
        sequence_index: usize,
        keyframe_index: usize,
        keyframe: Keyframe,
    },
    /// Delete a keyframe from a sequence.
    DeleteKeyframe {
        sequence_index: usize,
        keyframe_index: usize,
    },
    /// Move a keyframe within a sequence.
    MoveKeyframe {
        sequence_index: usize,
        from: usize,
        to: usize,
    },
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
                selected_sequence: None,
                selected_keyframe: None,
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
                    selected_sequence: self.selected_sequence,
                    selected_keyframe: self.selected_keyframe,
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
                        if sel > 0 {
                            Some(sel - 1)
                        } else if config.objects.is_empty() {
                            None
                        } else {
                            Some(0)
                        }
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
                    selected_sequence: self.selected_sequence,
                    selected_keyframe: self.selected_keyframe,
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
                    selected_sequence: None,
                    selected_keyframe: None,
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

            // Sequence actions
            EditorAction::SelectSequence(index) => Rc::new(Self {
                selected_sequence: index,
                selected_keyframe: None, // Clear keyframe selection when changing sequence
                ..(*self).clone()
            }),
            EditorAction::AddSequence(sequence) => {
                let mut config = self.config.clone();
                config.keyframes.push(sequence);
                let new_index = config.keyframes.len() - 1;
                Rc::new(Self {
                    config,
                    selected_sequence: Some(new_index),
                    selected_keyframe: None,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }
            EditorAction::UpdateSequence { index, sequence } => {
                let mut config = self.config.clone();
                if index < config.keyframes.len() {
                    config.keyframes[index] = sequence;
                }
                Rc::new(Self {
                    config,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }
            EditorAction::DeleteSequence(index) => {
                let mut config = self.config.clone();
                if index < config.keyframes.len() {
                    config.keyframes.remove(index);
                }
                let selected = if config.keyframes.is_empty() {
                    None
                } else if let Some(sel) = self.selected_sequence {
                    if sel >= config.keyframes.len() {
                        Some(config.keyframes.len().saturating_sub(1))
                    } else if sel > index {
                        Some(sel - 1)
                    } else if sel == index {
                        if sel > 0 {
                            Some(sel - 1)
                        } else if config.keyframes.is_empty() {
                            None
                        } else {
                            Some(0)
                        }
                    } else {
                        Some(sel)
                    }
                } else {
                    None
                };
                Rc::new(Self {
                    config,
                    selected_sequence: selected,
                    selected_keyframe: None,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }

            // Keyframe actions
            EditorAction::SelectKeyframe(index) => Rc::new(Self {
                selected_keyframe: index,
                ..(*self).clone()
            }),
            EditorAction::AddKeyframe {
                sequence_index,
                keyframe,
            } => {
                let mut config = self.config.clone();
                if sequence_index < config.keyframes.len() {
                    config.keyframes[sequence_index].keyframes.push(keyframe);
                    let new_index = config.keyframes[sequence_index].keyframes.len() - 1;
                    Rc::new(Self {
                        config,
                        selected_keyframe: Some(new_index),
                        is_dirty: true,
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
            EditorAction::UpdateKeyframe {
                sequence_index,
                keyframe_index,
                keyframe,
            } => {
                let mut config = self.config.clone();
                if sequence_index < config.keyframes.len() {
                    let seq = &mut config.keyframes[sequence_index];
                    if keyframe_index < seq.keyframes.len() {
                        seq.keyframes[keyframe_index] = keyframe;
                    }
                }
                Rc::new(Self {
                    config,
                    is_dirty: true,
                    ..(*self).clone()
                })
            }
            EditorAction::DeleteKeyframe {
                sequence_index,
                keyframe_index,
            } => {
                let mut config = self.config.clone();
                if sequence_index < config.keyframes.len() {
                    let seq = &mut config.keyframes[sequence_index];
                    if keyframe_index < seq.keyframes.len() {
                        seq.keyframes.remove(keyframe_index);
                    }
                    let selected = if seq.keyframes.is_empty() {
                        None
                    } else if let Some(sel) = self.selected_keyframe {
                        if sel >= seq.keyframes.len() {
                            Some(seq.keyframes.len().saturating_sub(1))
                        } else if sel > keyframe_index {
                            Some(sel - 1)
                        } else if sel == keyframe_index {
                            if sel > 0 {
                                Some(sel - 1)
                            } else if seq.keyframes.is_empty() {
                                None
                            } else {
                                Some(0)
                            }
                        } else {
                            Some(sel)
                        }
                    } else {
                        None
                    };
                    Rc::new(Self {
                        config,
                        selected_keyframe: selected,
                        is_dirty: true,
                        ..(*self).clone()
                    })
                } else {
                    Rc::new((*self).clone())
                }
            }
            EditorAction::MoveKeyframe {
                sequence_index,
                from,
                to,
            } => {
                let mut config = self.config.clone();
                if sequence_index < config.keyframes.len() {
                    let seq = &mut config.keyframes[sequence_index];
                    if from < seq.keyframes.len() && to < seq.keyframes.len() && from != to {
                        let keyframe = seq.keyframes.remove(from);
                        seq.keyframes.insert(to, keyframe);
                        Rc::new(Self {
                            config,
                            selected_keyframe: Some(to),
                            is_dirty: true,
                            ..(*self).clone()
                        })
                    } else {
                        Rc::new((*self).clone())
                    }
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
    pub selected_sequence: Option<usize>,
    pub selected_keyframe: Option<usize>,
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
    // Sequence callbacks
    pub on_select_sequence: Callback<Option<usize>>,
    pub on_add_sequence: Callback<KeyframeSequence>,
    pub on_update_sequence: Callback<(usize, KeyframeSequence)>,
    pub on_delete_sequence: Callback<usize>,
    // Keyframe callbacks
    pub on_select_keyframe: Callback<Option<usize>>,
    pub on_add_keyframe: Callback<(usize, Keyframe)>,
    pub on_update_keyframe: Callback<(usize, usize, Keyframe)>,
    pub on_delete_keyframe: Callback<(usize, usize)>,
    pub on_move_keyframe: Callback<(usize, usize, usize)>,
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
                        selected_sequence: None,
                        selected_keyframe: None,
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
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
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

    // Sequence callbacks
    let on_select_sequence = {
        let state = state.clone();
        Callback::from(move |index: Option<usize>| {
            state.dispatch(EditorAction::SelectSequence(index));
        })
    };

    let on_add_sequence = {
        let state = state.clone();
        Callback::from(move |sequence: KeyframeSequence| {
            state.dispatch(EditorAction::AddSequence(sequence));
        })
    };

    let on_update_sequence = {
        let state = state.clone();
        Callback::from(move |(index, sequence): (usize, KeyframeSequence)| {
            state.dispatch(EditorAction::UpdateSequence { index, sequence });
        })
    };

    let on_delete_sequence = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            state.dispatch(EditorAction::DeleteSequence(index));
        })
    };

    // Keyframe callbacks
    let on_select_keyframe = {
        let state = state.clone();
        Callback::from(move |index: Option<usize>| {
            state.dispatch(EditorAction::SelectKeyframe(index));
        })
    };

    let on_add_keyframe = {
        let state = state.clone();
        Callback::from(move |(sequence_index, keyframe): (usize, Keyframe)| {
            state.dispatch(EditorAction::AddKeyframe {
                sequence_index,
                keyframe,
            });
        })
    };

    let on_update_keyframe = {
        let state = state.clone();
        Callback::from(
            move |(sequence_index, keyframe_index, keyframe): (usize, usize, Keyframe)| {
                state.dispatch(EditorAction::UpdateKeyframe {
                    sequence_index,
                    keyframe_index,
                    keyframe,
                });
            },
        )
    };

    let on_delete_keyframe = {
        let state = state.clone();
        Callback::from(move |(sequence_index, keyframe_index): (usize, usize)| {
            state.dispatch(EditorAction::DeleteKeyframe {
                sequence_index,
                keyframe_index,
            });
        })
    };

    let on_move_keyframe = {
        let state = state.clone();
        Callback::from(move |(sequence_index, from, to): (usize, usize, usize)| {
            state.dispatch(EditorAction::MoveKeyframe {
                sequence_index,
                from,
                to,
            });
        })
    };

    EditorStateHandle {
        config: state.config.clone(),
        selected_object: state.selected_object,
        is_dirty: state.is_dirty,
        clipboard: state.clipboard.clone(),
        selected_sequence: state.selected_sequence,
        selected_keyframe: state.selected_keyframe,
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
        on_select_sequence,
        on_add_sequence,
        on_update_sequence,
        on_delete_sequence,
        on_select_keyframe,
        on_add_keyframe,
        on_update_keyframe,
        on_delete_keyframe,
        on_move_keyframe,
    }
}

/// Creates a default obstacle object.
pub fn create_default_obstacle() -> MapObject {
    MapObject {
        id: None,
        role: ObjectRole::Obstacle,
        shape: Shape::Circle {
            center: Vec2OrExpr::Static([3.0, 5.0]),
            radius: NumberOrExpr::Number(0.3),
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
            center: Vec2OrExpr::Static([3.0, 1.0]),
            size: Vec2OrExpr::Static([2.0, 0.5]),
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
            center: Vec2OrExpr::Static([3.0, 9.5]),
            radius: NumberOrExpr::Number(0.4),
        },
        properties: Default::default(),
    }
}

/// Round to 0.01m (1 pixel) grid.
fn snap(v: f32) -> f32 {
    (v / 0.01).round() * 0.01
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
        Shape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            // Get current points
            let sv = match &*start {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c1v = match &*control1 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c2v = match &*control2 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let ev = match &*end {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };

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
        Shape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            // Get current points
            let sv = match &*start {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c1v = match &*control1 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c2v = match &*control2 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let ev = match &*end {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };

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
        Shape::Bezier {
            start,
            control1,
            control2,
            end,
            ..
        } => {
            // Get current points
            let sv = match &*start {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c1v = match &*control1 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let c2v = match &*control2 {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };
            let ev = match &*end {
                Vec2OrExpr::Static(v) => *v,
                _ => return,
            };

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
