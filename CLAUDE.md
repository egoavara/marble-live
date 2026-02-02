# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 프로젝트 개요

Marble-Live: 구슬 룰렛 게임 + WebRTC P2P 실시간 세션 공유. Bevy 게임 엔진 기반의 결정론적 물리 시뮬레이션.

## 빌드 및 개발 명령어

```bash
just check-clt     # 클라이언트 빌드 체크 (WASM 타겟)
just check-svr     # 서버 빌드 체크
just test          # 전체 테스트
just lint          # Clippy lint
just fmt           # 코드 포맷
just fmt-check     # 포맷 검사
just build-proto   # Proto 재빌드 (스키마 변경 시)
just build         # 전체 빌드
just build-server  # 릴리즈 빌드 (서버 + 클라이언트 통합)
just clean         # 빌드 아티팩트 정리
```

## 중요 규칙

### 서버 실행 금지
`trunk serve`, `cargo run -p marble-server` 등 포트를 사용하는 서버는 **직접 실행하지 말 것**. 사용자가 직접 실행함.

### in-progress 프로젝트
정규 버전 전이므로 레거시 호환성 유지 불필요. 가장 적합한 방법으로 구현할 것

### 최신 버전 이용중

모델에 학습된 것보다 더 최신 라이브러리 사용중으로 확인된 라이브러리 코드를 작성할 때는 라이브러리 최신 사용법부터 먼저 참조.

## 프로젝트 구조

```
crates/
├── marble-core/      # 게임 로직 및 물리 시뮬레이션
│   └── src/
│       ├── bevy/     # Bevy ECS 통합 (현재 주력)
│       │   ├── plugin.rs      # MarbleCorePlugin, MarbleGamePlugin, MarbleEditorPlugin
│       │   ├── components.rs  # ECS 컴포넌트 (MainCamera, GameCamera 등)
│       │   ├── resources.rs   # ECS 리소스 (MarbleGameState, CommandQueue 등)
│       │   ├── events.rs      # 이벤트 (SpawnMarblesEvent, MapLoadedEvent 등)
│       │   ├── state_store.rs # Yew ↔ Bevy 상태 동기화
│       │   ├── wasm_entry.rs  # WASM 진입점 (Bevy App → Canvas 마운트)
│       │   └── systems/       # ECS 시스템
│       │       ├── camera/    # 카메라 (editor, follow, overview)
│       │       ├── editor/    # 에디터 (gizmo, input, selection)
│       │       ├── physics.rs, keyframe.rs, rendering.rs ...
│       ├── map.rs             # 맵 데이터 구조 (MapObject, Keyframe, Shape 등)
│       ├── keyframe.rs        # 키프레임 애니메이션
│       ├── physics.rs         # 물리 설정 (PHYSICS_DT, default_gravity)
│       ├── game.rs            # 게임 상태 (GameState, Player)
│       └── expr.rs, dsl.rs    # CEL 표현식 기반 DSL
├── marble-client/    # Yew WASM 프론트엔드
│   └── src/
│       ├── main.rs            # 진입점 (tracing 초기화, Yew 렌더)
│       ├── app.rs             # 메인 App 컴포넌트
│       ├── pages/             # 페이지 (editor, play, panic)
│       ├── components/        # UI 컴포넌트
│       │   ├── editor/        # 에디터 UI (canvas, gizmo, property_panel, timeline_panel)
│       │   ├── marble_editor.rs, marble_game.rs  # Bevy 캔버스 래퍼
│       │   └── meatball.rs, canvas.rs ...
│       ├── hooks/             # Yew 훅 (use_bevy, use_editor_state, use_game_loop)
│       ├── services/          # gRPC 서비스 클라이언트
│       └── state/             # 전역 상태
├── marble-server/    # Axum 백엔드
│   └── src/
│       ├── main.rs            # 진입점 (gRPC-Web + signaling + SPA 서빙)
│       ├── handler/           # gRPC 핸들러 (room_service)
│       ├── service/           # 서비스 (database)
│       └── topology/          # P2P 토폴로지 (bridge, mesh_group, manager)
└── marble-proto/     # Protobuf 정의
    └── src/
        └── *.proto → tonic-prost-build로 생성
```

