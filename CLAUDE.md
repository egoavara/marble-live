# Marble-Live

구슬 룰렛 + WebRTC P2P 실시간 세션 공유

## Claude 규칙

### 서버 실행 금지
- `trunk serve`, `cargo run -p marble-server` 등 포트를 사용하는 서버 실행 명령어는 **절대 직접 호출하지 말 것**
- 사용자가 직접 서버를 띄워야 함
- 빌드/체크 명령어(`trunk build`, `cargo check`, `cargo build`)는 허용

## 기술 스택

- Backend: Axum + tonic (gRPC-Web)
- Frontend: Yew (WASM)
- 통신: protobuf
- 물리엔진: Rapier2D (`enhanced-determinism`)
- P2P: matchbox

## 프로젝트 구조

- `crates/marble-core/` - 물리, 게임 로직
- `crates/marble-proto/` - protobuf (tonic-build)
- `crates/marble-client/` - Yew WASM
- `crates/marble-server/` - Axum + tonic

## 프로젝트 관리

- [GitHub Project Board](https://github.com/users/egoavara/projects/3)
- [GitHub Issues](https://github.com/egoavara/marble-live/issues)

## 상세 문서 (필요시 참조)

- Issue 목록 → `docs/issues.md`
- P2P 동기화 (호스트 선출, 해시 검증, 재동기화) → `docs/p2p-sync.md`
- 개발 환경, 빌드 명령어 → `docs/dev-setup.md`
- 코드 컨벤션, 커밋, PR 규칙 → `docs/conventions.md`
- 참고 자료 링크 → `docs/links.md`
