//! Meatball menu component for settings access.

use crate::components::SettingsModal;
use yew::prelude::*;

/// Meatball menu button that opens settings modal.
#[function_component(MeatballMenu)]
pub fn meatball_menu() -> Html {
    let show_settings = use_state(|| false);

    let on_open = {
        let show_settings = show_settings.clone();
        Callback::from(move |_| {
            show_settings.set(true);
        })
    };

    let on_close = {
        let show_settings = show_settings.clone();
        Callback::from(move |_| {
            show_settings.set(false);
        })
    };

    html! {
        <>
            <button class="meatball-btn" onclick={on_open} title="Settings">
                <span class="meatball-dots">{ "•••" }</span>
            </button>

            { if *show_settings {
                html! { <SettingsModal on_close={on_close} /> }
            } else {
                html! {}
            }}
        </>
    }
}
