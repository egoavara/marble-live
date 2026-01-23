//! LocalStorage utilities for user settings.

use crate::fingerprint::generate_hash_code;
use marble_core::Color;
use serde::{Deserialize, Serialize};

const USER_SETTINGS_KEY: &str = "marble-live-user-settings";

/// User settings stored in LocalStorage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub name: String,
    pub color: Color,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            color: Color::RED,
        }
    }
}

impl UserSettings {
    /// Check if user settings exist in LocalStorage.
    pub fn exists() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Ok(Some(storage)) = window.local_storage() else {
            return false;
        };
        storage.get_item(USER_SETTINGS_KEY).ok().flatten().is_some()
    }

    /// Load user settings from LocalStorage.
    pub fn load() -> Option<Self> {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        let json = storage.get_item(USER_SETTINGS_KEY).ok()??;
        serde_json::from_str(&json).ok()
    }

    /// Save user settings to LocalStorage.
    pub fn save(&self) -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Ok(Some(storage)) = window.local_storage() else {
            return false;
        };
        let Ok(json) = serde_json::to_string(self) else {
            return false;
        };
        storage.set_item(USER_SETTINGS_KEY, &json).is_ok()
    }

    /// Get display name with hash code (e.g., "PlayerName#A1B2").
    pub fn display_name(&self) -> String {
        let hash_code = generate_hash_code(&self.name);
        format!("{}#{}", self.name, hash_code)
    }

    /// Get the hash code for the current name.
    pub fn hash_code(&self) -> String {
        generate_hash_code(&self.name)
    }
}

/// Available colors for player selection.
pub fn available_colors() -> Vec<Color> {
    Color::palette()
}

/// Get color name for display.
pub fn color_name(color: &Color) -> &'static str {
    match (color.r, color.g, color.b) {
        (255, 0, 0) => "Red",
        (0, 0, 255) => "Blue",
        (0, 255, 0) => "Green",
        (255, 255, 0) => "Yellow",
        (128, 0, 128) => "Purple",
        (255, 165, 0) => "Orange",
        (0, 255, 255) => "Cyan",
        (255, 192, 203) => "Pink",
        _ => "Custom",
    }
}

/// Convert Color to CSS color string.
pub fn color_to_css(color: &Color) -> String {
    format!("rgb({}, {}, {})", color.r, color.g, color.b)
}