## 핵심 라이브러리 버전

| 카테고리 | 라이브러리 | 버전 |
|----------|-----------|------|
| **Game Engine** | bevy | 0.18 |
| **Physics** | rapier2d | 0.32 |
| | bevy_rapier2d | git:bevy-0.18.0 (Buncys/bevy_rapier) |
| **Frontend** | yew | 0.22 |
| | yew-router | 0.19 |
| | wasm-bindgen | 0.2 |
| | web-sys | 0.3 |
| | gloo | 0.11 |
| **Backend** | axum | 0.8 |
| | tokio | 1 |
| | tower-http | 0.6 |
| **gRPC** | tonic | 0.14 |
| | prost | 0.14 |
| | tonic-web-wasm-client | 0.8.0 |
| **P2P** | matchbox_socket | 0.13 |
| | matchbox_signaling | 0.13 |
| **Serialization** | serde | 1.0 |
| | postcard | 1.1 |
| **RNG** | rand | 0.9 |
| | rand_chacha | 0.9 |
| **DSL** | cel-interpreter | 0.10 |
| **Embed** | rust-embed | 8.11 |

## 아키텍처

### Bevy 플러그인 계층

```
MarbleCorePlugin (공통)
├── 물리 설정 (bevy_rapier2d, 60Hz 고정 타임스텝)
├── 키프레임 애니메이션, 블랙홀 힘 적용
└── 게임 규칙 (트리거 감지, 도착 처리)

MarbleGamePlugin (게임 모드)
└── MarbleCorePlugin + 카메라 팔로우, 렌더링

MarbleEditorPlugin (에디터 모드)
└── MarbleCorePlugin + 기즈모, 선택, 시뮬레이션 제어, 프리뷰
```

### Yew ↔ Bevy 통신

- `CommandQueue`: Yew → Bevy (명령 전달)
- `StateStores`: Bevy → Yew (상태 동기화)
- WASM 환경에서 `wasm_entry.rs`가 Bevy App을 브라우저 canvas에 마운트

### P2P 동기화

- matchbox 기반 WebRTC signaling
- `enhanced-determinism` 활성화로 크로스 플랫폼 물리 일관성 보장
- 서버는 signaling만 담당, 실제 시뮬레이션은 클라이언트

## 개발 플로우

### 기능 유형별 개발 순서

| 기능 유형 | 개발 순서 |
|-----------|----------|
| Bevy 전용 (물리, 게임 로직) | marble-core 단독 |
| Yew UI 전용 | marble-client 단독 |
| 전체 기능 (UI + 게임) | marble-core → marble-client |
| 서버 연동 기능 | marble-proto → marble-server → marble-client |

### Bevy 기능 개발 플로우

```
1. 데이터 정의
   ├── components.rs  → #[derive(Component)] 구조체
   ├── events.rs      → #[derive(Message)] 이벤트
   └── resources.rs   → #[derive(Resource)] 전역 상태

2. 시스템 구현
   └── systems/*.rs   → pub fn system_name(Query, Res, ResMut, MessageReader/Writer)

3. 플러그인 등록 (plugin.rs)
   ├── app.insert_resource(...)
   ├── app.add_message::<MyEvent>()
   └── app.add_systems(Schedule, systems)

4. 공개 API 노출
   └── mod.rs → pub use
```

**시스템 스케줄 선택 기준:**
- `FixedUpdate` + `before(PhysicsSet::SyncBackend)`: 물리 시뮬레이션 전 (힘 적용, 애니메이션)
- `FixedUpdate` + `after(PhysicsSet::Writeback)`: 물리 시뮬레이션 후 (충돌 감지, 게임 규칙)
- `Update`: 이벤트 처리, 명령 처리, 렌더링
- `PostUpdate`: 상태 동기화 (Bevy → Yew)

