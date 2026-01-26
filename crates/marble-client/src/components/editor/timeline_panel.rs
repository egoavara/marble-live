//! Timeline panel component for keyframe animation editing.

use marble_core::dsl::NumberOrExpr;
use marble_core::map::{EasingType, Keyframe, KeyframeSequence};
use yew::prelude::*;
use yew_icons::{Icon, IconData};

/// Props for the TimelinePanel component.
#[derive(Properties, PartialEq)]
pub struct TimelinePanelProps {
    pub sequence: Option<KeyframeSequence>,
    pub sequence_index: Option<usize>,
    pub selected_keyframe: Option<usize>,
    pub on_select_keyframe: Callback<Option<usize>>,
    pub on_add_keyframe: Callback<Keyframe>,
    pub on_update_keyframe: Callback<(usize, Keyframe)>,
    pub on_delete_keyframe: Callback<usize>,
    #[prop_or_default]
    pub on_move_keyframe: Callback<(usize, usize)>,
    // Sequence update callback for target_ids editing and keyframe insertion
    #[prop_or_default]
    pub on_update_sequence: Callback<KeyframeSequence>,
    // Available object IDs for target selection
    #[prop_or_default]
    pub available_object_ids: Vec<String>,
    /// Preview the entire sequence animation
    #[prop_or_default]
    pub on_preview_sequence: Callback<()>,
    /// Whether preview is currently playing
    #[prop_or_default]
    pub is_previewing: bool,
}

