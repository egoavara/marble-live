//! Property panel component for editing object and meta properties.

use std::collections::HashMap;

use marble_core::dsl::{BoolOrExpr, NumberOrExpr, Vec2OrExpr};
use marble_core::map::{
    BumperProperties, EasingType, GuidelineProperties, Keyframe, KeyframeSequence, MapMeta,
    MapObject, ObjectProperties, ObjectRole, PivotMode, RollDirection, RouletteConfig, Shape, SpawnProperties,
    TriggerProperties, VectorFieldFalloff, VectorFieldProperties,
};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_icons::{Icon, IconData};

use crate::hooks::{create_shape_from_cache, get_shape_center, ShapeCache};

/// Helper to extract static value from NumberOrExpr
fn get_number_static(n: &NumberOrExpr) -> Option<f32> {
    match n {
        NumberOrExpr::Number(v) => Some(*v),
        NumberOrExpr::Expr(_) => None,
    }
}

/// Helper to extract static value from Vec2OrExpr
fn get_vec2_static(v: &Vec2OrExpr) -> Option<[f32; 2]> {
    match v {
        Vec2OrExpr::Static(arr) => Some(*arr),
        Vec2OrExpr::Dynamic(_) => None,
    }
}

/// Helper to extract static value from BoolOrExpr
fn get_bool_static(b: &BoolOrExpr) -> Option<bool> {
    match b {
        BoolOrExpr::Bool(v) => Some(*v),
        BoolOrExpr::Expr(_) => None,
    }
}

/// Helper to extract expression string from BoolOrExpr
fn get_bool_expr(b: &BoolOrExpr) -> Option<&str> {
    match b {
        BoolOrExpr::Bool(_) => None,
        BoolOrExpr::Expr(s) => Some(s.as_str()),
    }
}

/// Props for the PropertyPanel component.
#[derive(Properties, PartialEq)]
pub struct PropertyPanelProps {
    pub config: RouletteConfig,
    pub selected_index: Option<usize>,
    pub on_update_meta: Callback<MapMeta>,
    pub on_update_object: Callback<(usize, MapObject)>,
    /// Currently selected keyframe sequence
    #[prop_or_default]
    pub sequence: Option<KeyframeSequence>,
    /// Currently selected keyframe index
    #[prop_or_default]
    pub selected_keyframe: Option<usize>,
    /// Callback when a keyframe is updated
    #[prop_or_default]
    pub on_update_keyframe: Callback<(usize, Keyframe)>,
    /// 오브젝트별 shape 캐시
    #[prop_or_default]
    pub shape_cache: HashMap<usize, ShapeCache>,
    /// Shape 변경 전 현재 속성 캐시 콜백
    #[prop_or_default]
    pub on_cache_shape: Callback<(usize, Shape)>,
}

/// Determines the current editing context based on selection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyContext {
    /// Keyframe is selected (highest priority)
    Keyframe,
    /// Object is selected
    Object,
    /// Nothing selected - show map metadata
    Map,
}

impl PropertyContext {
    /// Get the context indicator icon and label.
    fn indicator(&self) -> (&'static str, &'static str) {
        match self {
            PropertyContext::Keyframe => ("\u{1F511}", "Keyframe"), // 🔑
            PropertyContext::Object => ("\u{1F4E6}", "Object"),     // 📦
            PropertyContext::Map => ("\u{1F5FA}", "Map"),           // 🗺️
        }
    }
}

/// Property panel component with context-based automatic switching.
#[function_component(PropertyPanel)]
pub fn property_panel(props: &PropertyPanelProps) -> Html {
    // Determine context based on selection state
    // Priority: Keyframe > Object > Map
    let context = if props.selected_keyframe.is_some() && props.sequence.is_some() {
        PropertyContext::Keyframe
    } else if props.selected_index.is_some() {
        PropertyContext::Object
    } else {
        PropertyContext::Map
    };

    let (icon, label) = context.indicator();

    html! {
        <div class="property-panel property-panel-contextual">
            <div class="property-panel-header">
                <div class="context-indicator">
                    <span class="context-icon">{icon}</span>
                    <span class="context-label">{label}</span>
                </div>
            </div>
            <div class="property-content">
                {match context {
                    PropertyContext::Keyframe => html! {
                        <KeyframePropertiesPanel
                            sequence={props.sequence.clone()}
                            selected_keyframe={props.selected_keyframe}
                            on_update_keyframe={props.on_update_keyframe.clone()}
                        />
                    },
                    PropertyContext::Object => html! {
                        <ObjectPropertiesPanel
                            config={props.config.clone()}
                            selected_index={props.selected_index}
                            on_update={props.on_update_object.clone()}
                            shape_cache={props.shape_cache.clone()}
                            on_cache_shape={props.on_cache_shape.clone()}
                        />
                    },
                    PropertyContext::Map => html! {
                        <MetaPropertiesPanel
                            meta={props.config.meta.clone()}
                            on_update={props.on_update_meta.clone()}
                        />
                    },
                }}
            </div>
        </div>
    }
}

/// Props for ObjectPropertiesPanel.
#[derive(Properties, PartialEq)]
struct ObjectPropertiesPanelProps {
    config: RouletteConfig,
    selected_index: Option<usize>,
    on_update: Callback<(usize, MapObject)>,
    /// 오브젝트별 shape 캐시
    #[prop_or_default]
    shape_cache: HashMap<usize, ShapeCache>,
    /// Shape 변경 전 현재 속성 캐시 콜백
    #[prop_or_default]
    on_cache_shape: Callback<(usize, Shape)>,
}

