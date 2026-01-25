//! Object list panel component.

use marble_core::map::{MapObject, ObjectRole, Shape};
use yew::prelude::*;

use crate::hooks::{create_default_obstacle, create_default_spawner, create_default_trigger};

/// Props for the ObjectList component.
#[derive(Properties, PartialEq)]
pub struct ObjectListProps {
    pub objects: Vec<MapObject>,
    pub selected_index: Option<usize>,
    pub on_select: Callback<Option<usize>>,
    pub on_add: Callback<MapObject>,
    pub on_delete: Callback<usize>,
}

/// Object list component.
#[function_component(ObjectList)]
pub fn object_list(props: &ObjectListProps) -> Html {
    let show_add_menu = use_state(|| false);

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

    html! {
        <div class="object-list">
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
                    };

                    let role_label = match obj.role {
                        ObjectRole::Spawner => "S",
                        ObjectRole::Obstacle => "O",
                        ObjectRole::Trigger => "T",
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
                        </div>
                    }
                </div>
            </div>
        </div>
    }
}
