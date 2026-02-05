//! Object list panel component with tabbed interface.

use marble_core::map::{KeyframeSequence, MapObject, ObjectRole, Shape};
use yew::prelude::*;

use super::sequence_list::SequenceList;
use crate::hooks::{create_default_guideline, create_default_obstacle, create_default_spawner, create_default_trigger, create_default_vector_field};

/// Tab selection for the object list panel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ObjectListTab {
    Objects,
    Keyframes,
}

/// Props for the ObjectList component.
#[derive(Properties, PartialEq)]
pub struct ObjectListProps {
    pub objects: Vec<MapObject>,
    pub selected_index: Option<usize>,
    pub on_select: Callback<Option<usize>>,
    pub on_add: Callback<MapObject>,
    pub on_delete: Callback<usize>,
    // Sequence props
    #[prop_or_default]
    pub sequences: Vec<KeyframeSequence>,
    #[prop_or_default]
    pub selected_sequence: Option<usize>,
    #[prop_or_default]
    pub on_select_sequence: Callback<Option<usize>>,
    #[prop_or_default]
    pub on_add_sequence: Callback<KeyframeSequence>,
    #[prop_or_default]
    pub on_delete_sequence: Callback<usize>,
    #[prop_or_default]
    pub on_update_sequence: Callback<(usize, KeyframeSequence)>,
}

/// Object list component with tabbed interface.
#[function_component(ObjectList)]
pub fn object_list(props: &ObjectListProps) -> Html {
    let current_tab = use_state(|| ObjectListTab::Objects);
    let show_add_menu = use_state(|| false);

    let on_objects_tab = {
        let current_tab = current_tab.clone();
        Callback::from(move |_: MouseEvent| {
            current_tab.set(ObjectListTab::Objects);
        })
    };

    let on_keyframes_tab = {
        let current_tab = current_tab.clone();
        Callback::from(move |_: MouseEvent| {
            current_tab.set(ObjectListTab::Keyframes);
        })
    };

    let toggle_add_menu = {
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            show_add_menu.set(!*show_add_menu);
        })
    };

    let add_obstacle = {
        let on_add = props.on_add.clone();
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            on_add.emit(create_default_obstacle());
            show_add_menu.set(false);
        })
    };

    let add_spawner = {
        let on_add = props.on_add.clone();
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            on_add.emit(create_default_spawner());
            show_add_menu.set(false);
        })
    };

    let add_trigger = {
        let on_add = props.on_add.clone();
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            on_add.emit(create_default_trigger());
            show_add_menu.set(false);
        })
    };

    let add_guideline = {
        let on_add = props.on_add.clone();
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            on_add.emit(create_default_guideline());
            show_add_menu.set(false);
        })
    };

    let add_vector_field = {
        let on_add = props.on_add.clone();
        let show_add_menu = show_add_menu.clone();
        Callback::from(move |_: MouseEvent| {
            on_add.emit(create_default_vector_field());
            show_add_menu.set(false);
        })
    };

    html! {
        <div class="object-list">
            // Tab header
            <div class="object-list-tabs">
                <button
                    class={classes!("tab-btn", (*current_tab == ObjectListTab::Objects).then_some("active"))}
                    onclick={on_objects_tab}
                >
                    {"Objects"}
                </button>
                <button
                    class={classes!("tab-btn", (*current_tab == ObjectListTab::Keyframes).then_some("active"))}
                    onclick={on_keyframes_tab}
                >
                    {"Keyframes"}
                </button>
            </div>

            // Tab content
            if *current_tab == ObjectListTab::Objects {
                // Objects tab content
                <div class="object-list-header">
                    <span class="object-list-title">{"Objects"}</span>
                    <span class="object-list-count">{format!("({})", props.objects.len())}</span>
                </div>
                <div class="object-list-items">
                {for props.objects.iter().enumerate().map(|(i, obj)| {
                    let on_select = props.on_select.clone();
                    let on_delete = props.on_delete.clone();
                    let is_selected = props.selected_index == Some(i);

                    let on_item_click = {
                        let on_select = on_select.clone();
                        Callback::from(move |_: MouseEvent| {
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

                    let role_class = match obj.role {
                        ObjectRole::Spawner => "role-spawner",
                        ObjectRole::Obstacle => "role-obstacle",
                        ObjectRole::Trigger => "role-trigger",
                        ObjectRole::Guideline => "role-guideline",
                        ObjectRole::VectorField => "role-vector-field",
                    };

                    let role_label = match obj.role {
                        ObjectRole::Spawner => "S",
                        ObjectRole::Obstacle => "O",
                        ObjectRole::Trigger => "T",
                        ObjectRole::Guideline => "G",
                        ObjectRole::VectorField => "V",
                    };

                    let shape_label = match &obj.shape {
                        Shape::Line { .. } => "Line",
                        Shape::Circle { .. } => "Circle",
                        Shape::Rect { .. } => "Rect",
                        Shape::Bezier { .. } => "Bezier",
                    };

                    let name = obj.id.clone().unwrap_or_else(|| format!("{} {}", shape_label, i));

                    html! {
                        <div
                            class={classes!("object-list-item", is_selected.then_some("selected"))}
                            onclick={on_item_click}
                        >
                            <span class={classes!("object-role-badge", role_class)}>{role_label}</span>
                            <span class="object-name">{name}</span>
                            <button
                                class="object-delete-btn"
                                onclick={on_delete_click}
                                title="Delete"
                            >
                                {"x"}
                            </button>
                        </div>
                    }
                })}
                if props.objects.is_empty() {
                    <div class="object-list-empty">
                        {"No objects. Click + to add."}
                    </div>
                }
            </div>
            <div class="object-list-footer">
                <div class="add-button-container">
                    <button class="add-btn" onclick={toggle_add_menu}>
                        {"+ Add Object"}
                    </button>
                    if *show_add_menu {
                        <div class="add-menu">
                            <button class="add-menu-item" onclick={add_obstacle}>
                                <span class="object-role-badge role-obstacle">{"O"}</span>
                                {"Obstacle"}
                            </button>
                            <button class="add-menu-item" onclick={add_spawner}>
                                <span class="object-role-badge role-spawner">{"S"}</span>
                                {"Spawner"}
                            </button>
                            <button class="add-menu-item" onclick={add_trigger}>
                                <span class="object-role-badge role-trigger">{"T"}</span>
                                {"Trigger"}
                            </button>
                            <button class="add-menu-item" onclick={add_guideline}>
                                <span class="object-role-badge role-guideline">{"G"}</span>
                                {"Guideline"}
                            </button>
                            <button class="add-menu-item" onclick={add_vector_field}>
                                <span class="object-role-badge role-vector-field">{"V"}</span>
                                {"Vector Field"}
                            </button>
                        </div>
                    }
                </div>
            </div>
            } else {
                // Keyframes tab content
                <SequenceList
                    sequences={props.sequences.clone()}
                    selected_index={props.selected_sequence}
                    on_select={props.on_select_sequence.clone()}
                    on_add={props.on_add_sequence.clone()}
                    on_delete={props.on_delete_sequence.clone()}
                    on_update={props.on_update_sequence.clone()}
                />
            }
        </div>
    }
}
