# Marble Live Development Commands

# Default recipe
default:
    @just --list

# Run client development server (trunk)
clt:
    trunk serve

# Run server development (watch mode)
svr:
    SKIP_CLIENT_BUILD=1 watchexec -w ./crates/marble-server -w ./crates/marble-proto -r -e rs,toml,proto -- cargo run -p marble-server

check-clt:
    cargo check -p marble-client --target wasm32-unknown-unknown

check-svr:
    cargo check -p marble-server

# Force rebuild proto
build-proto:
    cargo clean -p marble-proto
    cargo build -p marble-proto

# Build all crates
build:
    cargo build --all

# Build release
build-release:
    cargo build --release --all
    trunk build --release

# Build server with embedded client (single binary deployment)
build-server:
    trunk build --release
    cargo build -p marble-server --release

# Build server (skip client build, assumes dist/ exists)
build-server-only:
    SKIP_CLIENT_BUILD=1 cargo build -p marble-server --release

# Run tests (usage: just test, just test marble-core, just test marble-core --lib)
# marble-client is WASM-only and automatically routed to wasm32 target.
test *args:
    #!/usr/bin/env bash
    if [ -z "{{args}}" ]; then
        cargo test --workspace --exclude marble-client
        cargo test -p marble-client --target wasm32-unknown-unknown
    elif [[ "{{args}}" == marble-client* ]]; then
        cargo test -p {{args}} --target wasm32-unknown-unknown
    else
        cargo test -p {{args}}
    fi

# Run marble-core bevy system integration tests
test-bevy *filter:
    cargo test -p marble-core --lib -- bevy::systems {{filter}}

# Run marble-client WASM tests in Node.js (requires wasm-bindgen-cli)
test-wasm *args:
    cargo test -p marble-client --target wasm32-unknown-unknown {{args}}

# Run clippy
lint:
    cargo clippy --all -- -D warnings

# Format code
fmt:
    cargo fmt --all

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Clean build artifacts
clean:
    cargo clean
    trunk clean

