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
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            config: RouletteConfig::default_classic(),
            selected_object: None,
            is_dirty: false,
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
}

impl Reducible for EditorState {
    type Action = EditorAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            EditorAction::LoadConfig(config) => Rc::new(Self {
                config,
                selected_object: None,
                is_dirty: false,
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
                })
            }
            EditorAction::MarkSaved => Rc::new(Self {
                is_dirty: false,
                ..(*self).clone()
            }),
        }
    }
}

/// Editor state handle returned by `use_editor_state`.
#[derive(Clone)]
pub struct EditorStateHandle {
    pub config: RouletteConfig,
    pub selected_object: Option<usize>,
    pub is_dirty: bool,
    pub on_new: Callback<()>,
    pub on_load: Callback<RouletteConfig>,
    pub on_save: Callback<()>,
    pub on_select: Callback<Option<usize>>,
    pub on_add: Callback<MapObject>,
    pub on_delete: Callback<usize>,
    pub on_update_meta: Callback<MapMeta>,
    pub on_update_object: Callback<(usize, MapObject)>,
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

    EditorStateHandle {
        config: state.config.clone(),
        selected_object: state.selected_object,
        is_dirty: state.is_dirty,
        on_new,
        on_load,
        on_save,
        on_select,
        on_add,
        on_delete,
        on_update_meta,
        on_update_object,
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
