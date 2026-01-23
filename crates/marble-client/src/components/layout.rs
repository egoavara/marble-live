//! Minimal layout component with meatball menu.

use crate::components::MeatballMenu;
use yew::prelude::*;

/// Props for the Layout component.
#[derive(Properties, PartialEq)]
pub struct LayoutProps {
    /// Child content to render.
    pub children: Html,
    /// Whether to show the meatball menu (default: true).
    #[prop_or(true)]
    pub show_menu: bool,
}

/// Minimal layout component with meatball menu in top-right corner.
#[function_component(Layout)]
pub fn layout(props: &LayoutProps) -> Html {
    html! {
        <div class="app-layout">
            { if props.show_menu {
                html! {
                    <div class="top-right-menu">
                        <MeatballMenu />
                    </div>
                }
            } else {
                html! {}
            }}

            <main class="app-main">
                { props.children.clone() }
            </main>
        </div>
    }
}
