//! Common modal component.

use yew::prelude::*;

/// Props for the Modal component.
#[derive(Properties, PartialEq)]
pub struct ModalProps {
    pub state: UseStateHandle<bool>,
    /// Modal content.
    pub children: Children,
    /// Optional modal title (displayed in header).
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub onclose: Option<Callback<()>>,
    /// Whether clicking the overlay closes the modal.
    #[prop_or(false)]
    pub overlay_click_closes: bool,
    /// Whether to show the close button (×).
    #[prop_or(false)]
    pub show_close_button: bool,
    /// Additional CSS classes for the modal container.
    #[prop_or_default]
    pub class: Classes,
}

/// Common modal component with configurable header, close button, and overlay click behavior.
#[function_component(Modal)]
pub fn modal(props: &ModalProps) -> Html {
    let is_showing = props.state.clone();

    let on_overlay_click = {
        let onclose = props.onclose.clone();
        let overlay_click_closes = props.overlay_click_closes;
        let is_showing = is_showing.clone();
        Callback::from(move |_: MouseEvent| {
            if overlay_click_closes {
                is_showing.set(false);
                if let Some(cb) = onclose.as_ref() {
                    cb.emit(());
                }
            }
        })
    };

    let on_modal_click = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let on_close_button_click = {
        let onclose = props.onclose.clone();
        let is_showing = is_showing.clone();
        Callback::from(move |_: MouseEvent| {
            is_showing.set(false);
            if let Some(cb) = onclose.as_ref() {
                cb.emit(());
            }
        })
    };

    let modal_classes = classes!("modal", props.class.clone());

    let header = match (&props.title, props.show_close_button) {
        (Some(title), true) => html! {
            <div class="modal-header">
                <h2>{ title.clone() }</h2>
                <button class="modal-close-btn" onclick={on_close_button_click}>
                    { "×" }
                </button>
            </div>
        },
        (Some(title), false) => html! {
            <div class="modal-header modal-header-no-close">
                <h2>{ title.clone() }</h2>
            </div>
        },
        (None, true) => html! {
            <div class="modal-header modal-header-close-only">
                <button class="modal-close-btn" onclick={on_close_button_click}>
                    { "×" }
                </button>
            </div>
        },
        (None, false) => html! {},
    };

    html! {
        {if *is_showing.clone() {
            html! {
                <div class="modal-overlay" onclick={on_overlay_click}>
                    <div class={modal_classes} onclick={on_modal_click}>
                        { header }
                        <div class="modal-content">
                            { for props.children.iter() }
                        </div>
                    </div>
                </div>
            }
        } else {
            html! {}
        }}
    }
}
