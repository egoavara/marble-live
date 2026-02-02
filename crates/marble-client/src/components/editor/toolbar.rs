//! Editor toolbar component with meatball-style buttons.

use gloo::file::callbacks::FileReader;
use marble_core::map::RouletteConfig;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, Url};
use yew::prelude::*;
use yew_icons::{Icon, IconData};

use crate::hooks::{send_command, SnapConfigSummary};

/// Props for the EditorToolbar component.
#[derive(Properties, PartialEq)]
pub struct EditorToolbarProps {
    pub config: RouletteConfig,
    pub is_dirty: bool,
    pub on_new: Callback<()>,
    pub on_load: Callback<RouletteConfig>,
    pub on_save: Callback<()>,
    // Simulation controls
    pub is_simulating: bool,
    pub on_toggle_simulation: Callback<()>,
    pub spawn_count: u32,
    pub on_spawn_count_change: Callback<u32>,
    pub on_spawn: Callback<()>,
    pub on_reset: Callback<()>,
    // Snap configuration
    pub snap_config: SnapConfigSummary,
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
                    .and_then(|w| w.confirm_with_message("Unsaved changes will be lost.").ok())
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

    // Simulation control callbacks
    let show_spawn_input = use_state(|| false);

    // Snap settings state
    let show_snap_settings = use_state(|| false);

    let on_snap_hover_enter = {
        let show = show_snap_settings.clone();
        Callback::from(move |_: MouseEvent| {
            show.set(true);
        })
    };

    let on_snap_hover_leave = {
        let show = show_snap_settings.clone();
        Callback::from(move |_: MouseEvent| {
            show.set(false);
        })
    };

    let on_grid_snap_toggle = {
        let current = props.snap_config.grid_snap_enabled;
        Callback::from(move |_: Event| {
            let cmd = serde_json::json!({
                "type": "update_snap_config",
                "grid_snap_enabled": !current
            });
            let _ = send_command(&cmd.to_string());
        })
    };