/// Timeline panel component for editing keyframe sequences.
#[function_component(TimelinePanel)]
pub fn timeline_panel(props: &TimelinePanelProps) -> Html {
    let editing_targets = use_state(|| false);
    let new_target_input = use_state(|| String::new());
    // Index where to insert new keyframe (None = no menu shown)
    let insert_at: UseStateHandle<Option<usize>> = use_state(|| None);
    // Drag state for reordering keyframes
    let dragging_index: UseStateHandle<Option<usize>> = use_state(|| None);
    let drag_over_index: UseStateHandle<Option<usize>> = use_state(|| None);

    let toggle_editing_targets = {
        let editing_targets = editing_targets.clone();
        Callback::from(move |_: MouseEvent| {
            editing_targets.set(!*editing_targets);
        })
    };

    // Keyframe type: (css_class, icon, label, is_cel_expr)
    fn keyframe_info(kf: &Keyframe) -> (&'static str, IconData, String, bool) {
        match kf {
            Keyframe::LoopStart { count } => {
                let label = match count {
                    Some(n) => format!("{}", n),
                    None => "".to_string(),
                };
                ("loop-start", IconData::LUCIDE_REPEAT, label, false)
            }
            Keyframe::LoopEnd => ("loop-end", IconData::LUCIDE_CORNER_DOWN_LEFT, String::new(), false),
            Keyframe::Delay { duration } => {
                match duration {
                    NumberOrExpr::Number(n) => ("delay", IconData::LUCIDE_TIMER, format!("{:.1}s", n), false),
                    NumberOrExpr::Expr(_) => ("delay cel-expr", IconData::LUCIDE_TIMER, String::new(), true),
                }
            }
            Keyframe::Apply { duration, .. } => {
                ("apply", IconData::LUCIDE_MOVE, format!("{:.1}s", duration), false)
            }
            Keyframe::PivotRotate { duration, .. } => {
                ("pivot", IconData::LUCIDE_ROTATE_CW, format!("{:.1}s", duration), false)
            }
        }
    }

    // Target management
    let on_add_target = {
        let on_update_sequence = props.on_update_sequence.clone();
        let sequence = props.sequence.clone();
        let new_target_input = new_target_input.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(mut seq) = sequence.clone() {
                let new_target = (*new_target_input).trim().to_string();
                if !new_target.is_empty() && !seq.target_ids.contains(&new_target) {
                    seq.target_ids.push(new_target);
                    on_update_sequence.emit(seq);
                    new_target_input.set(String::new());
                }
            }
        })
    };

    let on_target_input = {
        let new_target_input = new_target_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                new_target_input.set(input.value());
            }
        })
    };

    let on_target_select = {
        let on_update_sequence = props.on_update_sequence.clone();
        let sequence = props.sequence.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let selected = select.value();
                if let Some(mut seq) = sequence.clone() {
                    if !selected.is_empty() && !seq.target_ids.contains(&selected) {
                        seq.target_ids.push(selected);
                        on_update_sequence.emit(seq);
                    }
                }
                select.set_value("");
            }
        })
    };

    // Close insert menu when clicking outside
    let close_insert_menu = {
        let insert_at = insert_at.clone();
        Callback::from(move |_: MouseEvent| {
            insert_at.set(None);
        })
    };

    // Helper to render insert button
    let render_insert_btn = |idx: usize| {
        let is_menu_open = **&insert_at == Some(idx);
        let insert_at_clone = insert_at.clone();
        let on_update_sequence = props.on_update_sequence.clone();
        let sequence = props.sequence.clone();

        let on_plus_click = {
            let insert_at = insert_at_clone.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                insert_at.set(Some(idx));
            })
        };

        let make_insert = |kf: Keyframe| {
            let on_update_sequence = on_update_sequence.clone();
            let sequence = sequence.clone();
            let insert_at = insert_at_clone.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                if let Some(mut seq) = sequence.clone() {
                    seq.keyframes.insert(idx, kf.clone());
                    on_update_sequence.emit(seq);
                }
                insert_at.set(None);
            })
        };

        html! {
            <div class={classes!("timeline-insert-btn-wrapper", is_menu_open.then_some("menu-open"))}>
                <button
                    class="timeline-insert-btn"
                    onclick={on_plus_click}
                    title="Insert keyframe"
                >
                    <Icon data={IconData::LUCIDE_PLUS} width="12px" height="12px" />
                </button>
                if is_menu_open {
                    <div class="timeline-insert-menu">
                        <button class="timeline-insert-menu-item loop-start" onclick={make_insert(Keyframe::LoopStart { count: None })} title="Loop Start">
                            <Icon data={IconData::LUCIDE_REPEAT} width="12px" height="12px" />
                        </button>
                        <button class="timeline-insert-menu-item loop-end" onclick={make_insert(Keyframe::LoopEnd)} title="Loop End">
                            <Icon data={IconData::LUCIDE_CORNER_DOWN_LEFT} width="12px" height="12px" />
                        </button>
                        <button class="timeline-insert-menu-item delay" onclick={make_insert(Keyframe::Delay { duration: NumberOrExpr::Number(1.0) })} title="Delay">
                            <Icon data={IconData::LUCIDE_TIMER} width="12px" height="12px" />
                        </button>
                        <button class="timeline-insert-menu-item apply" onclick={make_insert(Keyframe::Apply {
                            translation: None,
                            rotation: None,
                            duration: 0.5,
                            easing: Default::default(),
                        })} title="Apply Transform">
                            <Icon data={IconData::LUCIDE_MOVE} width="12px" height="12px" />
                        </button>
                        <button class="timeline-insert-menu-item pivot" onclick={make_insert(Keyframe::PivotRotate {
                            pivot: [0.0, 0.0],
                            angle: 30.0,
                            duration: 0.5,
                            easing: Default::default(),
                        })} title="Pivot Rotate">
                            <Icon data={IconData::LUCIDE_ROTATE_CW} width="12px" height="12px" />
                        </button>
                    </div>
                }
            </div>
        }
    };

    html! {
        <div class="timeline-panel" onclick={close_insert_menu}>
            if let Some(seq) = &props.sequence {
                // Sequence header
                <div class="timeline-header">
                    <div class="timeline-sequence-info">
                        <span class="timeline-sequence-name">{&seq.name}</span>
                        <button
                            class="timeline-edit-targets-btn"
                            onclick={toggle_editing_targets.clone()}
                            title="Edit target objects"
                        >
                            {"T"}
                        </button>
                        <button
                            class={classes!("timeline-play-btn", props.is_previewing.then_some("playing"))}
                            onclick={{
                                let on_preview = props.on_preview_sequence.clone();
                                Callback::from(move |_: MouseEvent| {
                                    on_preview.emit(());
                                })
                            }}
                            disabled={props.is_previewing}
                            title="Play sequence"
                        >
                            <Icon data={IconData::LUCIDE_PLAY} width="14px" height="14px" />
                            if props.is_previewing {
                                {"Playing..."}
                            } else {
                                {"Play"}
                            }
                        </button>
                    </div>
                    // Targets display/edit
                    if *editing_targets {
                        <div class="timeline-targets-editor">
                            <div class="timeline-targets-list">
                                {for seq.target_ids.iter().enumerate().map(|(i, id)| {
                                    let on_update_sequence = props.on_update_sequence.clone();
                                    let sequence = props.sequence.clone();
                                    let on_remove = Callback::from(move |_: MouseEvent| {
                                        if let Some(mut seq) = sequence.clone() {
                                            seq.target_ids.remove(i);
                                            on_update_sequence.emit(seq);
                                        }
                                    });
                                    html! {
                                        <span class="timeline-target-tag">
                                            {id}
                                            <button class="timeline-target-remove" onclick={on_remove}>{"x"}</button>
                                        </span>
                                    }
                                })}
                            </div>
                            <div class="timeline-targets-add">
                                if !props.available_object_ids.is_empty() {
                                    <select class="timeline-target-select" onchange={on_target_select}>
                                        <option value="">{"+ Select..."}</option>
                                        {for props.available_object_ids.iter()
                                            .filter(|id| !seq.target_ids.contains(id))
                                            .map(|id| {
                                                html! { <option value={id.clone()}>{id}</option> }
                                            })
                                        }
                                    </select>
                                }
                                <input
                                    type="text"
                                    class="timeline-target-input"
                                    placeholder="Custom ID"
                                    value={(*new_target_input).clone()}
                                    oninput={on_target_input}
                                />
                                <button
                                    class="timeline-target-add-btn"
                                    onclick={on_add_target}
                                    disabled={new_target_input.is_empty()}
                                >
                                    {"+"}
                                </button>
                            </div>
                        </div>
                    } else {
                        <div class="timeline-targets-display">
                            {if seq.target_ids.is_empty() {
                                html! { <span class="timeline-no-targets">{"(no targets)"}</span> }
                            } else {
                                html! {
                                    <span class="timeline-targets-summary">
                                        {seq.target_ids.join(", ")}
                                    </span>
                                }
                            }}
                        </div>
                    }
                </div>

                // Keyframe track with inline add buttons
                <div class="timeline-track">
                    <div class="timeline-keyframes">
                        // Initial + button (insert at index 0)
                        {render_insert_btn(0)}

                        {for seq.keyframes.iter().enumerate().map(|(i, kf)| {
                            let on_select = props.on_select_keyframe.clone();
                            let on_delete = props.on_delete_keyframe.clone();
                            let on_move = props.on_move_keyframe.clone();
                            let is_selected = props.selected_keyframe == Some(i);
                            let is_dragging = *dragging_index == Some(i);
                            let is_drag_over = *drag_over_index == Some(i) && *dragging_index != Some(i);
                            let (type_class, icon_data, label, is_cel_expr) = keyframe_info(kf);

                            let on_click = {
                                let on_select = on_select.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    on_select.emit(Some(i));
                                })
                            };

                            let on_delete_click = {
                                let on_delete = on_delete.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    on_delete.emit(i);
                                })
                            };

                            // Drag handlers
                            let on_drag_start = {
                                let dragging_index = dragging_index.clone();
                                Callback::from(move |_: DragEvent| {
                                    dragging_index.set(Some(i));
                                })
                            };

                            let on_drag_over = {
                                let drag_over_index = drag_over_index.clone();
                                Callback::from(move |e: DragEvent| {
                                    e.prevent_default();
                                    drag_over_index.set(Some(i));
                                })
                            };

                            let on_drag_leave = {
                                let drag_over_index = drag_over_index.clone();
                                Callback::from(move |_: DragEvent| {
                                    drag_over_index.set(None);
                                })
                            };

                            let on_drop = {
                                let dragging_index = dragging_index.clone();
                                let drag_over_index = drag_over_index.clone();
                                let on_move = on_move.clone();
                                Callback::from(move |e: DragEvent| {
                                    e.prevent_default();
                                    if let Some(from_idx) = *dragging_index {
                                        if from_idx != i {
                                            on_move.emit((from_idx, i));
                                        }
                                    }
                                    dragging_index.set(None);
                                    drag_over_index.set(None);
                                })
                            };

                            let on_drag_end = {
                                let dragging_index = dragging_index.clone();
                                let drag_over_index = drag_over_index.clone();
                                Callback::from(move |_: DragEvent| {
                                    dragging_index.set(None);
                                    drag_over_index.set(None);
                                })
                            };

                            html! {
                                <>
                                    <div
                                        class={classes!(
                                            "timeline-keyframe",
                                            type_class,
                                            is_selected.then_some("selected"),
                                            is_dragging.then_some("dragging"),
                                            is_drag_over.then_some("drag-over")
                                        )}
                                        onclick={on_click}
                                        draggable="true"
                                        ondragstart={on_drag_start}
                                        ondragover={on_drag_over}
                                        ondragleave={on_drag_leave}
                                        ondrop={on_drop}
                                        ondragend={on_drag_end}
                                    >
                                        <Icon data={icon_data} width="12px" height="12px" class="timeline-keyframe-icon" />
                                        if is_cel_expr {
                                            // CEL expression: show "?" label to indicate variable duration
                                            <span class="timeline-keyframe-label">{"?"}</span>
                                            // S-curve wave tail on right edge
                                            <svg class="timeline-cel-wave" viewBox="0 0 4 100" preserveAspectRatio="none">
                                                <path d="M 0 0 Q 6 25 2 50 T 4 100 L 0 100 Z"/>
                                            </svg>
                                        } else if !label.is_empty() {
                                            <span class="timeline-keyframe-label">{label}</span>
                                        }
                                        <button
                                            class="timeline-keyframe-delete"
                                            onclick={on_delete_click}
                                            title="Delete"
                                        >
                                            <Icon data={IconData::LUCIDE_X} width="10px" height="10px" />
                                        </button>
                                    </div>
                                    // + button after each keyframe (insert at index i+1)
                                    {render_insert_btn(i + 1)}
                                </>
                            }
                        })}
                    </div>
                </div>

                // Selected keyframe editor
                if let Some(kf_idx) = props.selected_keyframe {
                    if let Some(keyframe) = seq.keyframes.get(kf_idx) {
                        <div class="timeline-keyframe-editor">
                            {render_keyframe_editor(
                                keyframe.clone(),
                                kf_idx,
                                &props.on_update_keyframe,
                                &props.on_preview_keyframe,
                                props.is_previewing,
                            )}
                        </div>
                    }
                }
            } else {
                <div class="timeline-empty">
                    {"Select a sequence from the Keyframes tab to edit"}
                </div>
            }
        </div>
    }
}

