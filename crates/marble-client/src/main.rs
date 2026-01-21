//! Marble-Live Client
//!
//! Yew WASM frontend application.

mod app;
mod components;
mod pages;
mod renderer;
mod routes;
mod state;

use app::App;

fn main() {
    // Initialize console error panic hook for better error messages
    console_error_panic_hook::set_once();

    // Initialize tracing for wasm
    tracing_wasm::set_as_global_default();

    yew::Renderer::<App>::new().render();
}
