//! Minimal layout component with meatball menu.

use crate::components::{LogoExpandable, Meatball, SettingsModal};
use yew::prelude::*;
use yew_icons::IconData;

/// Props for the Layout component.
#[derive(Properties, PartialEq)]
pub struct LayoutProps {
    /// Child content to render.
    pub children: Html,
    /// Whether to show the meatball menu (default: true).
    #[prop_or(true)]
    pub show_settings: bool,
    /// Whether to make layout transparent for Bevy canvas (Play/Editor pages).
    #[prop_or(false)]
    pub transparent: bool,
}

/// Minimal layout component with meatball menu in top-right corner.
#[function_component(Layout)]
pub fn layout(props: &LayoutProps) -> Html {
    let show_settings_modal = use_state(|| false);
    let logo_hovered = use_state(|| false);

    let on_open_settings = {
        let show_settings_modal = show_settings_modal.clone();
        Callback::from(move |_| {
            show_settings_modal.set(true);
        })
    };

    let on_logo_mouseenter = {
        let logo_hovered = logo_hovered.clone();
        Callback::from(move |_: MouseEvent| {
            logo_hovered.set(true);
        })
    };

    let on_logo_mouseleave = {
        let logo_hovered = logo_hovered.clone();
        Callback::from(move |_: MouseEvent| {
            logo_hovered.set(false);
        })
    };

    let layout_class = if props.transparent {
        "app-layout app-layout--transparent"
    } else {
        "app-layout"
    };

    html! {
        <div class={layout_class}>
            <div class="top-left-logo" onmouseenter={on_logo_mouseenter} onmouseleave={on_logo_mouseleave}>
                <LogoExpandable state={*logo_hovered} size={28} />
            </div>

            { if props.show_settings {
                html! {
                    <div class="top-right-menu">
                        <Meatball data={IconData::LUCIDE_COG} onclick={on_open_settings.clone()} />
                    </div>
                }
            } else {
                html! {}
            }}

            <SettingsModal state={show_settings_modal.clone()} />

            <main class="app-main">
                { props.children.clone() }
            </main>
        </div>
    }
}