/// Panel for editing selected object properties.
#[function_component(ObjectPropertiesPanel)]
fn object_properties_panel(props: &ObjectPropertiesPanelProps) -> Html {
    let Some(index) = props.selected_index else {
        return html! {
            <div class="property-empty">
                {"Select an object to edit its properties"}
            </div>
        };
    };

    let Some(object) = props.config.objects.get(index) else {
        return html! {
            <div class="property-empty">
                {"Object not found"}
            </div>
        };
    };

    let object = object.clone();
    let on_update = props.on_update.clone();

    // ID input
    let on_id_input = {
        let object = object.clone();
        let on_update = on_update.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            let mut new_obj = object.clone();
            new_obj.id = if value.is_empty() { None } else { Some(value) };
            on_update.emit((index, new_obj));
        })
    };

    // Role select
    let on_role_change = {
        let object = object.clone();
        let on_update = on_update.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let role = match input.value().as_str() {
                "spawner" => ObjectRole::Spawner,
                "trigger" => ObjectRole::Trigger,
                _ => ObjectRole::Obstacle,
            };
            let mut new_obj = object.clone();
            new_obj.role = role;
            on_update.emit((index, new_obj));
        })
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">{"Basic"}</div>
                <div class="property-field">
                    <label>{"ID"}</label>
                    <input
                        type="text"
                        value={object.id.clone().unwrap_or_default()}
                        oninput={on_id_input}
                        placeholder="(optional)"
                    />
                </div>
                <div class="property-field">
                    <label>{"Role"}</label>
                    <select onchange={on_role_change}>
                        <option value="obstacle" selected={object.role == ObjectRole::Obstacle}>{"Obstacle"}</option>
                        <option value="spawner" selected={object.role == ObjectRole::Spawner}>{"Spawner"}</option>
                        <option value="trigger" selected={object.role == ObjectRole::Trigger}>{"Trigger"}</option>
                    </select>
                </div>
            </div>

            <ShapeEditor
                shape={object.shape.clone()}
                index={index}
                on_update={on_update.clone()}
                object={object.clone()}
                shape_cache={props.shape_cache.get(&index).cloned()}
                on_cache_shape={props.on_cache_shape.clone()}
            />

            <PropertiesEditor
                properties={object.properties.clone()}
                role={object.role.clone()}
                index={index}
                on_update={on_update.clone()}
                object={object.clone()}
            />
        </div>
    }
}

/// Props for ShapeEditor.
#[derive(Properties, PartialEq)]
struct ShapeEditorProps {
    shape: Shape,
    index: usize,
    on_update: Callback<(usize, MapObject)>,
    object: MapObject,
    /// 이 오브젝트의 shape 캐시
    #[prop_or_default]
    shape_cache: Option<ShapeCache>,
    /// Shape 변경 전 현재 속성 캐시 콜백
    #[prop_or_default]
    on_cache_shape: Callback<(usize, Shape)>,
}