### Yew UI 개발 플로우

```
1. 라우트 추가 (페이지인 경우)
   └── routes.rs → Route enum에 variant 추가

2. 컴포넌트 생성
   ├── #[derive(Properties, PartialEq)] Props 정의
   └── #[function_component] 함수 작성

3. 훅 생성 (상태 관리 필요시)
   ├── 단순 상태: use_state
   ├── 복잡 상태: use_reducer + Reducible 구현
   └── Bevy 상태 폴링: use_effect_with + Interval

4. 스타일 추가
   └── style.css (BEM 컨벤션)

5. 내보내기
   └── mod.rs → pub use
```

### Yew ↔ Bevy 통신 패턴

**Yew → Bevy (명령 전달):**
```
send_command(JSON) → CommandQueue → process_commands() → 이벤트 발행
```

**Bevy → Yew (상태 동기화):**
```
ECS 상태 → sync_*_to_stores() → StateStores → get_*() WASM 함수 → use_bevy_*() 훅 폴링
```

> **⚠️ 핵심 주의사항: Yew 상태 변경 시 Bevy 동기화 필수**
>
> Yew에서 게임 상태를 변경하는 모든 콜백(add, delete, paste, update, mirror 등)은 **반드시** `send_command()`를 통해 Bevy에도 동기화해야 합니다.
>
> - Yew reducer만 업데이트하고 Bevy에 명령을 보내지 않으면, UI에는 반영되지만 캔버스에는 새로고침 전까지 반영되지 않음
> - 콜백 구현 시 체크리스트:
>   1. `send_command()`로 Bevy에 명령 전송
>   2. `state.dispatch()`로 Yew reducer 업데이트
> - 참고: `use_editor_state.rs`의 `on_paste`, `on_add`, `on_delete`, `on_mirror_x`, `on_mirror_y` 구현 참조

**새 명령 추가 시:**
1. `resources.rs`의 `GameCommand` enum에 variant 추가
2. `wasm_entry.rs`의 `send_command()`에 JSON 파싱 추가
3. `systems/command.rs`의 `process_commands()`에 처리 로직 추가

**새 상태 동기화 추가 시:**
1. `state_store.rs`에 Store 구조체 추가 (버전 필드 포함)
2. `StateStores`에 새 Store 필드 추가
3. `wasm_entry.rs`에 `get_*()` / `get_*_version()` WASM 함수 추가
4. `systems/state_sync.rs`에 동기화 시스템 추가
5. `hooks/`에 `use_bevy_*()` 훅 추가

### 맵 오브젝트 추가 플로우

```
1. 데이터 정의 (marble-core/src/map.rs)
   ├── ObjectRole enum에 variant 추가 (필요시)
   ├── Shape enum에 variant 추가 (새 도형)
   └── ObjectProperties에 Option<NewProperties> 필드 추가

2. ECS 컴포넌트 (bevy/components.rs)
   └── #[derive(Component)] 새 컴포넌트

3. 스폰 로직 (bevy/systems/map_loader.rs)
   └── handle_load_map()에 spawn 분기 추가

4. 관련 시스템 구현
   ├── physics.rs (힘 적용)
   ├── rendering.rs (시각화)
   └── 기타 상호작용 시스템
```

### gRPC 서비스 추가 플로우

```
1. Proto 정의
   └── proto/*.proto에 service/message 추가

2. 빌드
   └── just build-proto

3. 서버 구현 (marble-server)
   ├── handler/*.rs에 #[tonic::async_trait] 구현
   └── main.rs에 서비스 등록

4. 클라이언트 호출 (marble-client)
   └── services/*.rs에서 tonic-web-wasm-client로 호출
```

## 프로젝트 관리

- [GitHub Project Board](https://github.com/users/egoavara/projects/3)
- [GitHub Issues](https://github.com/egoavara/marble-live/issues)
