//! Property panel component for editing object and meta properties.

use marble_core::dsl::{NumberOrExpr, Vec2OrExpr};
use marble_core::map::{
    BumperProperties, EasingType, Keyframe, KeyframeSequence, MapMeta, MapObject, ObjectProperties,
    ObjectRole, RouletteConfig, Shape, SpawnProperties, TriggerProperties,
};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_icons::{Icon, IconData};

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
}

/// Property panel component.
#[function_component(PropertyPanel)]
pub fn property_panel(props: &PropertyPanelProps) -> Html {
    let active_tab = use_state(|| "object".to_string());

    // Auto-switch to object tab when an object is selected
    {
        let active_tab = active_tab.clone();
        let selected_index = props.selected_index;
        use_effect_with(selected_index, move |idx| {
            if idx.is_some() {
                active_tab.set("object".to_string());
            }
        });
    }

    // Auto-switch to keyframe tab when a keyframe is selected
    {
        let active_tab = active_tab.clone();
        let selected_keyframe = props.selected_keyframe;
        use_effect_with(selected_keyframe, move |kf| {
            if kf.is_some() {
                active_tab.set("keyframe".to_string());
            }
        });
    }

    let on_tab_click = {
        let active_tab = active_tab.clone();
        Callback::from(move |tab: String| {
            active_tab.set(tab);
        })
    };

    html! {
        <div class="property-panel">
            <div class="property-tabs">
                <button
                    class={classes!("property-tab", (*active_tab == "object").then_some("active"))}
                    onclick={{
                        let on_tab_click = on_tab_click.clone();
                        Callback::from(move |_: MouseEvent| on_tab_click.emit("object".to_string()))
                    }}
                >
                    {"Object"}
                </button>
                <button
                    class={classes!("property-tab", (*active_tab == "meta").then_some("active"))}
                    onclick={{
                        let on_tab_click = on_tab_click.clone();
                        Callback::from(move |_: MouseEvent| on_tab_click.emit("meta".to_string()))
                    }}
                >
                    {"Meta"}
                </button>
                <button
                    class={classes!("property-tab", (*active_tab == "keyframe").then_some("active"))}
                    onclick={{
                        let on_tab_click = on_tab_click.clone();
                        Callback::from(move |_: MouseEvent| on_tab_click.emit("keyframe".to_string()))
                    }}
                >
                    {"KF"}
                </button>
            </div>
            <div class="property-content">
                if *active_tab == "object" {
                    <ObjectPropertiesPanel
                        config={props.config.clone()}
                        selected_index={props.selected_index}
                        on_update={props.on_update_object.clone()}
                    />
                } else if *active_tab == "meta" {
                    <MetaPropertiesPanel
                        meta={props.config.meta.clone()}
                        on_update={props.on_update_meta.clone()}
                    />
                } else {
                    <KeyframePropertiesPanel
                        sequence={props.sequence.clone()}
                        selected_keyframe={props.selected_keyframe}
                        on_update_keyframe={props.on_update_keyframe.clone()}
                    />
                }
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
    let on_id_change = {
        let object = object.clone();
        let on_update = on_update.clone();
        Callback::from(move |e: Event| {
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
                        onchange={on_id_change}
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
}

/// Shape editor component.
#[function_component(ShapeEditor)]
fn shape_editor(props: &ShapeEditorProps) -> Html {
    let index = props.index;
    let on_update = props.on_update.clone();
    let object = props.object.clone();

    let on_shape_type_change = {
        let on_update = on_update.clone();
        let object = object.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let new_shape = match input.value().as_str() {
                "line" => Shape::Line {
                    start: Vec2OrExpr::Static([0.0, 0.0]),
                    end: Vec2OrExpr::Static([100.0, 0.0]),
                },
                "circle" => Shape::Circle {
                    center: Vec2OrExpr::Static([400.0, 300.0]),
                    radius: NumberOrExpr::Number(30.0),
                },
                "rect" => Shape::Rect {
                    center: Vec2OrExpr::Static([400.0, 300.0]),
                    size: Vec2OrExpr::Static([100.0, 50.0]),
                    rotation: NumberOrExpr::Number(0.0),
                },
                "bezier" => Shape::Bezier {
                    start: Vec2OrExpr::Static([350.0, 300.0]),
                    control1: Vec2OrExpr::Static([375.0, 250.0]),
                    control2: Vec2OrExpr::Static([425.0, 350.0]),
                    end: Vec2OrExpr::Static([450.0, 300.0]),
                    segments: 16,
                },
                _ => return,
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
                    let size_val = get_vec2_static(size).unwrap_or([100.0, 50.0]);
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
                    let end_val = get_vec2_static(end).unwrap_or([100.0, 0.0]);
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
    let on_name_change = {
        let meta = props.meta.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut new_meta = meta.clone();
            new_meta.name = input.value();
            on_update.emit(new_meta);
        })
    };

    let on_gamerule_change = {
        let meta = props.meta.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |e: Event| {
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
                        onchange={on_name_change}
                    />
                </div>
                <div class="property-field">
                    <label>{"Game Rules"}</label>
                    <input
                        type="text"
                        value={props.meta.gamerule.join(", ")}
                        onchange={on_gamerule_change}
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

    let on_x_change = {
        let on_change = on_change.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(x) = input.value().parse() {
                on_change.emit([x, value[1]]);
            }
        })
    };

    let on_y_change = {
        let on_change = on_change.clone();
        Callback::from(move |e: Event| {
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
                    onchange={on_x_change}
                    step="1"
                />
                <input
                    type="number"
                    value={value[1].to_string()}
                    onchange={on_y_change}
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
    let on_change = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: Event| {
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
                onchange={on_change}
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
            angle,
            duration,
            easing,
        } => render_pivot_editor(pivot, angle, duration, easing, kf_idx, on_update),
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

/// PivotRotate editor: pivot, angle, duration, easing
fn render_pivot_editor(
    pivot: [f32; 2],
    angle: f32,
    duration: f32,
    easing: EasingType,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
) -> Html {
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
                        angle,
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
                    <Icon data={IconData::LUCIDE_ROTATE_CW} width="14px" height="14px" />
                    <span style="margin-left: 6px;">{"Pivot Rotate"}</span>
                </div>
                <div class="property-field property-field-vec2">
                    <label>{"Pivot Point"}</label>
                    <div class="vec2-inputs">
                        <input
                            type="number"
                            step="1"
                            placeholder="X"
                            value={pivot[0].to_string()}
                            oninput={on_pivot_x_change}
                        />
                        <input
                            type="number"
                            step="1"
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
