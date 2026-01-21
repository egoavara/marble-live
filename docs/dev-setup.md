# 개발 환경 설정

## 필수 도구

```bash
# Rust WASM 타겟
rustup target add wasm32-unknown-unknown

# trunk (WASM 빌드)
cargo install trunk

# cargo-watch (핫 리로드)
cargo install cargo-watch

# protobuf 컴파일러
# Ubuntu/Debian
sudo apt install protobuf-compiler
# macOS
brew install protobuf
```

## 개발 서버 실행

```bash
# 클라이언트 (WASM) - localhost:8080
cd crates/marble-client
trunk serve --open

# 서버 - localhost:3000
cargo watch -x 'run -p marble-server'
```

## 빌드

```bash
# 전체 빌드
cargo build --workspace

# WASM 릴리스 빌드
trunk build --release

# 테스트
cargo test --workspace
```

## gRPC 디버깅

```bash
# grpcurl 설치
# Ubuntu/Debian
sudo apt install grpcurl
# macOS
brew install grpcurl

# 서비스 호출 예시
grpcurl -plaintext localhost:3000 marble.room.RoomService/ListRooms
```