/// Renders the keyframe editor based on keyframe type.
fn render_keyframe_editor(
    keyframe: Keyframe,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
    on_preview: &Callback<usize>,
    is_previewing: bool,
) -> Html {
    match keyframe {
        Keyframe::LoopStart { count } => {
            render_loop_start_editor(count, kf_idx, on_update)
        }
        Keyframe::LoopEnd => {
            render_loop_end_editor()
        }
        Keyframe::Delay { duration } => {
            render_delay_editor(duration, kf_idx, on_update, on_preview, is_previewing)
        }
        Keyframe::Apply { translation, rotation, duration, easing } => {
            render_apply_editor(translation, rotation, duration, easing, kf_idx, on_update, on_preview, is_previewing)
        }
        Keyframe::PivotRotate { pivot, angle, duration, easing } => {
            render_pivot_editor(pivot, angle, duration, easing, kf_idx, on_update, on_preview, is_previewing)
        }
    }
}

/// LoopStart editor: count input (number or infinite)
fn render_loop_start_editor(count: Option<u32>, kf_idx: usize, on_update: &Callback<(usize, Keyframe)>) -> Html {
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
        <>
            <div class="timeline-editor-header">
                <Icon data={IconData::LUCIDE_REPEAT} width="14px" height="14px" />
                <span>{"Loop Start"}</span>
            </div>
            <div class="timeline-editor-content">
                <div class="editor-field">
                    <label>{"Count"}</label>
                    <input
                        type="text"
                        class="editor-input"
                        placeholder="∞ (infinite)"
                        value={count_str}
                        oninput={on_count_change}
                    />
                    <span class="editor-hint">{"Leave empty for infinite loop"}</span>
                </div>
            </div>
        </>
    }
}

