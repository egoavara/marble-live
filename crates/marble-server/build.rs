//! Build script for marble-server
//!
//! Validates that the dist/ folder exists for rust-embed.
//! The actual client build should be done separately via `trunk build`
//! before building the server.
//!
//! Use `just build-server` to build both client and server in the correct order.

use std::env;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir).join("../..");
    let dist_dir = workspace_root.join("dist");

    // Rerun if dist/ directory changes
    println!("cargo:rerun-if-changed={}", dist_dir.display());

    // Check if dist/ directory exists
    if !dist_dir.exists() {
        // SKIP_CLIENT_BUILD allows building without dist/ (for CI or dev)
        if env::var("SKIP_CLIENT_BUILD").is_ok() {
            println!("cargo:warning=SKIP_CLIENT_BUILD set, dist/ directory missing but continuing");
            // Create empty dist with placeholder for rust-embed
            std::fs::create_dir_all(&dist_dir).ok();
            std::fs::write(dist_dir.join("index.html"), "<!-- placeholder -->").ok();
        } else {
            println!("cargo:warning=dist/ directory not found!");
            println!("cargo:warning=Run `trunk build --release` first, or use `just build-server`");
            println!("cargo:warning=Set SKIP_CLIENT_BUILD=1 to skip this check");
        }
    }
}
