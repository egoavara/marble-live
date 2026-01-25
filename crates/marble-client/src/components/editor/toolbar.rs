//! Editor toolbar component with meatball-style buttons.

use gloo::file::callbacks::FileReader;
use marble_core::map::RouletteConfig;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, Url};
use yew::prelude::*;
use yew_icons::{Icon, IconData};

/// Props for the EditorToolbar component.
#[derive(Properties, PartialEq)]
pub struct EditorToolbarProps {
    pub config: RouletteConfig,
    pub is_dirty: bool,
    pub on_new: Callback<()>,
    pub on_load: Callback<RouletteConfig>,
    pub on_save: Callback<()>,
    pub show_object_list: bool,
    pub show_property_panel: bool,
    pub on_toggle_object_list: Callback<()>,
    pub on_toggle_property_panel: Callback<()>,
}

/// Editor toolbar with meatball-style buttons.
#[function_component(EditorToolbar)]
pub fn editor_toolbar(props: &EditorToolbarProps) -> Html {
    let file_reader = use_state(|| None::<FileReader>);
    let file_input_ref = use_node_ref();

    let on_new_click = {
        let on_new = props.on_new.clone();
        let is_dirty = props.is_dirty;
        Callback::from(move |_: MouseEvent| {
            if is_dirty {
                let confirmed = web_sys::window()
                    .and_then(|w| w.confirm_with_message("Unsaved changes will be lost. Continue?").ok())
                    .unwrap_or(false);
                if !confirmed {
                    return;
                }
            }
            on_new.emit(());
        })
    };

    let on_load_default_click = {
        let on_load = props.on_load.clone();
        let is_dirty = props.is_dirty;
        Callback::from(move |_: MouseEvent| {
            if is_dirty {
                let confirmed = web_sys::window()
                    .and_then(|w| w.confirm_with_message("Unsaved changes will be lost. Continue?").ok())
                    .unwrap_or(false);
                if !confirmed {
                    return;
                }
            }
            on_load.emit(RouletteConfig::default_classic());
        })
    };

    let on_export_click = {
        let config = props.config.clone();
        let on_save = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if let Ok(json) = config.to_json() {
                let mut blob_options = web_sys::BlobPropertyBag::new();
                blob_options.set_type("application/json");
                let blob = web_sys::Blob::new_with_str_sequence_and_options(
                    &js_sys::Array::of1(&json.into()),
                    &blob_options,
                )
                .ok();

                if let Some(blob) = blob {
                    if let Ok(url) = Url::create_object_url_with_blob(&blob) {
                        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                            if let Ok(a) = document.create_element("a") {
                                let _ = a.set_attribute("href", &url);
                                let _ = a.set_attribute("download", "map.json");
                                if let Some(a) = a.dyn_ref::<web_sys::HtmlElement>() {
                                    a.click();
                                }
                                let _ = Url::revoke_object_url(&url);
                            }
                        }
                    }
                }
                on_save.emit(());
            }
        })
    };

    let on_import_click = {
        let file_input_ref = file_input_ref.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                input.click();
            }
        })
    };

    let on_file_change = {
        let on_load = props.on_load.clone();
        let file_reader = file_reader.clone();
        let is_dirty = props.is_dirty;
        Callback::from(move |e: Event| {
            if is_dirty {
                let confirmed = web_sys::window()
                    .and_then(|w| w.confirm_with_message("Unsaved changes will be lost. Continue?").ok())
                    .unwrap_or(false);
                if !confirmed {
                    return;
                }
            }

            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    let on_load = on_load.clone();
                    let file_reader_setter = file_reader.clone();
                    let reader = gloo::file::callbacks::read_as_text(&file.into(), move |result| {
                        if let Ok(text) = result {
                            if let Ok(config) = RouletteConfig::from_json(&text) {
                                on_load.emit(config);
                            } else {
                                let _ = web_sys::window()
                                    .and_then(|w| w.alert_with_message("Invalid JSON format").ok());
                            }
                        }
                        file_reader_setter.set(None);
                    });
                    file_reader.set(Some(reader));
                }
            }
            input.set_value("");
        })
    };

    let on_toggle_objects = {
        let on_toggle = props.on_toggle_object_list.clone();
        Callback::from(move |_: MouseEvent| {
            on_toggle.emit(());
        })
    };

    let on_toggle_properties = {
        let on_toggle = props.on_toggle_property_panel.clone();
        Callback::from(move |_: MouseEvent| {
            on_toggle.emit(());
        })
    };

    html! {
        <div class="editor-toolbar-floating">
            // Left group: File operations
            <div class="editor-toolbar-group">
                <button
                    class="editor-meatball-btn"
                    onclick={on_new_click}
                    title="New Map"
                >
                    <Icon data={IconData::LUCIDE_FILE_PLUS} width="18px" height="18px" />
                </button>
                <button
                    class="editor-meatball-btn"
                    onclick={on_load_default_click}
                    title="Load Default Map"
                >
                    <Icon data={IconData::LUCIDE_FILE_CODE} width="18px" height="18px" />
                </button>
                <button
                    class="editor-meatball-btn"
                    onclick={on_import_click}
                    title="Import JSON"
                >
                    <Icon data={IconData::LUCIDE_FOLDER_OPEN} width="18px" height="18px" />
                </button>
                <button
                    class="editor-meatball-btn editor-meatball-btn-primary"
                    onclick={on_export_click}
                    title="Export JSON"
                >
                    <Icon data={IconData::LUCIDE_DOWNLOAD} width="18px" height="18px" />
                </button>
            </div>

            // Center: Map info
            <div class="editor-toolbar-info">
                <span class="editor-toolbar-title">{&props.config.meta.name}</span>
                if props.is_dirty {
                    <span class="editor-toolbar-dirty">{"*"}</span>
                }
                <span class="editor-toolbar-count">
                    {format!("({} objects)", props.config.objects.len())}
                </span>
            </div>

            // Right group: Panel toggles
            <div class="editor-toolbar-group">
                <button
                    class={classes!(
                        "editor-meatball-btn",
                        props.show_object_list.then_some("active")
                    )}
                    onclick={on_toggle_objects}
                    title="Toggle Object List"
                >
                    <Icon data={IconData::LUCIDE_LIST} width="18px" height="18px" />
                </button>
                <button
                    class={classes!(
                        "editor-meatball-btn",
                        props.show_property_panel.then_some("active")
                    )}
                    onclick={on_toggle_properties}
                    title="Toggle Properties"
                >
                    <Icon data={IconData::LUCIDE_SETTINGS_2} width="18px" height="18px" />
                </button>
            </div>

            <input
                ref={file_input_ref}
                type="file"
                accept=".json"
                style="display: none"
                onchange={on_file_change}
            />
        </div>
    }
}
