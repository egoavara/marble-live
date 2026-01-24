//! Settings modal component.

use crate::{
    components::Modal,
    hooks::{use_config_username, use_opt_userhash},
};
use yew::prelude::*;

/// Props for the SettingsModal component.
#[derive(Properties, PartialEq)]
pub struct SettingsModalProps {
    pub state: UseStateHandle<bool>,
    #[prop_or_default]
    pub onclose: Option<Callback<()>>,
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

    html! {
        <Modal
            state={props.state.clone()}
            title="Settings"
            onclose={props.onclose.clone()}
            overlay_click_closes=true
            show_close_button=true
            class="settings-modal"
        >
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
        </Modal>
    }
}
