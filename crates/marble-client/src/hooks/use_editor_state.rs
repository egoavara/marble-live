//! Editor state management hook.

use std::collections::HashMap;
use std::rc::Rc;

use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use marble_core::map::{
    Keyframe, KeyframeSequence, MapMeta, MapObject, ObjectProperties, ObjectRole, RouletteConfig,
    Shape,
};
use yew::prelude::*;

const STORAGE_KEY: &str = "marble-live-editor-state";

// ============================================================================
// Shape Cache Types
// ============================================================================

/// 각 shape 타입별 캐시된 속성 (위치 제외)
#[derive(Clone, PartialEq, Default, Debug)]
pub struct ShapeCache {
    pub line: Option<LineCache>,
    pub circle: Option<CircleCache>,
    pub rect: Option<RectCache>,
    pub bezier: Option<BezierCache>,
}

/// Line shape 캐시 (start/end에서 중심까지의 거리와 각도)
#[derive(Clone, PartialEq, Debug)]
pub struct LineCache {
    /// 중심에서 start/end까지의 거리 (half length)
    pub half_length: f32,
    /// Line의 각도 (radians)
    pub angle: f32,
}

/// Circle shape 캐시
#[derive(Clone, PartialEq, Debug)]
pub struct CircleCache {
    pub radius: f32,
}

/// Rect shape 캐시
#[derive(Clone, PartialEq, Debug)]
pub struct RectCache {
    pub size: [f32; 2],
    pub rotation: f32,
}

/// Bezier shape 캐시 (상대 위치 기반)
#[derive(Clone, PartialEq, Debug)]
pub struct BezierCache {
    /// control1의 중심 기준 상대 오프셋
    pub control1_offset: [f32; 2],
    /// control2의 중심 기준 상대 오프셋
    pub control2_offset: [f32; 2],
    /// 중심에서 start/end까지의 거리
    pub half_span: f32,
    /// Line의 각도 (radians)
    pub angle: f32,
    /// 세그먼트 수
    pub segments: u32,
}

// ============================================================================
// Shape Helper Functions
// ============================================================================

