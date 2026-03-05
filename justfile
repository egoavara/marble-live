# Marble Live Development Commands

set dotenv-load := false

# Default recipe
default:
    @just --list

[private]
require-dotenv:
    #!/usr/bin/env bash
    if [ ! -f .env ]; then
        echo "ERROR: .env 파일이 없습니다. .env.example을 복사하세요:"
        echo "  cp .env.example .env"
        exit 1
    fi
    set -a; source .env; set +a

# Run client development server (trunk)
clt: require-dotenv
    #!/usr/bin/env bash
    set -a; source .env; set +a
    trunk serve

# Run server development (watch mode)
svr: require-dotenv
    #!/usr/bin/env bash
    set -a; source .env; set +a
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

# Run k6 gRPC API tests (usage: just k6, just k6 user, just k6 scenario, just k6-load, just k6-load scenario)
k6 target='scenario':
    k6 run tests/k6/{{target}}.js

# Run k6 load tests (usage: just k6-load, just k6-load scenario 20)
k6-load target='scenario' vus='10':
    K6_MODE=load K6_VUS={{vus}} k6 run tests/k6/{{target}}.js

# Run all k6 functional tests
k6-all:
    #!/usr/bin/env bash
    set -e
    for f in user avatar map room scenario; do
        echo "=== Running $f ==="
        k6 run tests/k6/$f.js
    done

# Clean build artifacts
clean:
    cargo clean
    trunk clean

