//! Marble-Live Client
//!
//! Yew WASM frontend application.
//!
//! This crate is WASM-only. Use `trunk build` or `cargo check --target wasm32-unknown-unknown`.

#[cfg(not(target_arch = "wasm32"))]
compile_error!("marble-client only supports wasm32 target. Use: cargo check -p marble-client --target wasm32-unknown-unknown");

mod app;
mod camera;
mod components;
mod fingerprint;
mod hooks;
mod pages;
mod ranking;
mod renderer;
mod routes;
mod services;
mod state;
mod util;

use app::App;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};

fn main() {
    // Initialize custom panic hook that redirects to panic page
    pages::set_panic_hook();

    // Initialize tracing for wasm with tracing-web
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(MakeWebConsoleWriter::new());

    let perf_layer = performance_layer().with_details_from_fields(Pretty::default());

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();

    yew::Renderer::<App>::new().render();
}
