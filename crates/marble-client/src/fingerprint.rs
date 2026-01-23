//! Browser fingerprint generation for user identification.

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, WebGlRenderingContext};

const FINGERPRINT_STORAGE_KEY: &str = "marble-live-fingerprint";

/// Get the browser fingerprint, using cached value if available.
pub fn get_browser_fingerprint() -> String {
    // Try to get cached fingerprint from LocalStorage
    if let Some(cached) = get_cached_fingerprint() {
        return cached;
    }

    // Generate new fingerprint
    let fingerprint = generate_browser_fingerprint();

    // Cache it
    cache_fingerprint(&fingerprint);

    fingerprint
}

/// Get cached fingerprint from LocalStorage.
fn get_cached_fingerprint() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(FINGERPRINT_STORAGE_KEY).ok()?
}

/// Cache fingerprint to LocalStorage.
fn cache_fingerprint(fingerprint: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let _ = storage.set_item(FINGERPRINT_STORAGE_KEY, fingerprint);
}

/// Generate a new browser fingerprint using Canvas and WebGL.
fn generate_browser_fingerprint() -> String {
    let canvas_fp = generate_canvas_fingerprint();
    let webgl_fp = generate_webgl_fingerprint();

    // Combine fingerprints
    let combined = format!("{}{}", canvas_fp, webgl_fp);

    // Hash the combined string
    hash_string(&combined)
}

/// Generate a canvas-based fingerprint.
fn generate_canvas_fingerprint() -> String {
    let Some(window) = web_sys::window() else {
        return String::new();
    };
    let Some(document) = window.document() else {
        return String::new();
    };
    let Ok(canvas) = document.create_element("canvas") else {
        return String::new();
    };
    let Ok(canvas) = canvas.dyn_into::<HtmlCanvasElement>() else {
        return String::new();
    };

    canvas.set_width(200);
    canvas.set_height(50);

    let Ok(Some(context)) = canvas.get_context("2d") else {
        return String::new();
    };
    let Ok(ctx) = context.dyn_into::<CanvasRenderingContext2d>() else {
        return String::new();
    };

    // Draw text with various styles
    ctx.set_font("14px Arial");
    ctx.set_fill_style_str("#f60");
    ctx.fill_rect(125.0, 1.0, 62.0, 20.0);

    ctx.set_fill_style_str("#069");
    let _ = ctx.fill_text("Marble Live!", 2.0, 15.0);

    ctx.set_fill_style_str("rgba(102, 204, 0, 0.7)");
    let _ = ctx.fill_text("Fingerprint", 4.0, 37.0);

    // Get data URL
    canvas.to_data_url().unwrap_or_default()
}

/// Generate a WebGL-based fingerprint.
fn generate_webgl_fingerprint() -> String {
    let Some(window) = web_sys::window() else {
        return String::new();
    };
    let Some(document) = window.document() else {
        return String::new();
    };
    let Ok(canvas) = document.create_element("canvas") else {
        return String::new();
    };
    let Ok(canvas) = canvas.dyn_into::<HtmlCanvasElement>() else {
        return String::new();
    };

    // Try WebGL
    let context = canvas
        .get_context("webgl")
        .ok()
        .flatten()
        .or_else(|| canvas.get_context("experimental-webgl").ok().flatten());

    let Some(context) = context else {
        return String::new();
    };

    let Ok(gl) = context.dyn_into::<WebGlRenderingContext>() else {
        return String::new();
    };

    let mut parts = Vec::new();

    // Get renderer info
    if let Some(ext) = gl.get_extension("WEBGL_debug_renderer_info").ok().flatten() {
        // UNMASKED_VENDOR_WEBGL = 0x9245
        // UNMASKED_RENDERER_WEBGL = 0x9246
        if let Some(vendor) = gl.get_parameter(0x9245).ok() {
            if let Some(vendor_str) = vendor.as_string() {
                parts.push(vendor_str);
            }
        }
        if let Some(renderer) = gl.get_parameter(0x9246).ok() {
            if let Some(renderer_str) = renderer.as_string() {
                parts.push(renderer_str);
            }
        }
        drop(ext);
    }

    // Get WebGL version
    if let Some(version) = gl.get_parameter(WebGlRenderingContext::VERSION).ok() {
        if let Some(version_str) = version.as_string() {
            parts.push(version_str);
        }
    }

    // Get shading language version
    if let Some(sl_version) = gl
        .get_parameter(WebGlRenderingContext::SHADING_LANGUAGE_VERSION)
        .ok()
    {
        if let Some(sl_str) = sl_version.as_string() {
            parts.push(sl_str);
        }
    }

    parts.join("|")
}

/// Simple hash function for strings.
fn hash_string(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    format!("{:016x}", hash)
}

/// Generate a hash code for display name from fingerprint and name.
pub fn generate_hash_code(name: &str) -> String {
    let fingerprint = get_browser_fingerprint();
    let combined = format!("{}{}", fingerprint, name);

    let mut hash: u32 = 0;
    for byte in combined.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(byte));
    }

    // Return 4-digit hex code
    format!("{:04X}", hash & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_string() {
        let hash = hash_string("test");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 16);
    }
}