/// Shape editor component.
#[function_component(ShapeEditor)]
fn shape_editor(props: &ShapeEditorProps) -> Html {
    let index = props.index;
    let on_update = props.on_update.clone();
    let object = props.object.clone();
    let shape_cache = props.shape_cache.clone();
    let on_cache_shape = props.on_cache_shape.clone();

    let on_shape_type_change = {
        let on_update = on_update.clone();
        let object = object.clone();
        let shape_cache = shape_cache.clone();
        let on_cache_shape = on_cache_shape.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let target_type = input.value();

            // 1. 현재 shape 캐시
            on_cache_shape.emit((index, object.shape.clone()));

            // 2. 현재 shape에서 중심 추출
            let center = get_shape_center(&object.shape);

            // 3. 새 shape 생성 (캐시된 속성 사용 또는 기본값)
            let cache = shape_cache.clone().unwrap_or_default();
            let new_shape = match create_shape_from_cache(&target_type, center, &cache) {
                Some(shape) => shape,
                None => return,
            };

            let mut new_obj = object.clone();
            new_obj.shape = new_shape;
            on_update.emit((index, new_obj));
        })
    };

    let shape_type = match &props.shape {
        Shape::Line { .. } => "line",
        Shape::Circle { .. } => "circle",
        Shape::Rect { .. } => "rect",
        Shape::Bezier { .. } => "bezier",
    };

    html! {
        <div class="property-section">
            <div class="property-section-title">{"Shape"}</div>
            <div class="property-field">
                <label>{"Type"}</label>
                <select onchange={on_shape_type_change}>
                    <option value="circle" selected={shape_type == "circle"}>{"Circle"}</option>
                    <option value="rect" selected={shape_type == "rect"}>{"Rectangle"}</option>
                    <option value="line" selected={shape_type == "line"}>{"Line"}</option>
                    <option value="bezier" selected={shape_type == "bezier"}>{"Bezier"}</option>
                </select>
            </div>
            {match &props.shape {
                Shape::Circle { center, radius } => {
                    let center_val = get_vec2_static(center).unwrap_or([0.0, 0.0]);
                    let radius_val = get_number_static(radius).unwrap_or(30.0);
                    html! {
                        <>
                            <Vec2Field
                                label="Center"
                                value={center_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let radius = radius.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Circle {
                                            center: Vec2OrExpr::Static(v),
                                            radius: radius.clone(),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <NumberField
                                label="Radius"
                                value={radius_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let center = center.clone();
                                    Callback::from(move |v: f32| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Circle {
                                            center: center.clone(),
                                            radius: NumberOrExpr::Number(v),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                        </>
                    }
                }
                Shape::Rect { center, size, rotation } => {
                    let center_val = get_vec2_static(center).unwrap_or([0.0, 0.0]);
                    let size_val = get_vec2_static(size).unwrap_or([1.0, 0.5]);
                    let rotation_val = get_number_static(rotation).unwrap_or(0.0);
                    html! {
                        <>
                            <Vec2Field
                                label="Center"
                                value={center_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let size = size.clone();
                                    let rotation = rotation.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Rect {
                                            center: Vec2OrExpr::Static(v),
                                            size: size.clone(),
                                            rotation: rotation.clone(),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <Vec2Field
                                label="Size"
                                value={size_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let center = center.clone();
                                    let rotation = rotation.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Rect {
                                            center: center.clone(),
                                            size: Vec2OrExpr::Static(v),
                                            rotation: rotation.clone(),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <NumberField
                                label="Rotation"
                                value={rotation_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let center = center.clone();
                                    let size = size.clone();
                                    Callback::from(move |v: f32| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Rect {
                                            center: center.clone(),
                                            size: size.clone(),
                                            rotation: NumberOrExpr::Number(v),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                        </>
                    }
                }
                Shape::Line { start, end } => {
                    let start_val = get_vec2_static(start).unwrap_or([0.0, 0.0]);
                    let end_val = get_vec2_static(end).unwrap_or([1.0, 0.0]);
                    html! {
                        <>
                            <Vec2Field
                                label="Start"
                                value={start_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let end = end.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Line {
                                            start: Vec2OrExpr::Static(v),
                                            end: end.clone(),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <Vec2Field
                                label="End"
                                value={end_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let start = start.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Line {
                                            start: start.clone(),
                                            end: Vec2OrExpr::Static(v),
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                        </>
                    }
                }
                Shape::Bezier { start, control1, control2, end, segments } => {
                    let start_val = get_vec2_static(start).unwrap_or([0.0, 0.0]);
                    let ctrl1_val = get_vec2_static(control1).unwrap_or([0.0, 0.0]);
                    let ctrl2_val = get_vec2_static(control2).unwrap_or([0.0, 0.0]);
                    let end_val = get_vec2_static(end).unwrap_or([0.0, 0.0]);
                    let segments_val = *segments;
                    html! {
                        <>
                            <Vec2Field
                                label="Start"
                                value={start_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let control1 = control1.clone();
                                    let control2 = control2.clone();
                                    let end = end.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Bezier {
                                            start: Vec2OrExpr::Static(v),
                                            control1: control1.clone(),
                                            control2: control2.clone(),
                                            end: end.clone(),
                                            segments: segments_val,
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <Vec2Field
                                label="Control 1"
                                value={ctrl1_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let start = start.clone();
                                    let control2 = control2.clone();
                                    let end = end.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Bezier {
                                            start: start.clone(),
                                            control1: Vec2OrExpr::Static(v),
                                            control2: control2.clone(),
                                            end: end.clone(),
                                            segments: segments_val,
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <Vec2Field
                                label="Control 2"
                                value={ctrl2_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let start = start.clone();
                                    let control1 = control1.clone();
                                    let end = end.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Bezier {
                                            start: start.clone(),
                                            control1: control1.clone(),
                                            control2: Vec2OrExpr::Static(v),
                                            end: end.clone(),
                                            segments: segments_val,
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <Vec2Field
                                label="End"
                                value={end_val}
                                on_change={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let start = start.clone();
                                    let control1 = control1.clone();
                                    let control2 = control2.clone();
                                    Callback::from(move |v: [f32; 2]| {
                                        let mut new_obj = object.clone();
                                        new_obj.shape = Shape::Bezier {
                                            start: start.clone(),
                                            control1: control1.clone(),
                                            control2: control2.clone(),
                                            end: Vec2OrExpr::Static(v),
                                            segments: segments_val,
                                        };
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                        </>
                    }
                },
            }}
        </div>
    }
}

/// Props for PropertiesEditor.
#[derive(Properties, PartialEq)]
struct PropertiesEditorProps {
    properties: ObjectProperties,
    role: ObjectRole,
    index: usize,
    on_update: Callback<(usize, MapObject)>,
    object: MapObject,
}

/// Properties editor component.
#[function_component(PropertiesEditor)]
fn properties_editor(props: &PropertiesEditorProps) -> Html {
    let index = props.index;
    let on_update = props.on_update.clone();
    let object = props.object.clone();

    match props.role {
        ObjectRole::Spawner => {
            let spawn = props.properties.spawn.clone().unwrap_or_default();
            html! {
                <div class="property-section">
                    <div class="property-section-title">{"Spawn Properties"}</div>
                    <div class="property-field">
                        <label>{"Mode"}</label>
                        <select onchange={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let spawn = spawn.clone();
                            Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let mut new_obj = object.clone();
                                new_obj.properties.spawn = Some(SpawnProperties {
                                    mode: input.value(),
                                    ..spawn.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}>
                            <option value="random" selected={spawn.mode == "random"}>{"Random"}</option>
                            <option value="center" selected={spawn.mode == "center"}>{"Center"}</option>
                        </select>
                    </div>
                    <div class="property-field">
                        <label>{"Initial Force"}</label>
                        <select onchange={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let spawn = spawn.clone();
                            Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let mut new_obj = object.clone();
                                new_obj.properties.spawn = Some(SpawnProperties {
                                    initial_force: input.value(),
                                    ..spawn.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}>
                            <option value="random" selected={spawn.initial_force == "random"}>{"Random"}</option>
                            <option value="none" selected={spawn.initial_force == "none"}>{"None"}</option>
                            <option value="down" selected={spawn.initial_force == "down"}>{"Down"}</option>
                        </select>
                    </div>
                </div>
            }
        }
        ObjectRole::Trigger => {
            let trigger = props.properties.trigger.clone();
            let action = trigger.as_ref().map(|t| t.action.clone()).unwrap_or_else(|| "gamerule".to_string());
            html! {
                <div class="property-section">
                    <div class="property-section-title">{"Trigger Properties"}</div>
                    <div class="property-field">
                        <label>{"Action"}</label>
                        <select onchange={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let mut new_obj = object.clone();
                                new_obj.properties.trigger = Some(TriggerProperties {
                                    action: input.value(),
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}>
                            <option value="gamerule" selected={action == "gamerule"}>{"Gamerule (Arrive)"}</option>
                            <option value="eliminate" selected={action == "eliminate"}>{"Eliminate"}</option>
                        </select>
                    </div>
                </div>
            }
        }
        ObjectRole::Obstacle => {
            let has_bumper = props.properties.bumper.is_some();
            let bumper_force = props.properties.bumper.as_ref()
                .and_then(|b| get_number_static(&b.force))
                .unwrap_or(1.0);
            html! {
                <div class="property-section">
                    <div class="property-section-title">{"Obstacle Properties"}</div>
                    <div class="property-field property-field-checkbox">
                        <label>
                            <input
                                type="checkbox"
                                checked={has_bumper}
                                onchange={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let mut new_obj = object.clone();
                                        if input.checked() {
                                            new_obj.properties.bumper = Some(BumperProperties {
                                                force: NumberOrExpr::Number(1.0),
                                            });
                                        } else {
                                            new_obj.properties.bumper = None;
                                        }
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            {"Bumper"}
                        </label>
                    </div>
                    if has_bumper {
                        <NumberField
                            label="Bumper Force"
                            value={bumper_force}
                            on_change={{
                                let on_update = on_update.clone();
                                let object = object.clone();
                                Callback::from(move |v: f32| {
                                    let mut new_obj = object.clone();
                                    new_obj.properties.bumper = Some(BumperProperties {
                                        force: NumberOrExpr::Number(v),
                                    });
                                    on_update.emit((index, new_obj));
                                })
                            }}
                        />
                    }
                </div>
            }
        }
        ObjectRole::Guideline => {
            let guideline = props.properties.guideline.clone().unwrap_or_default();
            html! {
                <div class="property-section">
                    <div class="property-section-title">{"Guideline Properties"}</div>
                    <div class="property-field property-field-checkbox">
                        <label>
                            <input
                                type="checkbox"
                                checked={guideline.show_ruler}
                                onchange={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let guideline = guideline.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let mut new_obj = object.clone();
                                        new_obj.properties.guideline = Some(GuidelineProperties {
                                            show_ruler: input.checked(),
                                            ..guideline.clone()
                                        });
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            {"Show Ruler"}
                        </label>
                    </div>
                    <div class="property-field property-field-checkbox">
                        <label>
                            <input
                                type="checkbox"
                                checked={guideline.snap_enabled}
                                onchange={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let guideline = guideline.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let mut new_obj = object.clone();
                                        new_obj.properties.guideline = Some(GuidelineProperties {
                                            snap_enabled: input.checked(),
                                            ..guideline.clone()
                                        });
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            {"Snap Enabled"}
                        </label>
                    </div>
                    <NumberField
                        label="Snap Distance"
                        value={guideline.snap_distance}
                        on_change={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let guideline = guideline.clone();
                            Callback::from(move |v: f32| {
                                let mut new_obj = object.clone();
                                new_obj.properties.guideline = Some(GuidelineProperties {
                                    snap_distance: v,
                                    ..guideline.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}
                    />
                    <div class="property-group">
                        <label class="property-label">{"Ruler Interval"}</label>
                        <div class="interval-presets">
                            {for [0.25, 0.5, 1.0, 2.0].iter().map(|&interval| {
                                let on_update = on_update.clone();
                                let object = object.clone();
                                let guideline = guideline.clone();
                                let is_selected = (guideline.ruler_interval - interval).abs() < 0.01;
                                let onclick = Callback::from(move |_: MouseEvent| {
                                    let mut new_obj = object.clone();
                                    new_obj.properties.guideline = Some(GuidelineProperties {
                                        ruler_interval: interval,
                                        ..guideline.clone()
                                    });
                                    on_update.emit((index, new_obj));
                                });
                                html! {
                                    <button
                                        class={classes!("interval-btn", is_selected.then_some("selected"))}
                                        onclick={onclick}
                                    >
                                        {format!("{}m", interval)}
                                    </button>
                                }
                            })}
                        </div>
                        <NumberField
                            label="Custom"
                            value={guideline.ruler_interval}
                            on_change={{
                                let on_update = on_update.clone();
                                let object = object.clone();
                                let guideline = guideline.clone();
                                Callback::from(move |v: f32| {
                                    let mut new_obj = object.clone();
                                    new_obj.properties.guideline = Some(GuidelineProperties {
                                        ruler_interval: v.max(0.01),
                                        ..guideline.clone()
                                    });
                                    on_update.emit((index, new_obj));
                                })
                            }}
                        />
                    </div>
                </div>
            }
        }
        ObjectRole::VectorField => {
            let vf = props.properties.vector_field.clone().unwrap_or_else(|| VectorFieldProperties {
                direction: Vec2OrExpr::Static([0.0, -1.0]),
                magnitude: NumberOrExpr::Number(5.0),
                enabled: BoolOrExpr::Bool(true),
                falloff: VectorFieldFalloff::Uniform,
            });
            let direction = get_vec2_static(&vf.direction).unwrap_or([0.0, -1.0]);
            let magnitude = get_number_static(&vf.magnitude).unwrap_or(5.0);
            let enabled_bool = get_bool_static(&vf.enabled).unwrap_or(true);
            let enabled_expr = get_bool_expr(&vf.enabled).unwrap_or("").to_string();
            let has_expr = !enabled_expr.is_empty();

            html! {
                <div class="property-section">
                    <div class="property-section-title">{"Vector Field Properties"}</div>
                    <div class="property-field-hint">
                        {"Vector fields apply directional forces to marbles within their area."}
                    </div>

                    // Enabled: checkbox + CEL expression input
                    <div class="property-field">
                        <label>{"Enabled"}</label>
                        <div class="property-field-row">
                            <input
                                type="checkbox"
                                checked={enabled_bool}
                                disabled={has_expr}
                                onchange={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let vf = vf.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let mut new_obj = object.clone();
                                        new_obj.properties.vector_field = Some(VectorFieldProperties {
                                            enabled: BoolOrExpr::Bool(input.checked()),
                                            ..vf.clone()
                                        });
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                            <input
                                type="text"
                                class="expr-input"
                                value={enabled_expr.clone()}
                                placeholder="CEL expr (e.g. game.time > 5)"
                                oninput={{
                                    let on_update = on_update.clone();
                                    let object = object.clone();
                                    let vf = vf.clone();
                                    let enabled_bool = enabled_bool;
                                    Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let expr = input.value();
                                        let mut new_obj = object.clone();
                                        let enabled = if expr.trim().is_empty() {
                                            BoolOrExpr::Bool(enabled_bool)
                                        } else {
                                            BoolOrExpr::Expr(expr)
                                        };
                                        new_obj.properties.vector_field = Some(VectorFieldProperties {
                                            enabled,
                                            ..vf.clone()
                                        });
                                        on_update.emit((index, new_obj));
                                    })
                                }}
                            />
                        </div>
                    </div>

                    // Direction X
                    <NumberField
                        label="Direction X"
                        value={direction[0]}
                        on_change={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let vf = vf.clone();
                            let direction = direction;
                            Callback::from(move |v: f32| {
                                let mut new_obj = object.clone();
                                new_obj.properties.vector_field = Some(VectorFieldProperties {
                                    direction: Vec2OrExpr::Static([v, direction[1]]),
                                    ..vf.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}
                    />

                    // Direction Y
                    <NumberField
                        label="Direction Y"
                        value={direction[1]}
                        on_change={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let vf = vf.clone();
                            let direction = direction;
                            Callback::from(move |v: f32| {
                                let mut new_obj = object.clone();
                                new_obj.properties.vector_field = Some(VectorFieldProperties {
                                    direction: Vec2OrExpr::Static([direction[0], v]),
                                    ..vf.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}
                    />

                    // Magnitude
                    <NumberField
                        label="Magnitude"
                        value={magnitude}
                        on_change={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let vf = vf.clone();
                            Callback::from(move |v: f32| {
                                let mut new_obj = object.clone();
                                new_obj.properties.vector_field = Some(VectorFieldProperties {
                                    magnitude: NumberOrExpr::Number(v),
                                    ..vf.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}
                    />

                    // Falloff select
                    <div class="property-field">
                        <label>{"Falloff"}</label>
                        <select onchange={{
                            let on_update = on_update.clone();
                            let object = object.clone();
                            let vf = vf.clone();
                            Callback::from(move |e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let falloff = match input.value().as_str() {
                                    "distance_based" => VectorFieldFalloff::DistanceBased,
                                    _ => VectorFieldFalloff::Uniform,
                                };
                                let mut new_obj = object.clone();
                                new_obj.properties.vector_field = Some(VectorFieldProperties {
                                    falloff,
                                    ..vf.clone()
                                });
                                on_update.emit((index, new_obj));
                            })
                        }}>
                            <option value="uniform" selected={vf.falloff == VectorFieldFalloff::Uniform}>
                                {"Uniform"}
                            </option>
                            <option value="distance_based" selected={vf.falloff == VectorFieldFalloff::DistanceBased}>
                                {"Distance Based"}
                            </option>
                        </select>
                    </div>
                </div>
            }
        }
    }
}

/// Props for MetaPropertiesPanel.
#[derive(Properties, PartialEq)]
struct MetaPropertiesPanelProps {
    meta: MapMeta,
    on_update: Callback<MapMeta>,
}

/// Panel for editing map metadata.
#[function_component(MetaPropertiesPanel)]
fn meta_properties_panel(props: &MetaPropertiesPanelProps) -> Html {
    let on_name_input = {
        let meta = props.meta.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut new_meta = meta.clone();
            new_meta.name = input.value();
            on_update.emit(new_meta);
        })
    };

    let on_gamerule_input = {
        let meta = props.meta.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut new_meta = meta.clone();
            new_meta.gamerule = input.value()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            on_update.emit(new_meta);
        })
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">{"Map Info"}</div>
                <div class="property-field">
                    <label>{"Name"}</label>
                    <input
                        type="text"
                        value={props.meta.name.clone()}
                        oninput={on_name_input}
                    />
                </div>
                <div class="property-field">
                    <label>{"Game Rules"}</label>
                    <input
                        type="text"
                        value={props.meta.gamerule.join(", ")}
                        oninput={on_gamerule_input}
                        placeholder="e.g., top_n, elimination"
                    />
                </div>
            </div>
        </div>
    }
}

/// Props for Vec2Field.
#[derive(Properties, PartialEq)]
struct Vec2FieldProps {
    label: &'static str,
    value: [f32; 2],
    on_change: Callback<[f32; 2]>,
}

/// Vec2 input field component.
#[function_component(Vec2Field)]
fn vec2_field(props: &Vec2FieldProps) -> Html {
    let value = props.value;
    let on_change = props.on_change.clone();

    let on_x_input = {
        let on_change = on_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(x) = input.value().parse() {
                on_change.emit([x, value[1]]);
            }
        })
    };

    let on_y_input = {
        let on_change = on_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(y) = input.value().parse() {
                on_change.emit([value[0], y]);
            }
        })
    };

    html! {
        <div class="property-field property-field-vec2">
            <label>{props.label}</label>
            <div class="vec2-inputs">
                <input
                    type="number"
                    value={value[0].to_string()}
                    oninput={on_x_input}
                    step="1"
                />
                <input
                    type="number"
                    value={value[1].to_string()}
                    oninput={on_y_input}
                    step="1"
                />
            </div>
        </div>
    }
}

/// Props for NumberField.
#[derive(Properties, PartialEq)]
struct NumberFieldProps {
    label: &'static str,
    value: f32,
    on_change: Callback<f32>,
}

/// Number input field component.
#[function_component(NumberField)]
fn number_field(props: &NumberFieldProps) -> Html {
    let on_input = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(v) = input.value().parse() {
                on_change.emit(v);
            }
        })
    };

    html! {
        <div class="property-field">
            <label>{props.label}</label>
            <input
                type="number"
                value={props.value.to_string()}
                oninput={on_input}
                step="any"
            />
        </div>
    }
}

// ============================================================================
// Keyframe Properties Panel
// ============================================================================

/// Props for KeyframePropertiesPanel.
#[derive(Properties, PartialEq)]
struct KeyframePropertiesPanelProps {
    sequence: Option<KeyframeSequence>,
    selected_keyframe: Option<usize>,
    on_update_keyframe: Callback<(usize, Keyframe)>,
}

/// Panel for editing selected keyframe properties.
#[function_component(KeyframePropertiesPanel)]
fn keyframe_properties_panel(props: &KeyframePropertiesPanelProps) -> Html {
    let Some(seq) = &props.sequence else {
        return html! {
            <div class="property-empty">
                {"Select a sequence from the Keyframes tab"}
            </div>
        };
    };

    let Some(kf_idx) = props.selected_keyframe else {
        return html! {
            <div class="property-empty">
                {"Select a keyframe from the timeline"}
            </div>
        };
    };

    let Some(keyframe) = seq.keyframes.get(kf_idx) else {
        return html! {
            <div class="property-empty">
                {"Keyframe not found"}
            </div>
        };
    };

    html! {
        <div class="keyframe-editor">
            {render_keyframe_editor(
                keyframe.clone(),
                kf_idx,
                &props.on_update_keyframe,
            )}
        </div>
    }
}

/// Renders the keyframe editor based on keyframe type.
fn render_keyframe_editor(
    keyframe: Keyframe,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    match keyframe {
        Keyframe::LoopStart { count } => render_loop_start_editor(count, kf_idx, on_update),
        Keyframe::LoopEnd => render_loop_end_editor(),
        Keyframe::Delay { duration } => render_delay_editor(duration, kf_idx, on_update),
        Keyframe::Apply {
            translation,
            rotation,
            duration,
            easing,
        } => render_apply_editor(translation, rotation, duration, easing, kf_idx, on_update),
        Keyframe::PivotRotate {
            pivot,
            pivot_mode,
            angle,
            duration,
            easing,
        } => render_pivot_editor(pivot, pivot_mode, angle, duration, easing, kf_idx, on_update),
        Keyframe::ContinuousRotate { speed, direction } => {
            render_continuous_rotate_editor(speed, direction, kf_idx, on_update)
        }
    }
}

/// LoopStart editor: count input (number or infinite)
fn render_loop_start_editor(
    count: Option<u32>,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    let on_count_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let value = input.value();
                let new_count = if value.is_empty() {
                    None // infinite
                } else {
                    value.parse::<u32>().ok()
                };
                on_update.emit((kf_idx, Keyframe::LoopStart { count: new_count }));
            }
        })
    };

    let count_str = count.map(|n| n.to_string()).unwrap_or_default();

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_REPEAT} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Loop Start"}</span>
                </div>
                <div class="property-field">
                    <label>{"Count"}</label>
                    <input
                        type="text"
                        placeholder="∞ (infinite)"
                        value={count_str}
                        oninput={on_count_change}
                    />
                    <span class="property-note">{"Leave empty for infinite loop"}</span>
                </div>
            </div>
        </div>
    }
}

/// LoopEnd editor: no editable fields
fn render_loop_end_editor() -> Html {
    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_CORNER_DOWN_LEFT} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Loop End"}</span>
                </div>
                <div class="property-note">
                    {"Marks the end of a loop block. No editable properties."}
                </div>
            </div>
        </div>
    }
}

/// Delay editor: duration (number or CEL expression)
fn render_delay_editor(
    duration: NumberOrExpr,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    let is_expr = matches!(duration, NumberOrExpr::Expr(_));
    let duration_str = match &duration {
        NumberOrExpr::Number(n) => n.to_string(),
        NumberOrExpr::Expr(e) => e.clone(),
    };

    let on_duration_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let value = input.value();
                let new_duration = if let Ok(n) = value.parse::<f32>() {
                    NumberOrExpr::Number(n)
                } else {
                    NumberOrExpr::Expr(value)
                };
                on_update.emit((kf_idx, Keyframe::Delay { duration: new_duration }));
            }
        })
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_TIMER} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Delay"}</span>
                </div>
                <div class="property-field">
                    <label>{"Duration (seconds)"}</label>
                    <input
                        type="text"
                        class={is_expr.then_some("cel-expr")}
                        placeholder="e.g. 1.5 or random(1, 3)"
                        value={duration_str}
                        oninput={on_duration_change}
                    />
                    if is_expr {
                        <span class="property-note" style="color: #ff9800;">{"CEL expression - duration varies at runtime"}</span>
                    } else {
                        <span class="property-note">{"Enter a number or CEL expression like random(1, 3)"}</span>
                    }
                </div>
            </div>
        </div>
    }
}

/// Apply editor: translation, rotation, duration, easing
fn render_apply_editor(
    translation: Option<[f32; 2]>,
    rotation: Option<f32>,
    duration: f32,
    easing: EasingType,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    let trans_x = translation.map(|t| t[0]).unwrap_or(0.0);
    let trans_y = translation.map(|t| t[1]).unwrap_or(0.0);
    let rot_deg = rotation.unwrap_or(0.0);

    // Translation X
    let on_trans_x_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = input.value().parse::<f32>().unwrap_or(0.0);
                let y = translation.map(|t| t[1]).unwrap_or(0.0);
                let new_translation = if x == 0.0 && y == 0.0 { None } else { Some([x, y]) };
                on_update.emit((
                    kf_idx,
                    Keyframe::Apply {
                        translation: new_translation,
                        rotation,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Translation Y
    let on_trans_y_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = translation.map(|t| t[0]).unwrap_or(0.0);
                let y = input.value().parse::<f32>().unwrap_or(0.0);
                let new_translation = if x == 0.0 && y == 0.0 { None } else { Some([x, y]) };
                on_update.emit((
                    kf_idx,
                    Keyframe::Apply {
                        translation: new_translation,
                        rotation,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Rotation
    let on_rotation_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let deg = input.value().parse::<f32>().unwrap_or(0.0);
                let new_rotation = if deg == 0.0 { None } else { Some(deg) };
                on_update.emit((
                    kf_idx,
                    Keyframe::Apply {
                        translation,
                        rotation: new_rotation,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Duration
    let on_duration_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_duration = input.value().parse::<f32>().unwrap_or(0.5).max(0.01);
                on_update.emit((
                    kf_idx,
                    Keyframe::Apply {
                        translation,
                        rotation,
                        duration: new_duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Easing
    let on_easing_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_easing = match select.value().as_str() {
                    "ease_in" => EasingType::EaseIn,
                    "ease_out" => EasingType::EaseOut,
                    "ease_in_out" => EasingType::EaseInOut,
                    _ => EasingType::Linear,
                };
                on_update.emit((
                    kf_idx,
                    Keyframe::Apply {
                        translation,
                        rotation,
                        duration,
                        easing: new_easing,
                    },
                ));
            }
        })
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_MOVE} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Apply Transform"}</span>
                </div>
                <div class="property-field property-field-vec2">
                    <label>{"Translation"}</label>
                    <div class="vec2-inputs">
                        <input
                            type="number"
                            step="1"
                            placeholder="X"
                            value={trans_x.to_string()}
                            oninput={on_trans_x_change}
                        />
                        <input
                            type="number"
                            step="1"
                            placeholder="Y"
                            value={trans_y.to_string()}
                            oninput={on_trans_y_change}
                        />
                    </div>
                </div>
                <div class="property-field">
                    <label>{"Rotation (°)"}</label>
                    <input
                        type="number"
                        step="1"
                        value={rot_deg.to_string()}
                        oninput={on_rotation_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Duration (s)"}</label>
                    <input
                        type="number"
                        step="0.1"
                        min="0.01"
                        value={duration.to_string()}
                        oninput={on_duration_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Easing"}</label>
                    {render_easing_select(easing, on_easing_change)}
                </div>
            </div>
        </div>
    }
}

/// PivotRotate editor: pivot, pivot_mode, angle, duration, easing
fn render_pivot_editor(
    pivot: [f32; 2],
    pivot_mode: PivotMode,
    angle: f32,
    duration: f32,
    easing: EasingType,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    // Pivot Mode
    let on_mode_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_mode = match select.value().as_str() {
                    "relative" => PivotMode::Relative,
                    _ => PivotMode::Absolute,
                };
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot,
                        pivot_mode: new_mode,
                        angle,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Pivot X
    let on_pivot_x_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot: [x, pivot[1]],
                        pivot_mode,
                        angle,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Pivot Y
    let on_pivot_y_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let y = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot: [pivot[0], y],
                        pivot_mode,
                        angle,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Angle
    let on_angle_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_angle = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot,
                        pivot_mode,
                        angle: new_angle,
                        duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Duration
    let on_duration_change = {
        let on_update = on_update.clone();
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_duration = input.value().parse::<f32>().unwrap_or(0.5).max(0.01);
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot,
                        pivot_mode,
                        angle,
                        duration: new_duration,
                        easing,
                    },
                ));
            }
        })
    };

    // Easing
    let on_easing_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_easing = match select.value().as_str() {
                    "ease_in" => EasingType::EaseIn,
                    "ease_out" => EasingType::EaseOut,
                    "ease_in_out" => EasingType::EaseInOut,
                    _ => EasingType::Linear,
                };
                on_update.emit((
                    kf_idx,
                    Keyframe::PivotRotate {
                        pivot,
                        pivot_mode,
                        angle,
                        duration,
                        easing: new_easing,
                    },
                ));
            }
        })
    };

    let mode_label = match pivot_mode {
        PivotMode::Absolute => "World coordinates",
        PivotMode::Relative => "Offset from object center",
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_ROTATE_CW} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Pivot Rotate"}</span>
                </div>
                <div class="property-field">
                    <label>{"Pivot Mode"}</label>
                    <select onchange={on_mode_change}>
                        <option value="absolute" selected={pivot_mode == PivotMode::Absolute}>{"Absolute (World)"}</option>
                        <option value="relative" selected={pivot_mode == PivotMode::Relative}>{"Relative (Object)"}</option>
                    </select>
                    <span class="property-note">{mode_label}</span>
                </div>
                <div class="property-field property-field-vec2">
                    <label>{"Pivot Point"}</label>
                    <div class="vec2-inputs">
                        <input
                            type="number"
                            step="0.1"
                            placeholder="X"
                            value={pivot[0].to_string()}
                            oninput={on_pivot_x_change}
                        />
                        <input
                            type="number"
                            step="0.1"
                            placeholder="Y"
                            value={pivot[1].to_string()}
                            oninput={on_pivot_y_change}
                        />
                    </div>
                </div>
                <div class="property-field">
                    <label>{"Angle (°)"}</label>
                    <input
                        type="number"
                        step="1"
                        value={angle.to_string()}
                        oninput={on_angle_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Duration (s)"}</label>
                    <input
                        type="number"
                        step="0.1"
                        min="0.01"
                        value={duration.to_string()}
                        oninput={on_duration_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Easing"}</label>
                    {render_easing_select(easing, on_easing_change)}
                </div>
            </div>
        </div>
    }
}

/// Renders an easing type selector
fn render_easing_select(current: EasingType, on_change: Callback<Event>) -> Html {
    html! {
        <select onchange={on_change}>
            <option value="linear" selected={current == EasingType::Linear}>{"Linear"}</option>
            <option value="ease_in" selected={current == EasingType::EaseIn}>{"Ease In"}</option>
            <option value="ease_out" selected={current == EasingType::EaseOut}>{"Ease Out"}</option>
            <option value="ease_in_out" selected={current == EasingType::EaseInOut}>{"Ease In Out"}</option>
        </select>
    }
}

/// ContinuousRotate editor: speed and direction
fn render_continuous_rotate_editor(
    speed: f32,
    direction: RollDirection,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
    let on_speed_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_speed = input.value().parse::<f32>().unwrap_or(45.0).max(0.0);
                on_update.emit((
                    kf_idx,
                    Keyframe::ContinuousRotate {
                        speed: new_speed,
                        direction,
                    },
                ));
            }
        })
    };

    let on_direction_change = {
        let on_update = on_update.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_direction = match select.value().as_str() {
                    "counterclockwise" => RollDirection::Counterclockwise,
                    _ => RollDirection::Clockwise,
                };
                on_update.emit((
                    kf_idx,
                    Keyframe::ContinuousRotate {
                        speed,
                        direction: new_direction,
                    },
                ));
            }
        })
    };

    html! {
        <div class="property-fields">
            <div class="property-section">
                <div class="property-section-title">
                    <Icon data={IconData::LUCIDE_REFRESH_CW} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Continuous Rotate"}</span>
                </div>
                <div class="property-field">
                    <label>{"Speed (°/s)"}</label>
                    <input
                        type="number"
                        step="5"
                        min="0"
                        value={speed.to_string()}
                        oninput={on_speed_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Direction"}</label>
                    <select onchange={on_direction_change}>
                        <option value="clockwise" selected={direction == RollDirection::Clockwise}>{"Clockwise"}</option>
                        <option value="counterclockwise" selected={direction == RollDirection::Counterclockwise}>{"Counter-CW"}</option>
                    </select>
                </div>
            </div>
        </div>
    }
}
