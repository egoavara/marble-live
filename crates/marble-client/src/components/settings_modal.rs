//! Settings modal component.

use crate::{
    fingerprint::generate_hash_code,
    hooks::{use_config_username, use_opt_userhash},
};
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

    let on_overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| {
            on_close.emit(());
        })
    };

    let on_modal_click = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    html! {
        <div class="modal-overlay" onclick={on_overlay_click}>
            <div class="settings-modal" onclick={on_modal_click}>
                <div class="modal-header">
                    <h2>{ "Settings" }</h2>
                    <button class="modal-close-btn" onclick={props.on_close.reform(|_| ())}>
                        { "×" }
                    </button>
                </div>

                <form class="settings-form">
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
                </form>
            </div>
        </div>
    }
}
