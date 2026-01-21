# Marble Live Development Commands

# Default recipe
default:
    @just --list

# Run client development server (trunk)
client:
    trunk serve --open

# Run server development (watch mode)
server:
    watchexec -r -e rs,toml,proto -- cargo run -p marble-server

# Run both client and server (requires terminal multiplexer)
dev:
    @echo "Run 'just client' and 'just server' in separate terminals"

# Build all crates
build:
    cargo build --all

# Build client for WASM
build-wasm:
    cargo build -p marble-client --target wasm32-unknown-unknown --release

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

# Run all tests
test:
    cargo test --all

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

# Watch and run tests
watch-test:
    watchexec -e rs,toml -- cargo test --all

# Watch server
watch-server:
    watchexec -r -e rs,toml,proto -- cargo run -p marble-server
