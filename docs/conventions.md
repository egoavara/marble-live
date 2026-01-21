# 코드 컨벤션

## Rust

- `rustfmt` 기본 설정 사용
- `clippy` 경고 없이 유지
- 공개 API에는 문서 주석 (`///`) 작성

## 커밋 메시지

```
<type>(<scope>): <subject>
```

타입:
- `feat`: 새 기능
- `fix`: 버그 수정
- `refactor`: 리팩토링
- `docs`: 문서
- `test`: 테스트
- `chore`: 빌드/설정

예시:
```
feat(core): add PhysicsWorld hash computation
fix(client): resolve canvas resize issue
docs(readme): update quick start guide
```

## PR 규칙

- 관련 Issue 번호 연결 (`Closes #N`)
- CI 통과 필수
- 코드 리뷰 후 머지

## 브랜치 네이밍

```
feature/#N-short-description
fix/#N-short-description
```
