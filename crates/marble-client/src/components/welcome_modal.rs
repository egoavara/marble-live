//! Welcome modal component for first-time users.

use crate::fingerprint::generate_hash_code;
use crate::storage::{available_colors, color_to_css, UserSettings};
use marble_core::Color;
use yew::prelude::*;

/// Props for the WelcomeModal component.
#[derive(Properties, PartialEq)]
pub struct WelcomeModalProps {
    /// Callback when user completes setup.
    pub on_complete: Callback<UserSettings>,
}

/// Welcome modal component shown to first-time users.
#[function_component(WelcomeModal)]
pub fn welcome_modal(props: &WelcomeModalProps) -> Html {
    let name = use_state(String::new);
    let selected_color = use_state(|| Color::RED);

    let on_name_input = {
        let name = name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            name.set(input.value());
        })
    };

    let on_submit = {
        let name = name.clone();
        let selected_color = selected_color.clone();
        let on_complete = props.on_complete.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !name.is_empty() {
                let settings = UserSettings {
                    name: (*name).clone(),
                    color: *selected_color,
                };
                settings.save();
                on_complete.emit(settings);
            }
        })
    };

    let hash_code = generate_hash_code(&name);
    let display_name = if name.is_empty() {
        "???#????".to_string()
    } else {
        format!("{}#{}", *name, hash_code)
    };

    let colors = available_colors();

    html! {
        <div class="modal-overlay">
            <div class="welcome-modal">
                <h2>{ "Marble Live" }</h2>
                <p class="welcome-subtitle">{ "Welcome! Set up your profile to get started." }</p>

                <form onsubmit={on_submit}>
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
                                        title={format!("{:?}", color)}
                                    />
                                }
                            })}
                        </div>
                    </div>

                    <button
                        type="submit"
                        class="btn btn-primary submit-btn"
                        disabled={name.is_empty()}
                    >
                        { "Start" }
                    </button>
                </form>
            </div>
        </div>
    }
}
