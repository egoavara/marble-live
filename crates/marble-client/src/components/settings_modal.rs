//! Settings modal component.

use crate::fingerprint::generate_hash_code;
use crate::storage::{available_colors, color_to_css, UserSettings};
use yew::prelude::*;

/// Props for the SettingsModal component.
#[derive(Properties, PartialEq)]
pub struct SettingsModalProps {
    /// Callback when modal is closed.
    pub on_close: Callback<()>,
}

/// Settings modal component.
#[function_component(SettingsModal)]
pub fn settings_modal(props: &SettingsModalProps) -> Html {
    // Load existing settings or use defaults
    let settings = UserSettings::load().unwrap_or_default();
    let name = use_state(|| settings.name.clone());
    let selected_color = use_state(|| settings.color);

    let on_name_input = {
        let name = name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            name.set(input.value());
        })
    };

    let on_save = {
        let name = name.clone();
        let selected_color = selected_color.clone();
        let on_close = props.on_close.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !name.is_empty() {
                let settings = UserSettings {
                    name: (*name).clone(),
                    color: *selected_color,
                };
                settings.save();
                on_close.emit(());
            }
        })
    };

    let on_overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| {
            on_close.emit(());
        })
    };

    let on_modal_click = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let hash_code = generate_hash_code(&name);
    let display_name = if name.is_empty() {
        "???#????".to_string()
    } else {
        format!("{}#{}", *name, hash_code)
    };

    let colors = available_colors();

    html! {
        <div class="modal-overlay" onclick={on_overlay_click}>
            <div class="settings-modal" onclick={on_modal_click}>
                <div class="modal-header">
                    <h2>{ "Settings" }</h2>
                    <button class="modal-close-btn" onclick={props.on_close.reform(|_| ())}>
                        { "×" }
                    </button>
                </div>

                <form class="settings-form" onsubmit={on_save}>
                    <div class="form-group">
                        <label for="name-input">{ "Name" }</label>
                        <input
                            id="name-input"
                            type="text"
                            class="name-input"
                            placeholder="Enter your name"
                            value={(*name).clone()}
                            oninput={on_name_input}
                            maxlength="20"
                            required=true
                        />
                    </div>

                    <div class="display-name-preview">
                        <span class="preview-label">{ "Display Name: " }</span>
                        <span class="preview-value">{ display_name }</span>
                    </div>

                    <div class="form-group">
                        <label>{ "Color" }</label>
                        <div class="color-picker">
                            { for colors.iter().map(|&color| {
                                let is_selected = *selected_color == color;
                                let color_clone = color;
                                let selected_color = selected_color.clone();
                                let onclick = Callback::from(move |_| {
                                    selected_color.set(color_clone);
                                });
                                html! {
                                    <button
                                        type="button"
                                        class={classes!("color-btn", is_selected.then_some("selected"))}
                                        style={format!("background-color: {}", color_to_css(&color))}
                                        onclick={onclick}
                                    />
                                }
                            })}
                        </div>
                    </div>

                    <div class="form-actions">
                        <button
                            type="submit"
                            class="btn btn-primary"
                            disabled={name.is_empty()}
                        >
                            { "Save" }
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