/// LoopEnd editor: no editable fields
fn render_loop_end_editor() -> Html {
    html! {
        <>
            <div class="timeline-editor-header">
                <Icon data={IconData::LUCIDE_CORNER_DOWN_LEFT} width="14px" height="14px" />
                <span>{"Loop End"}</span>
            </div>
            <div class="timeline-editor-content">
                <div class="editor-info">
                    {"Marks the end of a loop block. No editable properties."}
                </div>
            </div>
        </>
    }
}

/// Delay editor: duration (number or CEL expression)
fn render_delay_editor(
    duration: NumberOrExpr,
    kf_idx: usize,
    on_update: &Callback<(usize, Keyframe)>,
    on_preview: &Callback<usize>,
    is_previewing: bool,
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

    let on_preview_click = {
        let on_preview = on_preview.clone();
        Callback::from(move |_: MouseEvent| {
            on_preview.emit(kf_idx);
        })
    };

    html! {
        <>
            <div class="timeline-editor-header">
                <Icon data={IconData::LUCIDE_TIMER} width="14px" height="14px" />
                <span>{"Delay"}</span>
                <button
                    class={classes!("timeline-preview-btn", is_previewing.then_some("playing"))}
                    onclick={on_preview_click}
                    disabled={is_previewing}
                    title="Preview this keyframe"
                >
                    <Icon data={IconData::LUCIDE_PLAY} width="12px" height="12px" />
                    {"Play"}
                </button>
            </div>
            <div class="timeline-editor-content">
                <div class="editor-field">
                    <label>{"Duration (seconds)"}</label>
                    <input
                        type="text"
                        class={classes!("editor-input", is_expr.then_some("cel-expr"))}
                        placeholder="e.g. 1.5 or random(1, 3)"
                        value={duration_str}
                        oninput={on_duration_change}
                    />
                    if is_expr {
                        <span class="editor-hint cel">{"CEL expression - duration varies at runtime"}</span>
                    } else {
                        <span class="editor-hint">{"Enter a number or CEL expression like random(1, 3)"}</span>
                    }
                </div>
            </div>
        </>
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
    on_preview: &Callback<usize>,
    is_previewing: bool,
) -> Html {
    let trans_x = translation.map(|t| t[0]).unwrap_or(0.0);
    let trans_y = translation.map(|t| t[1]).unwrap_or(0.0);
    let rot_deg = rotation.unwrap_or(0.0);

    // Translation X
    let on_trans_x_change = {
        let on_update = on_update.clone();
        let translation = translation;
        let rotation = rotation;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = input.value().parse::<f32>().unwrap_or(0.0);
                let y = translation.map(|t| t[1]).unwrap_or(0.0);
                let new_translation = if x == 0.0 && y == 0.0 { None } else { Some([x, y]) };
                on_update.emit((kf_idx, Keyframe::Apply {
                    translation: new_translation,
                    rotation,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Translation Y
    let on_trans_y_change = {
        let on_update = on_update.clone();
        let translation = translation;
        let rotation = rotation;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = translation.map(|t| t[0]).unwrap_or(0.0);
                let y = input.value().parse::<f32>().unwrap_or(0.0);
                let new_translation = if x == 0.0 && y == 0.0 { None } else { Some([x, y]) };
                on_update.emit((kf_idx, Keyframe::Apply {
                    translation: new_translation,
                    rotation,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Rotation
    let on_rotation_change = {
        let on_update = on_update.clone();
        let translation = translation;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let deg = input.value().parse::<f32>().unwrap_or(0.0);
                let new_rotation = if deg == 0.0 { None } else { Some(deg) };
                on_update.emit((kf_idx, Keyframe::Apply {
                    translation,
                    rotation: new_rotation,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Duration
    let on_duration_change = {
        let on_update = on_update.clone();
        let translation = translation;
        let rotation = rotation;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_duration = input.value().parse::<f32>().unwrap_or(0.5).max(0.01);
                on_update.emit((kf_idx, Keyframe::Apply {
                    translation,
                    rotation,
                    duration: new_duration,
                    easing,
                }));
            }
        })
    };

    // Easing
    let on_easing_change = {
        let on_update = on_update.clone();
        let translation = translation;
        let rotation = rotation;
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_easing = match select.value().as_str() {
                    "ease_in" => EasingType::EaseIn,
                    "ease_out" => EasingType::EaseOut,
                    "ease_in_out" => EasingType::EaseInOut,
                    _ => EasingType::Linear,
                };
                on_update.emit((kf_idx, Keyframe::Apply {
                    translation,
                    rotation,
                    duration,
                    easing: new_easing,
                }));
            }
        })
    };

    let on_preview_click = {
        let on_preview = on_preview.clone();
        Callback::from(move |_: MouseEvent| {
            on_preview.emit(kf_idx);
        })
    };

    html! {
        <>
            <div class="timeline-editor-header">
                <Icon data={IconData::LUCIDE_MOVE} width="14px" height="14px" />
                <span>{"Apply Transform"}</span>
                <button
                    class={classes!("timeline-preview-btn", is_previewing.then_some("playing"))}
                    onclick={on_preview_click}
                    disabled={is_previewing}
                    title="Preview this keyframe"
                >
                    <Icon data={IconData::LUCIDE_PLAY} width="12px" height="12px" />
                    {"Play"}
                </button>
            </div>
            <div class="timeline-editor-content">
                <div class="editor-row">
                    <div class="editor-field">
                        <label>{"Translation X"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={trans_x.to_string()}
                            oninput={on_trans_x_change}
                        />
                    </div>
                    <div class="editor-field">
                        <label>{"Translation Y"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={trans_y.to_string()}
                            oninput={on_trans_y_change}
                        />
                    </div>
                </div>
                <div class="editor-row">
                    <div class="editor-field">
                        <label>{"Rotation (°)"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={rot_deg.to_string()}
                            oninput={on_rotation_change}
                        />
                    </div>
                    <div class="editor-field">
                        <label>{"Duration (s)"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="0.1"
                            min="0.01"
                            value={duration.to_string()}
                            oninput={on_duration_change}
                        />
                    </div>
                </div>
                <div class="editor-field">
                    <label>{"Easing"}</label>
                    {render_easing_select(easing, on_easing_change)}
                </div>
            </div>
        </>
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
    on_preview: &Callback<usize>,
    is_previewing: bool,
) -> Html {
    // Pivot X
    let on_pivot_x_change = {
        let on_update = on_update.clone();
        let pivot = pivot;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let x = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((kf_idx, Keyframe::PivotRotate {
                    pivot: [x, pivot[1]],
                    angle,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Pivot Y
    let on_pivot_y_change = {
        let on_update = on_update.clone();
        let pivot = pivot;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let y = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((kf_idx, Keyframe::PivotRotate {
                    pivot: [pivot[0], y],
                    angle,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Angle
    let on_angle_change = {
        let on_update = on_update.clone();
        let pivot = pivot;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_angle = input.value().parse::<f32>().unwrap_or(0.0);
                on_update.emit((kf_idx, Keyframe::PivotRotate {
                    pivot,
                    angle: new_angle,
                    duration,
                    easing,
                }));
            }
        })
    };

    // Duration
    let on_duration_change = {
        let on_update = on_update.clone();
        let pivot = pivot;
        let easing = easing;
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let new_duration = input.value().parse::<f32>().unwrap_or(0.5).max(0.01);
                on_update.emit((kf_idx, Keyframe::PivotRotate {
                    pivot,
                    angle,
                    duration: new_duration,
                    easing,
                }));
            }
        })
    };

    // Easing
    let on_easing_change = {
        let on_update = on_update.clone();
        let pivot = pivot;
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let new_easing = match select.value().as_str() {
                    "ease_in" => EasingType::EaseIn,
                    "ease_out" => EasingType::EaseOut,
                    "ease_in_out" => EasingType::EaseInOut,
                    _ => EasingType::Linear,
                };
                on_update.emit((kf_idx, Keyframe::PivotRotate {
                    pivot,
                    angle,
                    duration,
                    easing: new_easing,
                }));
            }
        })
    };

    let on_preview_click = {
        let on_preview = on_preview.clone();
        Callback::from(move |_: MouseEvent| {
            on_preview.emit(kf_idx);
        })
    };

    html! {
        <>
            <div class="timeline-editor-header">
                <Icon data={IconData::LUCIDE_ROTATE_CW} width="14px" height="14px" />
                <span>{"Pivot Rotate"}</span>
                <button
                    class={classes!("timeline-preview-btn", is_previewing.then_some("playing"))}
                    onclick={on_preview_click}
                    disabled={is_previewing}
                    title="Preview this keyframe"
                >
                    <Icon data={IconData::LUCIDE_PLAY} width="12px" height="12px" />
                    {"Play"}
                </button>
            </div>
            <div class="timeline-editor-content">
                <div class="editor-row">
                    <div class="editor-field">
                        <label>{"Pivot X"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={pivot[0].to_string()}
                            oninput={on_pivot_x_change}
                        />
                    </div>
                    <div class="editor-field">
                        <label>{"Pivot Y"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={pivot[1].to_string()}
                            oninput={on_pivot_y_change}
                        />
                    </div>
                </div>
                <div class="editor-row">
                    <div class="editor-field">
                        <label>{"Angle (°)"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="1"
                            value={angle.to_string()}
                            oninput={on_angle_change}
                        />
                    </div>
                    <div class="editor-field">
                        <label>{"Duration (s)"}</label>
                        <input
                            type="number"
                            class="editor-input"
                            step="0.1"
                            min="0.01"
                            value={duration.to_string()}
                            oninput={on_duration_change}
                        />
                    </div>
                </div>
                <div class="editor-field">
                    <label>{"Easing"}</label>
                    {render_easing_select(easing, on_easing_change)}
                </div>
            </div>
        </>
    }
}

/// Renders an easing type selector
fn render_easing_select(current: EasingType, on_change: Callback<Event>) -> Html {
    let value = match current {
        EasingType::Linear => "linear",
        EasingType::EaseIn => "ease_in",
        EasingType::EaseOut => "ease_out",
        EasingType::EaseInOut => "ease_in_out",
    };

    html! {
        <select class="editor-select" onchange={on_change} value={value}>
            <option value="linear" selected={current == EasingType::Linear}>{"Linear"}</option>
            <option value="ease_in" selected={current == EasingType::EaseIn}>{"Ease In"}</option>
            <option value="ease_out" selected={current == EasingType::EaseOut}>{"Ease Out"}</option>
            <option value="ease_in_out" selected={current == EasingType::EaseInOut}>{"Ease In Out"}</option>
        </select>
    }
}
