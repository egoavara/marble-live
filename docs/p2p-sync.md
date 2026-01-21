# P2P 동기화 상세

## 핵심 원칙

1. **결정론적 시뮬레이션**: 동일 입력 → 동일 결과
2. **Rapier 설정**: `enhanced-determinism` 기능 필수
3. **고정 타임스텝**: 60Hz (1/60초)
4. **결정론적 RNG**: `rand_chacha::ChaCha8Rng`
5. **메시지 직렬화**: protobuf

## 호스트 자동 선출

가장 낮은 PeerId를 가진 피어가 호스트:

```rust
fn elect_host(peers: &[PeerId], my_id: PeerId) -> bool {
    let all_ids: Vec<_> = peers.iter().chain(std::iter::once(&my_id)).collect();
    let min_id = all_ids.iter().min().unwrap();
    *min_id == &my_id
}
```

호스트 역할:
- 게임 시작 시 seed 생성 및 브로드캐스트
- 초기 상태 배포

## 동기화 프로토콜

### 게임 시작
1. 모든 피어가 Ready 메시지 전송
2. 각 피어가 호스트 선출
3. 호스트가 GameStart 브로드캐스트 (seed, 초기 상태)
4. 동시에 시뮬레이션 시작

### 프레임 동기화
1. 각 클라이언트가 독립적으로 시뮬레이션 실행
2. 매 60프레임마다 FrameHash 메시지 교환
3. 해시 불일치 감지 시 재동기화 트리거

### 재동기화
1. 다수결로 "정답" 해시 결정
2. 소수파가 SyncRequest 전송
3. 다수파가 SyncState 응답
4. 소수파가 상태 복원 후 재개

## 해시 계산

```rust
impl PhysicsWorld {
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (_, rb) in self.rigid_body_set.iter() {
            rb.translation().x.to_bits().hash(&mut hasher);
            rb.translation().y.to_bits().hash(&mut hasher);
            rb.rotation().angle().to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
}
```

## 피어 이탈 처리

- 호스트 이탈: 남은 피어 중 재선출
- 일반 피어 이탈: 해당 구슬은 시뮬레이션에서 유지
