//! Map Editor page.

use yew::prelude::*;

use crate::components::editor::{EditorCanvas, ObjectList, PropertyPanel, EditorToolbar};
use crate::components::Layout;
use crate::hooks::use_editor_state;

/// Map Editor page component.
#[function_component(EditorPage)]
pub fn editor_page() -> Html {
    let editor_state = use_editor_state();

    // Panel visibility states
    let show_object_list = use_state(|| true);
    let show_property_panel = use_state(|| true);

    let toggle_object_list = {
        let show_object_list = show_object_list.clone();
        Callback::from(move |_| {
            show_object_list.set(!*show_object_list);
        })
    };

    let toggle_property_panel = {
        let show_property_panel = show_property_panel.clone();
        Callback::from(move |_| {
            show_property_panel.set(!*show_property_panel);
        })
    };

    html! {
        <Layout show_settings={false}>
            <div class="editor-fullscreen">
                // Full-screen canvas with Blender-style unified gizmo
                <EditorCanvas
                    config={editor_state.config.clone()}
                    selected_index={editor_state.selected_object}
                    on_select={editor_state.on_select.clone()}
                    on_object_update={editor_state.on_update_object.clone()}
                />

                // Toolbar (top-center)
                <EditorToolbar
                    config={editor_state.config.clone()}
                    is_dirty={editor_state.is_dirty}
                    on_new={editor_state.on_new.clone()}
                    on_load={editor_state.on_load.clone()}
                    on_save={editor_state.on_save.clone()}
                    show_object_list={*show_object_list}
                    show_property_panel={*show_property_panel}
                    on_toggle_object_list={toggle_object_list}
                    on_toggle_property_panel={toggle_property_panel}
                />

                // Floating Object List (left side)
                if *show_object_list {
                    <div class="editor-floating-panel editor-panel-left">
                        <ObjectList
                            objects={editor_state.config.objects.clone()}
                            selected_index={editor_state.selected_object}
                            on_select={editor_state.on_select.clone()}
                            on_add={editor_state.on_add.clone()}
                            on_delete={editor_state.on_delete.clone()}
                        />
                    </div>
                }

                // Floating Property Panel (right side)
                if *show_property_panel {
                    <div class="editor-floating-panel editor-panel-right">
                        <PropertyPanel
                            config={editor_state.config.clone()}
                            selected_index={editor_state.selected_object}
                            on_update_meta={editor_state.on_update_meta.clone()}
                            on_update_object={editor_state.on_update_object.clone()}
                        />
                    </div>
                }
            </div>
        </Layout>
    }
}
