//! Marble-Live Client
//!
//! Yew WASM frontend application.
//!
//! This crate is WASM-only. Use `trunk build` or `cargo check --target wasm32-unknown-unknown`.

#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "marble-client only supports wasm32 target. Use: cargo check -p marble-client --target wasm32-unknown-unknown"
);

mod app;
mod camera;
mod components;
mod fingerprint;
mod hooks;
mod pages;
mod ranking;
// mod renderer; // Removed - Bevy handles rendering now
mod renderer_stub; // Stub for migration period
mod routes;
mod services;
mod state;
mod util;

use app::App;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, Layer};
use tracing_web::MakeWebConsoleWriter;

fn main() {
    // Initialize custom panic hook that redirects to panic page
    pages::set_panic_hook();

    // Initialize tracing for wasm with tracing-web
    let filter = EnvFilter::new("info,wgpu=error,naga=warn");

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(MakeWebConsoleWriter::new())
        .with_filter(filter);

    // NOTE: performance_layer() removed - it records ALL tracing events to
    // browser's Performance API, causing memory leak (Bevy generates hundreds
    // of spans per frame). Use Chrome DevTools Performance tab instead.
    tracing_subscriber::registry().with(fmt_layer).init();

    yew::Renderer::<App>::new().render();
}
