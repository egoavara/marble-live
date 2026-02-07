//! MarbleEditor component - Bevy-based map editor view.
//!
//! Uses the global BevyProvider (from App.rs) for rendering.
//! UI panels (PropertyPanel, TimelinePanel, etc.) are still Yew components
//! that sync with Bevy state via polling.

use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;

use crate::app::BEVY_CANVAS_ID;
use crate::components::editor::{ContextMenu, ContextMenuState};
use crate::hooks::use_bevy;

/// Canvas ID for the editor (uses the global canvas from App.rs).
pub const EDITOR_CANVAS_ID: &str = BEVY_CANVAS_ID;

/// Props for MarbleEditor component.
#[derive(Properties, PartialEq)]
pub struct MarbleEditorProps {
    /// Children (UI panels).
    #[prop_or_default]
    pub children: Children,
    /// Whether there's content in the clipboard.
    #[prop_or(false)]
    pub has_clipboard: bool,
    /// Currently selected object index.
    #[prop_or(None)]
    pub selected_object: Option<usize>,
    /// Callback when copy is requested.
    #[prop_or_default]
    pub on_copy: Callback<usize>,
    /// Callback when paste is requested (world position).
    #[prop_or_default]
    pub on_paste: Callback<(f32, f32)>,
    /// Callback when delete is requested.
    #[prop_or_default]
    pub on_delete: Callback<usize>,
    /// Callback when mirror X is requested.
    #[prop_or_default]
    pub on_mirror_x: Callback<usize>,
    /// Callback when mirror Y is requested.
    #[prop_or_default]
    pub on_mirror_y: Callback<usize>,
}

/// MarbleEditor component - renders the map editor UI.
///
/// NOTE: The editor canvas is managed globally by App.rs to persist across
/// route changes and avoid Bevy's RecreationAttempt error in WASM.
#[function_component(MarbleEditor)]
pub fn marble_editor(props: &MarbleEditorProps) -> Html {
    // No local BevyProvider needed - uses the global one from App.rs
    html! {
        <MarbleEditorInner
            has_clipboard={props.has_clipboard}
            selected_object={props.selected_object}
            on_copy={props.on_copy.clone()}
            on_paste={props.on_paste.clone()}
            on_delete={props.on_delete.clone()}
            on_mirror_x={props.on_mirror_x.clone()}
            on_mirror_y={props.on_mirror_y.clone()}
        >
            { props.children.clone() }
        </MarbleEditorInner>
    }
}

/// Props for inner editor component.
#[derive(Properties, PartialEq)]
struct MarbleEditorInnerProps {
    children: Children,
    has_clipboard: bool,
    selected_object: Option<usize>,
    on_copy: Callback<usize>,
    on_paste: Callback<(f32, f32)>,
    on_delete: Callback<usize>,
    on_mirror_x: Callback<usize>,
    on_mirror_y: Callback<usize>,
}

/// Convert screen position to world position.
/// This is a simplified conversion - in a real implementation,
/// you'd need to account for camera position and zoom.
fn screen_to_world(screen_x: f32, screen_y: f32, canvas: &web_sys::HtmlCanvasElement) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    let canvas_width = rect.width() as f32;
    let canvas_height = rect.height() as f32;

    // Normalize to canvas-relative coordinates
    let rel_x = screen_x - rect.left() as f32;
    let rel_y = screen_y - rect.top() as f32;

    // Convert to world coordinates (assuming a 6x10 world view)
    // This matches the typical editor view dimensions
    let world_width = 6.0;
    let world_height = 10.0;

    let world_x = (rel_x / canvas_width) * world_width;
    let world_y = (rel_y / canvas_height) * world_height;

    (world_x, world_y)
}

/// Inner component that uses Bevy hooks.
#[function_component(MarbleEditorInner)]
fn marble_editor_inner(props: &MarbleEditorInnerProps) -> Html {
    let bevy = use_bevy();
    let context_menu_state = use_state(ContextMenuState::default);

    // Context menu close handler
    let on_close_context_menu = {
        let context_menu_state = context_menu_state.clone();
        Callback::from(move |_: ()| {
            context_menu_state.set(ContextMenuState::hide());
        })
    };

    // Context menu handlers - wrap the props callbacks
    let on_copy = props.on_copy.clone();
    let on_paste = props.on_paste.clone();
    let on_delete = props.on_delete.clone();
    let on_mirror_x = props.on_mirror_x.clone();
    let on_mirror_y = props.on_mirror_y.clone();

    // Bind contextmenu event to the global canvas via JS
    {
        let context_menu_state = context_menu_state.clone();
        let selected_object = props.selected_object;

        use_effect_with(selected_object, move |selected_object| {
            let selected_object = *selected_object;

            let listener = gloo::utils::document()
                .get_element_by_id(EDITOR_CANVAS_ID)
                .map(|canvas| {
                    gloo::events::EventListener::new(&canvas, "contextmenu", move |event| {
                        event.prevent_default();
                        let e: &MouseEvent = event.dyn_ref().unwrap();

                        let screen_x = e.client_x() as f32;
                        let screen_y = e.client_y() as f32;

                        let world_pos = if let Some(canvas) = gloo::utils::document()
                            .get_element_by_id(EDITOR_CANVAS_ID)
                            .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
                        {
                            screen_to_world(screen_x, screen_y, &canvas)
                        } else {
                            (3.0, 5.0)
                        };

                        context_menu_state.set(ContextMenuState::show(
                            (screen_x, screen_y),
                            world_pos,
                            selected_object,
                        ));
                    })
                });

            move || drop(listener)
        });
    }

    // Bind click event to close context menu
    {
        let context_menu_state = context_menu_state.clone();

        use_effect_with((), move |_| {
            let listener = gloo::utils::document()
                .get_element_by_id(EDITOR_CANVAS_ID)
                .map(|canvas| {
                    gloo::events::EventListener::new(&canvas, "click", move |_event| {
                        if context_menu_state.visible {
                            context_menu_state.set(ContextMenuState::hide());
                        }
                    })
                });

            move || drop(listener)
        });
    }

    html! {
        <div class="marble-editor">
            // NOTE: Canvas is now managed globally by App.rs
            // Loading overlay
            if !bevy.initialized {
                <div class="marble-editor__loading">
                    <div class="marble-editor__spinner"></div>
                    <p>{"Initializing editor..."}</p>
                </div>
            }

            // Context menu
            <ContextMenu
                state={(*context_menu_state).clone()}
                has_clipboard={props.has_clipboard}
                on_close={on_close_context_menu}
                on_copy={on_copy}
                on_paste={on_paste}
                on_delete={on_delete}
                on_mirror_x={on_mirror_x}
                on_mirror_y={on_mirror_y}
            />

            // UI panels (PropertyPanel, TimelinePanel, etc.)
            { props.children.clone() }
        </div>
    }
}
