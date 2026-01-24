//! Welcome modal component for first-time users.

use crate::{
    fingerprint::generate_hash_code,
    hooks::{use_config_username, use_opt_userhash},
};
use marble_core::Color;
use yew::prelude::*;

/// Props for the WelcomeModal component.
#[derive(Properties, PartialEq)]
pub struct WelcomeModalProps {}

/// Welcome modal component shown to first-time users.
#[function_component(WelcomeModal)]
pub fn welcome_modal(props: &WelcomeModalProps) -> Html {
    let username = use_config_username();
    let on_name_input = {
        let name = username.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            name.set(Some(input.value()));
        })
    };

    let userhash = use_opt_userhash(username.clone());

    let display_userhash = userhash
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "???".to_string());
    let display_username = username
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "???".to_string());
    let display_name = format!("{}#{}", display_username, display_userhash);

    html! {
        <div class="modal-overlay">
            <div class="welcome-modal">
                <h2>{ "Marble Live" }</h2>
                <p class="welcome-subtitle">{ "Welcome! Set up your profile to get started." }</p>

                <form>
                    <div class="form-group">
                        <label for="name-input">{ "Name" }</label>
                        <input
                            id="name-input"
                            type="text"
                            class="name-input"
                            placeholder="Enter your name"
                            value={(*username).clone().unwrap_or_default()}
                            oninput={on_name_input}
                            maxlength="20"
                            required=true
                        />
                    </div>

                    <div class="display-name-preview">
                        <span class="preview-label">{ "Display Name: " }</span>
                        <span class="preview-value">{ display_name }</span>
                    </div>

                    <button
                        type="submit"
                        class="btn btn-primary submit-btn"
                        disabled={username.is_none()}
                    >
                        { "Start" }
                    </button>
                </form>
            </div>
        </div>
    }
}
