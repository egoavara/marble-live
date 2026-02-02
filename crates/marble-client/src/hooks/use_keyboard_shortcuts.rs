//! Keyboard shortcuts hook for editor.
//!
//! Provides a reusable hook for handling keyboard shortcuts in Yew components.

use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;
use yew::prelude::*;

/// Configuration for keyboard shortcuts.
#[derive(Clone, PartialEq)]
pub struct KeyboardShortcutsConfig {
    /// Callback when Ctrl+C is pressed.
    pub on_copy: Option<Callback<()>>,
    /// Callback when Ctrl+V is pressed.
    pub on_paste: Option<Callback<()>>,
    /// Callback when Delete or Backspace is pressed.
    pub on_delete: Option<Callback<()>>,
    /// Callback when Ctrl+Z is pressed (undo).
    pub on_undo: Option<Callback<()>>,
    /// Callback when Ctrl+Y or Ctrl+Shift+Z is pressed (redo).
    pub on_redo: Option<Callback<()>>,
    /// Whether shortcuts are enabled.
    pub enabled: bool,
}

impl Default for KeyboardShortcutsConfig {
    fn default() -> Self {
        Self {
            on_copy: None,
            on_paste: None,
            on_delete: None,
            on_undo: None,
            on_redo: None,
            enabled: true,
        }
    }
}

/// Check if the event target is an input element (input, textarea, etc.)
fn is_input_element(event: &KeyboardEvent) -> bool {
    if let Some(target) = event.target() {
        if let Some(element) = target.dyn_ref::<web_sys::HtmlElement>() {
            let tag_name = element.tag_name().to_lowercase();
            return matches!(tag_name.as_str(), "input" | "textarea" | "select");
        }
    }
    false
}

/// Hook for handling keyboard shortcuts.
///
/// This hook attaches a global keydown listener to the document and calls
/// the appropriate callback based on the key combination pressed.
///
/// # Arguments
///
/// * `config` - Configuration containing callbacks for each shortcut.
///
/// # Example
///
/// ```ignore
/// use_keyboard_shortcuts(KeyboardShortcutsConfig {
///     on_copy: Some(on_copy_callback),
///     on_paste: Some(on_paste_callback),
///     on_delete: Some(on_delete_callback),
///     enabled: true,
///     ..Default::default()
/// });
/// ```
#[hook]
pub fn use_keyboard_shortcuts(config: KeyboardShortcutsConfig) {
    let listener_ref = use_mut_ref(|| None::<EventListener>);

    use_effect_with(config.clone(), move |config| {
        // Clean up previous listener
        *listener_ref.borrow_mut() = None;

        if !config.enabled {
            return;
        }

        let config = config.clone();
        let document = gloo::utils::document();

        let listener = EventListener::new(&document, "keydown", move |event| {
            let event = event.dyn_ref::<KeyboardEvent>().unwrap();

            // Skip if focus is on an input element
            if is_input_element(event) {
                return;
            }

            let key = event.key();
            let ctrl = event.ctrl_key() || event.meta_key(); // Support Cmd on macOS
            let shift = event.shift_key();

            // Ctrl+C - Copy
            if ctrl && !shift && key == "c" {
                if let Some(ref cb) = config.on_copy {
                    event.prevent_default();
                    cb.emit(());
                }
                return;
            }

            // Ctrl+V - Paste
            if ctrl && !shift && key == "v" {
                if let Some(ref cb) = config.on_paste {
                    event.prevent_default();
                    cb.emit(());
                }
                return;
            }

            // Delete or Backspace - Delete
            if !ctrl && !shift && (key == "Delete" || key == "Backspace") {
                if let Some(ref cb) = config.on_delete {
                    event.prevent_default();
                    cb.emit(());
                }
                return;
            }

            // Ctrl+Z - Undo
            if ctrl && !shift && key == "z" {
                if let Some(ref cb) = config.on_undo {
                    event.prevent_default();
                    cb.emit(());
                }
                return;
            }

            // Ctrl+Y or Ctrl+Shift+Z - Redo
            if ctrl && (key == "y" || (shift && key == "Z")) {
                if let Some(ref cb) = config.on_redo {
                    event.prevent_default();
                    cb.emit(());
                }
            }
        });

        *listener_ref.borrow_mut() = Some(listener);
    });
}