    let on_grid_interval_change = {
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f32>() {
                    let clamped = value.max(0.01).min(10.0);
                    let cmd = serde_json::json!({
                        "type": "update_snap_config",
                        "grid_snap_interval": clamped
                    });
                    let _ = send_command(&cmd.to_string());
                }
            }
        })
    };

    let on_angle_snap_toggle = {
        let current = props.snap_config.angle_snap_enabled;
        Callback::from(move |_: Event| {
            let cmd = serde_json::json!({
                "type": "update_snap_config",
                "angle_snap_enabled": !current
            });
            let _ = send_command(&cmd.to_string());
        })
    };

    // Grid interval - number input
    let on_grid_interval_change = {
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f32>() {
                    let clamped = value.max(0.01).min(10.0);
                    let cmd = serde_json::json!({
                        "type": "update_snap_config",
                        "grid_snap_interval": clamped
                    });
                    let _ = send_command(&cmd.to_string());
                }
            }
        })
    };

    // Angle interval - number input
    let on_angle_interval_change = {
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f32>() {
                    let clamped = value.max(0.1).min(90.0);
                    let cmd = serde_json::json!({
                        "type": "update_snap_config",
                        "angle_snap_interval": clamped
                    });
                    let _ = send_command(&cmd.to_string());
                }
            }
        })
    };

    // Grid interval option buttons
    let make_grid_option_callback = |value: f32| {
        Callback::from(move |_: MouseEvent| {
            let cmd = serde_json::json!({
                "type": "update_snap_config",
                "grid_snap_interval": value
            });
            let _ = send_command(&cmd.to_string());
        })
    };

    // Angle interval option buttons
    let make_angle_option_callback = |value: f32| {
        Callback::from(move |_: MouseEvent| {
            let cmd = serde_json::json!({
                "type": "update_snap_config",
                "angle_snap_interval": value
            });
            let _ = send_command(&cmd.to_string());
        })
    };

    let on_play_pause_click = {
        let on_toggle = props.on_toggle_simulation.clone();
        Callback::from(move |_: MouseEvent| {
            on_toggle.emit(());
        })
    };

    let on_spawn_click = {
        let on_spawn = props.on_spawn.clone();
        Callback::from(move |_: MouseEvent| {
            on_spawn.emit(());
        })
    };

    let on_reset_click = {
        let on_reset = props.on_reset.clone();
        Callback::from(move |_: MouseEvent| {
            on_reset.emit(());
        })
    };

    let on_spawn_count_input = {
        let on_change = props.on_spawn_count_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(count) = input.value().parse::<u32>() {
                    on_change.emit(count.max(1).min(100));
                }
            }
        })
    };

    let on_spawn_hover_enter = {
        let show = show_spawn_input.clone();
        Callback::from(move |_: MouseEvent| {
            show.set(true);
        })
    };

    let on_spawn_hover_leave = {
        let show = show_spawn_input.clone();
        Callback::from(move |_: MouseEvent| {
            show.set(false);
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

            // Right group: Snap settings + Simulation controls
            <div class="editor-toolbar-group">
                // Snap settings dropdown
                <div
                    class="editor-snap-container"
                    onmouseenter={on_snap_hover_enter}
                    onmouseleave={on_snap_hover_leave}
                >
                    <button
                        class={classes!(
                            "editor-meatball-btn",
                            (props.snap_config.grid_snap_enabled || props.snap_config.angle_snap_enabled).then_some("active")
                        )}
                        title="Snap Settings"
                    >
                        <Icon data={IconData::LUCIDE_MAGNET} width="18px" height="18px" />
                        <span class="editor-snap-arrow">{"▼"}</span>
                    </button>
                    if *show_snap_settings {
                        <div class="editor-snap-dropdown">
                            // Grid snap section
                            <div class="editor-snap-section">
                                <div class="editor-snap-header">
                                    <label class="editor-snap-label">
                                        <input
                                            type="checkbox"
                                            checked={props.snap_config.grid_snap_enabled}
                                            onchange={on_grid_snap_toggle}
                                        />
                                        {"Grid"}
                                    </label>
                                    <input
                                        type="number"
                                        class="editor-snap-input"
                                        value={format!("{:.2}", props.snap_config.grid_snap_interval)}
                                        step="0.01"
                                        min="0.01"
                                        max="10"
                                        oninput={on_grid_interval_change}
                                        disabled={!props.snap_config.grid_snap_enabled}
                                    />
                                </div>
                                <div class="editor-snap-options">
                                    {
                                        [0.05, 0.1, 0.5, 1.0].iter().map(|&val| {
                                            let is_selected = (props.snap_config.grid_snap_interval - val).abs() < 0.001;
                                            let disabled = !props.snap_config.grid_snap_enabled;
                                            html! {
                                                <button
                                                    class={classes!(
                                                        "editor-snap-option",
                                                        is_selected.then_some("selected"),
                                                        disabled.then_some("disabled")
                                                    )}
                                                    onclick={make_grid_option_callback(val)}
                                                    disabled={disabled}
                                                >
                                                    {if val < 1.0 { format!("{}", val) } else { "1".to_string() }}
                                                </button>
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            </div>

                            // Angle snap section
                            <div class="editor-snap-section">
                                <div class="editor-snap-header">
                                    <label class="editor-snap-label">
                                        <input
                                            type="checkbox"
                                            checked={props.snap_config.angle_snap_enabled}
                                            onchange={on_angle_snap_toggle}
                                        />
                                        {"Angle"}
                                    </label>
                                    <div class="editor-snap-input-group">
                                        <input
                                            type="number"
                                            class="editor-snap-input"
                                            value={format!("{:.1}", props.snap_config.angle_snap_interval)}
                                            step="0.5"
                                            min="0.1"
                                            max="90"
                                            oninput={on_angle_interval_change}
                                            disabled={!props.snap_config.angle_snap_enabled}
                                        />
                                        <span class="editor-snap-unit">{"°"}</span>
                                    </div>
                                </div>
                                <div class="editor-snap-options">
                                    {
                                        [0.5, 1.0, 5.0, 10.0].iter().map(|&val| {
                                            let is_selected = (props.snap_config.angle_snap_interval - val).abs() < 0.001;
                                            let disabled = !props.snap_config.angle_snap_enabled;
                                            html! {
                                                <button
                                                    class={classes!(
                                                        "editor-snap-option",
                                                        is_selected.then_some("selected"),
                                                        disabled.then_some("disabled")
                                                    )}
                                                    onclick={make_angle_option_callback(val)}
                                                    disabled={disabled}
                                                >
                                                    {if val < 1.0 { format!("{}°", val) } else { format!("{}°", val as i32) }}
                                                </button>
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            </div>
                        </div>
                    }
                </div>

                <button
                    class={classes!(
                        "editor-meatball-btn",
                        props.is_simulating.then_some("active")
                    )}
                    onclick={on_play_pause_click}
                    title={if props.is_simulating { "Pause Simulation" } else { "Play Simulation" }}
                >
                    if props.is_simulating {
                        <Icon data={IconData::LUCIDE_PAUSE} width="18px" height="18px" />
                    } else {
                        <Icon data={IconData::LUCIDE_PLAY} width="18px" height="18px" />
                    }
                </button>
                <div
                    class="editor-spawn-container"
                    onmouseenter={on_spawn_hover_enter}
                    onmouseleave={on_spawn_hover_leave}
                >
                    <button
                        class={classes!(
                            "editor-meatball-btn",
                            (!props.is_simulating).then_some("disabled")
                        )}
                        onclick={on_spawn_click}
                        disabled={!props.is_simulating}
                        title={if props.is_simulating { "Spawn Marbles" } else { "Start simulation first" }}
                    >
                        <Icon data={IconData::LUCIDE_CIRCLE_DOT} width="18px" height="18px" />
                        <span class="editor-spawn-arrow">{"▼"}</span>
                    </button>
                    if *show_spawn_input {
                        <div class="editor-spawn-dropdown">
                            <input
                                type="number"
                                class="editor-spawn-input"
                                value={props.spawn_count.to_string()}
                                min="1"
                                max="100"
                                oninput={on_spawn_count_input}
                            />
                        </div>
                    }
                </div>
                <button
                    class="editor-meatball-btn"
                    onclick={on_reset_click}
                    title="Reset Simulation"
                >
                    <Icon data={IconData::LUCIDE_ROTATE_CCW} width="18px" height="18px" />
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
