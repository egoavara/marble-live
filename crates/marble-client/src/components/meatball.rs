//! Meatball menu component for settings access.

use crate::components::SettingsModal;
use yew::prelude::*;
use yew_icons::{Icon, IconData};

/// Props for the Layout component.
#[derive(Properties, PartialEq)]
pub struct MeatballProps {
    pub data: IconData,
    #[prop_or_default]
    pub onclick: Option<Callback<MouseEvent>>,
}

/// Meatball menu button that opens settings modal.
#[function_component(Meatball)]
pub fn meatball(props: &MeatballProps) -> Html {
    let show_settings = use_state(|| false);

    html! {
        <>
            <button class="meatball-btn" onclick={props.onclick.clone()} title="Settings">
                <Icon data={props.data.clone()} class="meatball-icon"/>
            </button>
        </>
    }
}
