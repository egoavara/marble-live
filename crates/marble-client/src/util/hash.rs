use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Hash using Web Crypto API (SHA-256).
async fn hash_sha256(data: &str) -> Option<String> {
    let window = web_sys::window()?;
    let crypto = window.crypto().ok()?;
    let subtle = crypto.subtle();

    let data_array = Uint8Array::from(data.as_bytes());

    let promise = subtle
        .digest_with_str_and_buffer_source("SHA-256", &data_array)
        .ok()?;
    let result = JsFuture::from(promise).await.ok()?;
    let buffer = result.dyn_into::<ArrayBuffer>().ok()?;
    let array = Uint8Array::new(&buffer);

    // Convert to hex string
    let bytes = array.to_vec();
    Some(bytes.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Fallback hash function if Web Crypto API is unavailable.
fn hash_fallback(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    format!("{:016x}", hash)
}

pub async fn generate_hash(s: &str) -> String {
    hash_sha256(s).await.unwrap_or_else(|| hash_fallback(s))
}
