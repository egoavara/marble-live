//! Panic page displayed when a WASM panic occurs.

use crate::routes::Route;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_router::prelude::*;

const PANIC_INFO_KEY: &str = "marble_panic_info";

/// Retrieves panic info from localStorage.
fn get_panic_info() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(PANIC_INFO_KEY).ok()?
}

/// Clears panic info from localStorage.
fn clear_panic_info() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item(PANIC_INFO_KEY);
        }
    }
}

/// Sets up a custom panic hook that saves panic info to localStorage
/// and redirects to the panic page.
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // Format panic message
        let message = info.to_string();

        // Get location if available
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let panic_info = format!("{}\n\nLocation: {}", message, location);

        // Log to console for debugging
        web_sys::console::error_1(&JsValue::from_str(&panic_info));

        // Save to localStorage
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.set_item(PANIC_INFO_KEY, &panic_info);
            }

            // Redirect to panic page
            let _ = window.location().set_pathname("/panic");
        }
    }));
}

/// Panic page component.
#[function_component(PanicPage)]
pub fn panic_page() -> Html {
    let panic_info = use_state(|| get_panic_info());

    let on_clear = {
        let panic_info = panic_info.clone();
        Callback::from(move |_: MouseEvent| {
            clear_panic_info();
            panic_info.set(None);
        })
    };

    let on_copy = {
        let info = (*panic_info).clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(ref text) = info {
                if let Some(window) = web_sys::window() {
                    let clipboard = window.navigator().clipboard();
                    let _ = clipboard.write_text(text);
                }
            }
        })
    };

    html! {
        <main class="page panic-page">
            <div class="panic-container">
                <h1>{ "💥 Panic!" }</h1>
                <p class="panic-description">
                    { "애플리케이션에서 예기치 않은 오류가 발생했습니다." }
                </p>

                if let Some(info) = &*panic_info {
                    <div class="panic-info-box">
                        <h2>{ "오류 정보" }</h2>
                        <pre class="panic-details">{ info }</pre>
                        <div class="panic-actions">
                            <button onclick={on_copy} class="btn-secondary">
                                { "복사" }
                            </button>
                            <button onclick={on_clear} class="btn-secondary">
                                { "오류 정보 삭제" }
                            </button>
                        </div>
                    </div>
                } else {
                    <p class="panic-cleared">
                        { "오류 정보가 없거나 이미 삭제되었습니다." }
                    </p>
                }

                <div class="panic-navigation">
                    <Link<Route> to={Route::Home} classes="btn-primary">
                        { "홈으로 돌아가기" }
                    </Link<Route>>
                </div>
            </div>
        </main>
    }
}
