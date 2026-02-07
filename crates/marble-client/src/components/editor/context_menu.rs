//! Context menu component for editor.

use yew::prelude::*;

/// Context menu state.
#[derive(Clone, PartialEq, Default)]
pub struct ContextMenuState {
    /// Whether the menu is visible.
    pub visible: bool,
    /// Screen position for menu display.
    pub screen_pos: (f32, f32),
    /// World position for paste operation.
    pub world_pos: (f32, f32),
    /// Selected object index (None for empty space click).
    pub target_index: Option<usize>,
}

impl ContextMenuState {
    /// Create a new hidden context menu state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show context menu at the given position.
    pub fn show(
        screen_pos: (f32, f32),
        world_pos: (f32, f32),
        target_index: Option<usize>,
    ) -> Self {
        Self {
            visible: true,
            screen_pos,
            world_pos,
            target_index,
        }
    }

    /// Hide context menu.
    pub fn hide() -> Self {
        Self::default()
    }
}

#[derive(Properties, PartialEq)]
pub struct ContextMenuProps {
    pub state: ContextMenuState,
    pub has_clipboard: bool,
    pub on_close: Callback<()>,
    pub on_copy: Callback<usize>,
    pub on_paste: Callback<(f32, f32)>,
    pub on_delete: Callback<usize>,
    pub on_mirror_x: Callback<usize>,
    pub on_mirror_y: Callback<usize>,
}

#[function_component(ContextMenu)]
pub fn context_menu(props: &ContextMenuProps) -> Html {
    // Close menu when clicking outside
    let on_close = props.on_close.clone();
    let onmousedown_overlay = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        e.stop_propagation();
        on_close.emit(());
    });

    // Prevent event propagation on menu click
    let onmousedown_menu = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let onclick_copy = {
        let on_copy = props.on_copy.clone();
        let on_close = props.on_close.clone();
        let target_index = props.state.target_index;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(idx) = target_index {
                on_copy.emit(idx);
            }
            on_close.emit(());
        })
    };

    let onclick_paste = {
        let on_paste = props.on_paste.clone();
        let on_close = props.on_close.clone();
        let has_clipboard = props.has_clipboard;
        let world_pos = props.state.world_pos;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if has_clipboard {
                on_paste.emit(world_pos);
            }
            on_close.emit(());
        })
    };

    let onclick_delete = {
        let on_delete = props.on_delete.clone();
        let on_close = props.on_close.clone();
        let target_index = props.state.target_index;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(idx) = target_index {
                on_delete.emit(idx);
            }
            on_close.emit(());
        })
    };

    let onclick_mirror_x = {
        let on_mirror_x = props.on_mirror_x.clone();
        let on_close = props.on_close.clone();
        let target_index = props.state.target_index;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(idx) = target_index {
                on_mirror_x.emit(idx);
            }
            on_close.emit(());
        })
    };

    let onclick_mirror_y = {
        let on_mirror_y = props.on_mirror_y.clone();
        let on_close = props.on_close.clone();
        let target_index = props.state.target_index;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(idx) = target_index {
                on_mirror_y.emit(idx);
            }
            on_close.emit(());
        })
    };

    if !props.state.visible {
        return html! {};
    }

    let style = format!(
        "left: {}px; top: {}px;",
        props.state.screen_pos.0, props.state.screen_pos.1
    );

    let has_object = props.state.target_index.is_some();
    let paste_disabled = !props.has_clipboard;

    html! {
        <div class="context-menu-overlay" onmousedown={onmousedown_overlay}>
            <div class="context-menu" {style} onmousedown={onmousedown_menu}>
                if has_object {
                    // Object selected menu
                    <div class="context-menu-item" onclick={onclick_copy}>
                        <span class="context-menu-icon">{"📋"}</span>
                        <span>{"복사"}</span>
                    </div>
                    <div
                        class={classes!("context-menu-item", paste_disabled.then_some("disabled"))}
                        onclick={onclick_paste.clone()}
                    >
                        <span class="context-menu-icon">{"📥"}</span>
                        <span>{"붙여넣기"}</span>
                    </div>
                    <div class="context-menu-divider" />
                    <div class="context-menu-submenu">
                        <div class="context-menu-item context-menu-item-submenu">
                            <span class="context-menu-icon">{"🔄"}</span>
                            <span>{"대칭"}</span>
                            <span class="context-menu-arrow">{"▶"}</span>
                        </div>
                        <div class="context-menu-submenu-content">
                            <div class="context-menu-item" onclick={onclick_mirror_x}>
                                <span class="context-menu-icon">{"↔️"}</span>
                                <span>{"X축 대칭"}</span>
                            </div>
                            <div class="context-menu-item" onclick={onclick_mirror_y}>
                                <span class="context-menu-icon">{"↕️"}</span>
                                <span>{"Y축 대칭"}</span>
                            </div>
                        </div>
                    </div>
                    <div class="context-menu-divider" />
                    <div class="context-menu-item context-menu-item-danger" onclick={onclick_delete}>
                        <span class="context-menu-icon">{"🗑️"}</span>
                        <span>{"삭제"}</span>
                    </div>
                } else {
                    // Empty space menu
                    <div
                        class={classes!("context-menu-item", paste_disabled.then_some("disabled"))}
                        onclick={onclick_paste}
                    >
                        <span class="context-menu-icon">{"📥"}</span>
                        <span>{"붙여넣기"}</span>
                    </div>
                }
            </div>
        </div>
    }
}