/// Shape에서 중심 좌표 추출
pub fn get_shape_center(shape: &Shape) -> [f32; 2] {
    match shape {
        Shape::Circle { center, .. } | Shape::Rect { center, .. } => {
            get_vec2_static_internal(center).unwrap_or([3.0, 5.0])
        }
        Shape::Line { start, end } => {
            let s = get_vec2_static_internal(start).unwrap_or([0.0, 0.0]);
            let e = get_vec2_static_internal(end).unwrap_or([0.0, 0.0]);
            [(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0]
        }
        Shape::Bezier { start, end, .. } => {
            let s = get_vec2_static_internal(start).unwrap_or([0.0, 0.0]);
            let e = get_vec2_static_internal(end).unwrap_or([0.0, 0.0]);
            [(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0]
        }
    }
}

/// Helper to extract static value from Vec2OrExpr (internal)
fn get_vec2_static_internal(v: &Vec2OrExpr) -> Option<[f32; 2]> {
    match v {
        Vec2OrExpr::Static(arr) => Some(*arr),
        Vec2OrExpr::Dynamic(_) => None,
    }
}

/// Helper to extract static value from NumberOrExpr (internal)
fn get_number_static_internal(n: &NumberOrExpr) -> Option<f32> {
    match n {
        NumberOrExpr::Number(v) => Some(*v),
        NumberOrExpr::Expr(_) => None,
    }
}

/// Shape에서 캐시 가능한 속성 추출하여 기존 캐시에 업데이트
pub fn update_shape_cache(cache: &mut ShapeCache, shape: &Shape) {
    match shape {
        Shape::Line { start, end } => {
            let s = get_vec2_static_internal(start).unwrap_or([0.0, 0.0]);
            let e = get_vec2_static_internal(end).unwrap_or([0.0, 0.0]);
            let dx = e[0] - s[0];
            let dy = e[1] - s[1];
            let half_length = (dx * dx + dy * dy).sqrt() / 2.0;
            let angle = dy.atan2(dx);
            cache.line = Some(LineCache { half_length, angle });
        }
        Shape::Circle { radius, .. } => {
            let r = get_number_static_internal(radius).unwrap_or(0.3);
            cache.circle = Some(CircleCache { radius: r });
        }
        Shape::Rect { size, rotation, .. } => {
            let s = get_vec2_static_internal(size).unwrap_or([1.0, 0.5]);
            let r = get_number_static_internal(rotation).unwrap_or(0.0);
            cache.rect = Some(RectCache { size: s, rotation: r });
        }
        Shape::Bezier {
            start,
            control1,
            control2,
            end,
            segments,
        } => {
            let s = get_vec2_static_internal(start).unwrap_or([0.0, 0.0]);
            let e = get_vec2_static_internal(end).unwrap_or([0.0, 0.0]);
            let c1 = get_vec2_static_internal(control1).unwrap_or([0.0, 0.0]);
            let c2 = get_vec2_static_internal(control2).unwrap_or([0.0, 0.0]);

            // 중심점 계산 (start와 end의 중점)
            let center = [(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0];

            // start-end 거리의 절반
            let dx = e[0] - s[0];
            let dy = e[1] - s[1];
            let half_span = (dx * dx + dy * dy).sqrt() / 2.0;
            let angle = dy.atan2(dx);

            // control point의 중심 기준 상대 오프셋
            let control1_offset = [c1[0] - center[0], c1[1] - center[1]];
            let control2_offset = [c2[0] - center[0], c2[1] - center[1]];

            cache.bezier = Some(BezierCache {
                control1_offset,
                control2_offset,
                half_span,
                angle,
                segments: *segments,
            });
        }
    }
}

/// 캐시된 속성을 사용하여 새 shape 생성
pub fn create_shape_from_cache(
    shape_type: &str,
    center: [f32; 2],
    cache: &ShapeCache,
) -> Option<Shape> {
    match shape_type {
        "line" => {
            let (half_length, angle) = cache
                .line
                .as_ref()
                .map(|c| (c.half_length, c.angle))
                .unwrap_or((0.5, 0.0)); // 기본값: 길이 1.0, 수평

            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let start = [
                center[0] - half_length * cos_a,
                center[1] - half_length * sin_a,
            ];
            let end = [
                center[0] + half_length * cos_a,
                center[1] + half_length * sin_a,
            ];

            Some(Shape::Line {
                start: Vec2OrExpr::Static(start),
                end: Vec2OrExpr::Static(end),
            })
        }
        "circle" => {
            let radius = cache
                .circle
                .as_ref()
                .map(|c| c.radius)
                .unwrap_or(0.3);

            Some(Shape::Circle {
                center: Vec2OrExpr::Static(center),
                radius: NumberOrExpr::Number(radius),
            })
        }
        "rect" => {
            let (size, rotation) = cache
                .rect
                .as_ref()
                .map(|c| (c.size, c.rotation))
                .unwrap_or(([1.0, 0.5], 0.0));

            Some(Shape::Rect {
                center: Vec2OrExpr::Static(center),
                size: Vec2OrExpr::Static(size),
                rotation: NumberOrExpr::Number(rotation),
            })
        }
        "bezier" => {
            let bezier_cache = cache.bezier.as_ref();
            let (control1_offset, control2_offset, half_span, angle, segments) = bezier_cache
                .map(|c| {
                    (
                        c.control1_offset,
                        c.control2_offset,
                        c.half_span,
                        c.angle,
                        c.segments,
                    )
                })
                .unwrap_or(([-0.25, -0.5], [0.25, 0.5], 0.5, 0.0, 16)); // 기본값

            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let start = [
                center[0] - half_span * cos_a,
                center[1] - half_span * sin_a,
            ];
            let end = [
                center[0] + half_span * cos_a,
                center[1] + half_span * sin_a,
            ];
            let control1 = [
                center[0] + control1_offset[0],
                center[1] + control1_offset[1],
            ];
            let control2 = [
                center[0] + control2_offset[0],
                center[1] + control2_offset[1],
            ];

            Some(Shape::Bezier {
                start: Vec2OrExpr::Static(start),
                control1: Vec2OrExpr::Static(control1),
                control2: Vec2OrExpr::Static(control2),
                end: Vec2OrExpr::Static(end),
                segments,
            })
        }
        _ => None,
    }
}

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
    /// 오브젝트별 shape 캐시 (key = object index)
    pub shape_cache: HashMap<usize, ShapeCache>,
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
            shape_cache: HashMap::new(),
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
    /// Sync objects from Bevy (Bevy -> Yew).
    SyncObjectsFromBevy(Vec<MapObject>),
    /// Sync selection from Bevy (Bevy -> Yew).
    SyncSelectionFromBevy(Option<usize>),
    /// Sync keyframes from Bevy (Bevy -> Yew).
    SyncKeyframesFromBevy(Vec<KeyframeSequence>),
    /// Shape 변경 전 현재 속성 캐시
    CacheShape { index: usize, shape: Shape },
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
                shape_cache: HashMap::new(),
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
                    shape_cache: self.shape_cache.clone(),
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
                // shape_cache 인덱스 재정렬: 삭제된 인덱스보다 큰 인덱스를 1씩 감소
                let mut shape_cache = HashMap::new();
                for (idx, cache) in self.shape_cache.iter() {
                    if *idx < index {
                        shape_cache.insert(*idx, cache.clone());
                    } else if *idx > index {
                        shape_cache.insert(*idx - 1, cache.clone());
                    }
                    // idx == index인 경우 삭제
                }
                Rc::new(Self {
                    config,
                    selected_object: selected,
                    is_dirty: true,
                    clipboard: self.clipboard.clone(),
                    selected_sequence: self.selected_sequence,
                    selected_keyframe: self.selected_keyframe,
                    shape_cache,
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
                    shape_cache: HashMap::new(),
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
            EditorAction::SyncObjectsFromBevy(objects) => {
                // Skip update if objects are identical (avoid unnecessary re-renders)
                if self.config.objects == objects {
                    return Rc::new((*self).clone());
                }
                // Sync objects from Bevy without marking as dirty
                // (changes came from Bevy, not user editing)
                let mut config = self.config.clone();
                config.objects = objects;
                Rc::new(Self {
                    config,
                    // Don't set is_dirty - this is syncing from Bevy, not user changes
                    ..(*self).clone()
                })
            }
            EditorAction::SyncSelectionFromBevy(selected) => {
                // Skip update if selection is identical (avoid unnecessary re-renders)
                if self.selected_object == selected {
                    return Rc::new((*self).clone());
                }
                Rc::new(Self {
                    selected_object: selected,
                    ..(*self).clone()
                })
            }
            EditorAction::SyncKeyframesFromBevy(keyframes) => {
                // Skip update if keyframes are identical (avoid unnecessary re-renders)
                if self.config.keyframes == keyframes {
                    return Rc::new((*self).clone());
                }
                // Sync keyframes from Bevy without marking as dirty
                // (changes came from Bevy gizmo drag, not UI editing)
                let mut config = self.config.clone();
                config.keyframes = keyframes;
                Rc::new(Self {
                    config,
                    // Don't set is_dirty - this is syncing from Bevy
                    ..(*self).clone()
                })
            }
            EditorAction::CacheShape { index, shape } => {
                let mut shape_cache = self.shape_cache.clone();
                let cache = shape_cache.entry(index).or_insert_with(ShapeCache::default);
                update_shape_cache(cache, &shape);
                Rc::new(Self {
                    shape_cache,
                    ..(*self).clone()
                })
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
    /// 오브젝트별 shape 캐시
    pub shape_cache: HashMap<usize, ShapeCache>,
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
    /// Shape 변경 전 현재 속성 캐시
    pub on_cache_shape: Callback<(usize, Shape)>,
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
                        shape_cache: HashMap::new(),
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

    // Poll Bevy for object updates (Bevy -> Yew sync)
    let bevy_objects = crate::hooks::use_bevy_editor_objects();
    {
        let state = state.clone();
        use_effect_with(bevy_objects.clone(), move |bevy_objs| {
            // Dispatch to reducer - duplicate check is done inside reducer
            // to avoid closure capturing stale state
            state.dispatch(EditorAction::SyncObjectsFromBevy(bevy_objs.clone()));
        });
    }

    // Poll Bevy for selection updates (Bevy -> Yew sync)
    let bevy_editor_state = crate::hooks::use_bevy_editor_state();
    {
        let state = state.clone();
        use_effect_with(bevy_editor_state.selected_object, move |selected| {
            // Dispatch to reducer - duplicate check is done inside reducer
            state.dispatch(EditorAction::SyncSelectionFromBevy(*selected));
        });
    }

    // Poll Bevy for keyframes updates (Bevy -> Yew sync)
    let bevy_keyframes = crate::hooks::use_bevy_editor_keyframes();
    {
        let state = state.clone();
        use_effect_with(bevy_keyframes.clone(), move |bevy_kfs| {
            // Dispatch to reducer - duplicate check is done inside reducer
            state.dispatch(EditorAction::SyncKeyframesFromBevy(bevy_kfs.clone()));
        });
    }

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            // NewMap reducer와 동일한 기본 설정 생성
            let config = RouletteConfig {
                meta: MapMeta {
                    name: "New Map".to_string(),
                    gamerule: vec![],
                    live_ranking: Default::default(),
                },
                objects: vec![],
                keyframes: vec![],
            };
            // Bevy에 load_map 명령 전송
            if let Ok(config_json) = serde_json::to_string(&config) {
                let cmd = format!(
                    r#"{{"type":"load_map","config":{}}}"#,
                    config_json
                );
                if let Err(e) = crate::hooks::send_command(&cmd) {
                    tracing::warn!("Failed to sync new map to Bevy: {:?}", e);
                }
            }
            state.dispatch(EditorAction::NewMap);
        })
    };

    let on_load = {
        let state = state.clone();
        Callback::from(move |config: RouletteConfig| {
            // Bevy에 load_map 명령 전송
            if let Ok(config_json) = serde_json::to_string(&config) {
                let cmd = format!(
                    r#"{{"type":"load_map","config":{}}}"#,
                    config_json
                );
                if let Err(e) = crate::hooks::send_command(&cmd) {
                    tracing::warn!("Failed to sync load_map to Bevy: {:?}", e);
                }
            }
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
            // Sync selection to Bevy
            let cmd = if let Some(idx) = index {
                format!(r#"{{"type":"select_object","index":{}}}"#, idx)
            } else {
                r#"{"type":"select_object","index":null}"#.to_string()
            };
            if let Err(e) = crate::hooks::send_command(&cmd) {
                tracing::warn!("Failed to sync selection to Bevy: {:?}", e);
            }
        })
    };

    let on_add = {
        let state = state.clone();
        Callback::from(move |object: MapObject| {
            // Sync to Bevy
            if let Ok(object_json) = serde_json::to_string(&object) {
                let cmd = format!(
                    r#"{{"type":"add_object","object":{}}}"#,
                    object_json
                );
                if let Err(e) = crate::hooks::send_command(&cmd) {
                    tracing::warn!("Failed to sync add to Bevy: {:?}", e);
                }
            }
            state.dispatch(EditorAction::AddObject(object));
        })
    };

    let on_delete = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            // Sync to Bevy
            let cmd = format!(
                r#"{{"type":"delete_object","index":{}}}"#,
                index
            );
            if let Err(e) = crate::hooks::send_command(&cmd) {
                tracing::warn!("Failed to sync delete to Bevy: {:?}", e);
            }
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
            // Sync to Bevy
            if let Ok(object_json) = serde_json::to_string(&object) {
                let cmd = format!(
                    r#"{{"type":"update_object","index":{},"object":{}}}"#,
                    index, object_json
                );
                if let Err(e) = crate::hooks::send_command(&cmd) {
                    tracing::warn!("Failed to sync object update to Bevy: {:?}", e);
                }
            }
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
            // Sync to Bevy: create the pasted object and send add_object command
            if let Some(obj) = &state.clipboard {
                let mut new_obj = obj.clone();
                move_object_center(&mut new_obj, x, y);
                if let Ok(object_json) = serde_json::to_string(&new_obj) {
                    let cmd = format!(
                        r#"{{"type":"add_object","object":{}}}"#,
                        object_json
                    );
                    if let Err(e) = crate::hooks::send_command(&cmd) {
                        tracing::warn!("Failed to sync paste to Bevy: {:?}", e);
                    }
                }
            }
            state.dispatch(EditorAction::PasteObject { x, y });
        })
    };

    let on_mirror_x = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            // Sync to Bevy: apply mirror and send update_object command
            if let Some(obj) = state.config.objects.get(index) {
                let mut mirrored = obj.clone();
                mirror_object_x(&mut mirrored);
                if let Ok(object_json) = serde_json::to_string(&mirrored) {
                    let cmd = format!(
                        r#"{{"type":"update_object","index":{},"object":{}}}"#,
                        index, object_json
                    );
                    if let Err(e) = crate::hooks::send_command(&cmd) {
                        tracing::warn!("Failed to sync mirror_x to Bevy: {:?}", e);
                    }
                }
            }
            state.dispatch(EditorAction::MirrorObjectX(index));
        })
    };

    let on_mirror_y = {
        let state = state.clone();
        Callback::from(move |index: usize| {
            // Sync to Bevy: apply mirror and send update_object command
            if let Some(obj) = state.config.objects.get(index) {
                let mut mirrored = obj.clone();
                mirror_object_y(&mut mirrored);
                if let Ok(object_json) = serde_json::to_string(&mirrored) {
                    let cmd = format!(
                        r#"{{"type":"update_object","index":{},"object":{}}}"#,
                        index, object_json
                    );
                    if let Err(e) = crate::hooks::send_command(&cmd) {
                        tracing::warn!("Failed to sync mirror_y to Bevy: {:?}", e);
                    }
                }
            }
            state.dispatch(EditorAction::MirrorObjectY(index));
        })
    };

    let on_cache_shape = {
        let state = state.clone();
        Callback::from(move |(index, shape): (usize, Shape)| {
            state.dispatch(EditorAction::CacheShape { index, shape });
        })
    };

    // Sequence callbacks
    let on_select_sequence = {
        let state = state.clone();
        Callback::from(move |index: Option<usize>| {
            state.dispatch(EditorAction::SelectSequence(index));
            // Sync selection to Bevy
            let cmd = if let Some(idx) = index {
                format!(r#"{{"type":"select_sequence","index":{}}}"#, idx)
            } else {
                r#"{"type":"select_sequence","index":null}"#.to_string()
            };
            if let Err(e) = crate::hooks::send_command(&cmd) {
                tracing::warn!("Failed to sync sequence selection to Bevy: {:?}", e);
            }
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
            // Sync selection to Bevy
            let cmd = if let Some(idx) = index {
                format!(r#"{{"type":"select_keyframe","index":{}}}"#, idx)
            } else {
                r#"{"type":"select_keyframe","index":null}"#.to_string()
            };
            if let Err(e) = crate::hooks::send_command(&cmd) {
                tracing::warn!("Failed to sync keyframe selection to Bevy: {:?}", e);
            }
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
                // Sync to Bevy
                if let Ok(keyframe_json) = serde_json::to_string(&keyframe) {
                    let cmd = format!(
                        r#"{{"type":"update_keyframe","sequence_index":{},"keyframe_index":{},"keyframe":{}}}"#,
                        sequence_index, keyframe_index, keyframe_json
                    );
                    if let Err(e) = crate::hooks::send_command(&cmd) {
                        tracing::warn!("Failed to sync keyframe update to Bevy: {:?}", e);
                    }
                }
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
        shape_cache: state.shape_cache.clone(),
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
        on_cache_shape,
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

/// Creates a default guideline object (vertical line).
pub fn create_default_guideline() -> MapObject {
    use marble_core::map::GuidelineProperties;

    MapObject {
        id: None,
        role: ObjectRole::Guideline,
        shape: Shape::Line {
            start: Vec2OrExpr::Static([3.0, 0.0]),
            end: Vec2OrExpr::Static([3.0, 10.0]),
        },
        properties: ObjectProperties {
            guideline: Some(GuidelineProperties::default()),
            ..Default::default()
        },
    }
}

/// Round to 0.01m (1 pixel) grid.
fn snap(v: f32) -> f32 {
    (v / 0.01).round() * 0.01
}

/// Move an object's center to a new position (snapped to grid).
pub fn move_object_center(obj: &mut MapObject, x: f32, y: f32) {
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
pub fn mirror_object_x(obj: &mut MapObject) {
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
pub fn mirror_object_y(obj: &mut MapObject) {
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
