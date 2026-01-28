//! Timeline panel component for keyframe animation editing.

use marble_core::dsl::NumberOrExpr;
use marble_core::map::{Keyframe, KeyframeSequence};
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew::{create_portal, Html};
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
    /// Current keyframe index being executed during preview
    #[prop_or_default]
    pub preview_keyframe_index: Option<usize>,
}

/// Timeline panel component for editing keyframe sequences.
#[function_component(TimelinePanel)]
pub fn timeline_panel(props: &TimelinePanelProps) -> Html {
    let editing_targets = use_state(|| false);
    let new_target_input = use_state(|| String::new());
    // (index, x, y) where to insert new keyframe and show menu (None = no menu shown)
    let insert_menu: UseStateHandle<Option<(usize, f64, f64)>> = use_state(|| None);
    // Drag state for reordering keyframes
    let dragging_index: UseStateHandle<Option<usize>> = use_state(|| None);
    let drag_over_index: UseStateHandle<Option<usize>> = use_state(|| None);

    // Portal host element in document.body for the insert menu
    // (bypasses all ancestor overflow/transform clipping)
    let portal_host: UseStateHandle<Option<web_sys::Element>> = use_state(|| None);
    {
        let portal_host = portal_host.clone();
        use_effect_with((), move |_| {
            let doc = web_sys::window().unwrap().document().unwrap();
            let el = doc.create_element("div").unwrap();
            el.set_attribute("class", "timeline-insert-menu-portal").ok();
            doc.body().unwrap().append_child(&el).ok();
            portal_host.set(Some(el.clone()));
            move || {
                el.remove();
            }
        });
    }

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
        let insert_menu = insert_menu.clone();
        Callback::from(move |_: MouseEvent| {
            web_sys::console::log_1(&format!("[insert-menu] close_insert_menu fired, current state: {:?}", *insert_menu).into());
            insert_menu.set(None);
        })
    };

    // Helper to create insert callback for fixed menu
    let make_insert_at_menu = |kf: Keyframe| {
        let on_update_sequence = props.on_update_sequence.clone();
        let sequence = props.sequence.clone();
        let insert_menu = insert_menu.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            if let Some((idx, _, _)) = *insert_menu {
                if let Some(mut seq) = sequence.clone() {
                    seq.keyframes.insert(idx, kf.clone());
                    on_update_sequence.emit(seq);
                }
            }
            insert_menu.set(None);
        })
    };

    // Helper to render insert button (menu is rendered separately at panel level)
    let render_insert_btn = |idx: usize| {
        let is_menu_open = (*insert_menu).map(|(i, _, _)| i) == Some(idx);
        let insert_menu = insert_menu.clone();

        let on_plus_click = {
            let insert_menu = insert_menu.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                if let Some(target) = e.target() {
                    if let Some(el) = target.dyn_ref::<web_sys::Element>() {
                        let btn = el.closest(".timeline-insert-btn").ok().flatten().unwrap_or_else(|| el.clone());
                        let rect = btn.get_bounding_client_rect();
                        // Viewport-relative coordinates (portal renders in document.body)
                        let x = rect.left() + rect.width() / 2.0;
                        let y = rect.top() - 4.0;
                        insert_menu.set(Some((idx, x, y)));
                    }
                }
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
            </div>
        }
    };

    // Render the insert menu via portal into document.body
    let portal_menu = if let (Some(host), Some((_, menu_x, menu_y))) = ((*portal_host).clone(), *insert_menu) {
        create_portal(
            html! {
                <div
                    class="timeline-insert-menu-fixed"
                    style={format!("left: {}px; top: {}px;", menu_x, menu_y)}
                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                >
                    <button class="timeline-insert-menu-item loop-start" onclick={make_insert_at_menu(Keyframe::LoopStart { count: None })} title="Loop Start">
                        <Icon data={IconData::LUCIDE_REPEAT} width="12px" height="12px" />
                    </button>
                    <button class="timeline-insert-menu-item loop-end" onclick={make_insert_at_menu(Keyframe::LoopEnd)} title="Loop End">
                        <Icon data={IconData::LUCIDE_CORNER_DOWN_LEFT} width="12px" height="12px" />
                    </button>
                    <button class="timeline-insert-menu-item delay" onclick={make_insert_at_menu(Keyframe::Delay { duration: NumberOrExpr::Number(1.0) })} title="Delay">
                        <Icon data={IconData::LUCIDE_TIMER} width="12px" height="12px" />
                    </button>
                    <button class="timeline-insert-menu-item apply" onclick={make_insert_at_menu(Keyframe::Apply {
                        translation: None,
                        rotation: None,
                        duration: 0.5,
                        easing: Default::default(),
                    })} title="Apply Transform">
                        <Icon data={IconData::LUCIDE_MOVE} width="12px" height="12px" />
                    </button>
                    <button class="timeline-insert-menu-item pivot" onclick={make_insert_at_menu(Keyframe::PivotRotate {
                        pivot: [0.0, 0.0],
                        angle: 30.0,
                        duration: 0.5,
                        easing: Default::default(),
                    })} title="Pivot Rotate">
                        <Icon data={IconData::LUCIDE_ROTATE_CW} width="12px" height="12px" />
                    </button>
                </div>
            },
            host.into(),
        )
    } else {
        Html::default()
    };

    html! {
        <>
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
                            title={if props.is_previewing { "Stop preview" } else { "Play sequence" }}
                        >
                            if props.is_previewing {
                                <Icon data={IconData::LUCIDE_SQUARE} width="14px" height="14px" />
                                {"Stop"}
                            } else {
                                <Icon data={IconData::LUCIDE_PLAY} width="14px" height="14px" />
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
                            // Highlight currently playing keyframe during preview
                            // current_index points to the next keyframe to process, so current - 1 is being executed
                            let is_playing = props.preview_keyframe_index.map(|idx| idx > 0 && idx - 1 == i).unwrap_or(false);
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
                                            is_drag_over.then_some("drag-over"),
                                            is_playing.then_some("playing")
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

            } else {
                <div class="timeline-empty">
                    {"Select a sequence from the Keyframes tab"}
                </div>
            }
        </div>
        {portal_menu}
        </>
    }
}
