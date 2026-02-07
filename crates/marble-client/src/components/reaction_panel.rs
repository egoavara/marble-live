//! ReactionPanel component - emoji reaction buttons with expandable UI.

use gloo::storage::{LocalStorage, Storage};
use yew::prelude::*;

/// Global cooldown duration in milliseconds
pub const REACTION_COOLDOWN_MS: f64 = 300.0;

/// LocalStorage key for last used emoji
const LAST_EMOJI_KEY: &str = "marble-live:last-reaction-emoji";

/// Default emoji if none stored
const DEFAULT_EMOJI: &str = "\u{1F44D}"; // 👍

/// Supported reaction emojis with keyboard shortcuts
pub const REACTIONS: &[(&str, &str)] = &[
    ("1", "\u{1F44D}"),        // 👍
    ("2", "\u{2764}\u{FE0F}"), // ❤️
    ("3", "\u{1F602}"),        // 😂
    ("4", "\u{1F389}"),        // 🎉
    ("5", "\u{1F44F}"),        // 👏
];

/// Props for the ReactionPanel component.
#[derive(Properties, PartialEq)]
pub struct ReactionPanelProps {
    /// Callback when a reaction is sent
    pub on_send: Callback<String>,
    /// Whether reactions are disabled (disconnected or on cooldown)
    pub disabled: bool,
    /// Whether the panel is expanded
    pub expanded: bool,
    /// Callback when hover state changes
    pub on_hover_change: Callback<bool>,
    /// Last used emoji (state lifted to parent)
    pub last_emoji: String,
    /// Callback when last emoji should change (called when panel closes)
    pub on_last_emoji_change: Callback<String>,
}

/// Get the last used emoji from localStorage
pub fn get_last_emoji() -> String {
    LocalStorage::get(LAST_EMOJI_KEY).unwrap_or_else(|_| DEFAULT_EMOJI.to_string())
}

/// Save the last used emoji to localStorage
pub fn save_last_emoji(emoji: &str) {
    let _ = LocalStorage::set(LAST_EMOJI_KEY, emoji.to_string());
}

/// ReactionPanel component - expandable emoji reaction buttons.
///
/// Shows last used emoji when collapsed, expands on hover to show all emojis.
/// Uses pending_emoji pattern: emoji order only changes when panel closes.
#[function_component(ReactionPanel)]
pub fn reaction_panel(props: &ReactionPanelProps) -> Html {
    // pending_emoji stores the emoji to set as last when panel closes
    let pending_emoji = use_state(|| None::<String>);
    let prev_expanded = use_mut_ref(|| props.expanded);

    let on_send = props.on_send.clone();
    let on_hover_change = props.on_hover_change.clone();
    let on_last_emoji_change = props.on_last_emoji_change.clone();
    let disabled = props.disabled;
    let expanded = props.expanded;

    // Detect expanded state transition: true -> false
    {
        let pending_emoji = pending_emoji.clone();
        let on_last_emoji_change = on_last_emoji_change.clone();
        let prev_expanded = prev_expanded.clone();

        use_effect_with(expanded, move |&expanded| {
            let was_expanded = *prev_expanded.borrow();
            *prev_expanded.borrow_mut() = expanded;

            // When panel closes (expanded -> collapsed)
            if was_expanded && !expanded {
                if let Some(emoji) = (*pending_emoji).clone() {
                    on_last_emoji_change.emit(emoji);
                    pending_emoji.set(None);
                }
            }
            || {}
        });
    }

    // Build emoji list with last used first (from props)
    let ordered_emojis: Vec<&str> = {
        let last = props.last_emoji.as_str();
        let mut list: Vec<&str> = vec![last];
        for (_, emoji) in REACTIONS.iter() {
            if *emoji != last {
                list.push(emoji);
            }
        }
        list
    };

    let on_mouse_enter = {
        let on_hover_change = on_hover_change.clone();
        Callback::from(move |_| {
            on_hover_change.emit(true);
        })
    };

    let on_mouse_leave = {
        let on_hover_change = on_hover_change.clone();
        Callback::from(move |_| {
            on_hover_change.emit(false);
        })
    };

    let panel_class = classes!("reaction-panel-inline", expanded.then_some("expanded"));

    html! {
        <div
            class={panel_class}
            onmouseenter={on_mouse_enter}
            onmouseleave={on_mouse_leave}
        >
            { for ordered_emojis.iter().enumerate().map(|(idx, emoji)| {
                let on_send = on_send.clone();
                let pending_emoji = pending_emoji.clone();
                let emoji_str = emoji.to_string();

                // Check if this emoji is the pending (most recently selected) one
                let is_pending = (*pending_emoji).as_ref() == Some(&emoji_str);
                // Show primary only if no pending emoji is set
                let has_pending = pending_emoji.is_some();
                let is_primary = idx == 0 && !has_pending;

                let onclick = {
                    let emoji_str = emoji_str.clone();
                    Callback::from(move |_| {
                        // Save to localStorage immediately
                        save_last_emoji(&emoji_str);
                        // Set pending emoji (will be applied when panel closes)
                        pending_emoji.set(Some(emoji_str.clone()));
                        // Send the reaction
                        on_send.emit(emoji_str.clone());
                    })
                };

                // Find keyboard shortcut for this emoji
                let shortcut = REACTIONS.iter()
                    .find(|(_, e)| *e == *emoji)
                    .map(|(k, _)| *k)
                    .unwrap_or("");

                let btn_class = classes!(
                    "reaction-btn-inline",
                    is_primary.then_some("primary"),
                    disabled.then_some("disabled"),
                    is_pending.then_some("pending")
                );

                html! {
                    <button
                        class={btn_class}
                        onclick={onclick}
                        disabled={disabled}
                        title={format!("Press {} to send", shortcut)}
                        key={emoji_str}
                    >
                        { *emoji }
                    </button>
                }
            }) }
        </div>
    }
}

/// Get the emoji for a keyboard shortcut (1-5)
pub fn get_reaction_emoji(key: &str) -> Option<&'static str> {
    REACTIONS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, emoji)| *emoji)
}
