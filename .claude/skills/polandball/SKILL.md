---
name: polandball
description: Polandball 스타일 캐릭터 SVG 생성, 렌더링-검토-수정 반복 루프, 파츠별 분리 출력 및 스프라이트 시트 합성. "폴란드 공 만들기", "캐릭터 이미지 생성", "polandball", "SVG 캐릭터", "스프라이트 시트", "국가공 만들기" 요청 시 사용.
---

# Polandball SVG Character Creator

## Art Style

> **레퍼런스 이미지**: `references/polandball.png`을 반드시 참조할 것.

Polandball의 핵심 아트 스타일 규칙:

- **눈은 흰자(sclera)만으로 구성**. 동공(pupil)은 절대 그리지 않는다. 흰색 타원 + 검은 외곽선만 사용.
- 눈은 작고 가까이 붙어있으며, 국기의 경계선 근처에 위치
- 눈썹은 두껍고 표정을 결정하는 핵심 요소
- 몸은 완벽한 원형이 아닌 약간의 불규칙함이 자연스러움
- 외곽선은 두꺼운 검은색
- 국기 패턴은 원형 클리핑으로 몸에 입힘

## Directory Structure

스킬 전체가 `.claude/skills/polandball/` 하위에 자체 패키지로 존재한다.

```
.claude/skills/polandball/
├── SKILL.md                     # 이 파일 (워크플로우 가이드)
├── package.json                 # npm 의존성 및 스크립트
├── render.mjs                   # SVG → PNG 렌더러
├── preview.mjs                  # 애니메이션 프리뷰 (GIF/필름스트립)
├── spritesheet.mjs              # 스프라이트 시트 생성기
├── templates/_base.svg          # 기본 템플릿
├── characters/<country>/        # 국가별 캐릭터 SVG
│   ├── <country>_idle_00.svg
│   ├── <country>_idle_01.svg
│   └── ...
├── output/
│   ├── composite/               # 조합 렌더링 (검증용)
│   ├── preview/                 # GIF/필름스트립 프리뷰
│   ├── parts/                   # 파츠별 분리 PNG
│   └── sheets/                  # 스프라이트 시트 + 매니페스트
└── references/                  # 참고 문서
    ├── polandball.png       # 아트 스타일 레퍼런스 이미지
    ├── animation-guide.md
    └── flag-patterns.md
```

## Workflow

### Phase 1: 캐릭터 디자인 협의

**SVG 작업에 앞서, 사용자와 캐릭터의 특성을 충분히 논의한다.**

확인해야 할 항목:

1. **국가/깃발**: 어느 나라인가? (`references/flag-patterns.md` 참조)
2. **캐릭터 개성**: 기본 폴란드공 스타일인가, 아니면 고유한 특징이 있는가?
   - 예: 미국공 → 선글라스 + 입에 문 담배
   - 예: 영국공 → 모노클 + 탑햇 + 홍차
   - 예: 러시아공 → 우샨카 모자 + 보드카 병
   - 예: 독일공 → 피켈하우베 헬멧
3. **기본 표정**: 무표정? 화남? 자신만만? (눈썹 각도, 입 형태 결정)
4. **필요한 애니메이션**: idle, blink, bounce 등 어떤 것이 필요한가?
5. **프레임 수**: 애니메이션당 몇 프레임? (기본: idle 4프레임, blink 3프레임)

> 이 단계를 건너뛰지 말 것. 캐릭터 특성이 확정되어야 이후 반복 작업의 방향이 잡힌다.

### Phase 2: 기본 프레임 제작 (frame 00)

캐릭터의 **기본 포즈(idle_00)**를 먼저 완성한다. 이것이 모든 애니메이션 프레임의 원본이 된다.

**2-1. SVG 작성**

`templates/_base.svg`를 복사하여 캐릭터 SVG를 생성한다.

```
characters/<name>/<name>_idle_00.svg
```

SVG 내부 레이어 구조를 반드시 유지:
- `id="layer-body"` - 몸체 원형
- `id="layer-flag"` - 국기 패턴 (body-clip으로 클리핑)
- `id="layer-outline"` - 외곽선
- `id="layer-eyes"` - 눈 (흰자만, 동공 없음)
- `id="layer-eyebrows"` - 눈썹
- `id="layer-mouth"` - 입
- `id="layer-accessories"` - 악세서리 (선글라스, 모자, 담배 등)
- `id="layer-effects"` - 이펙트 (그림자, 연기 등)

**2-2. 렌더링 → AI 검토 → 사용자 보고**

> **중요: SVG를 만들거나 수정할 때마다 반드시 렌더링 → 사용자에게 이미지 보고 순서를 지킨다.**
> AI가 자체 판단으로 다음 단계로 넘어가지 말고, 항상 사용자에게 결과물을 보여주고 피드백을 받는다.

```bash
cd .claude/skills/polandball
npm run render -- characters/<name>/<name>_idle_00.svg
```

렌더링 완료 후:
1. **Read 도구로 composite PNG를 열어** AI가 먼저 체크리스트 확인
   - [ ] 국기 패턴이 올바른가
   - [ ] 눈이 흰자만으로 구성되었는가 (동공 없음)
   - [ ] 캐릭터 개성 요소가 잘 표현되었는가 (악세서리 등)
   - [ ] 외곽선이 깨끗한가
   - [ ] 전반적인 비율이 자연스러운가
2. **사용자에게 PNG를 보여주고 피드백 요청** ← 이 단계를 절대 건너뛰지 말 것
3. 사용자 피드백에 따라 SVG 수정 → 다시 렌더링 → 다시 보고 반복

### Phase 3: 애니메이션 프레임 제작

> **이 단계가 가장 핵심이다.** 스프라이트 시트를 만들려면 각 애니메이션의 프레임을
> 하나씩 조금씩 변형하여 여러 SVG를 만들어야 한다. 모든 프레임은 Phase 2의 기본
> 프레임을 원본으로 삼아 파츠별 속성을 조금씩 조정한 것이다.

**3-1. 프레임 생성**

1. 기본 프레임 SVG를 복사하여 `<name>_<anim>_<NN>.svg` 생성
2. `references/animation-guide.md`를 참고하여 해당 프레임의 변형 적용
3. 모든 프레임 SVG를 한 번에 작성

**예시: idle 애니메이션 (4프레임)**

| 프레임 | 파일 | 변형 내용 |
|--------|------|-----------|
| 00 | `usa_idle_00.svg` | 기본 포즈 (Phase 2에서 완성됨) |
| 01 | `usa_idle_01.svg` | body 살짝 아래 (squash), 악세서리 +2px 아래 |
| 02 | `usa_idle_02.svg` | 기본으로 복귀 (frame 00과 동일하거나 미세 차이) |
| 03 | `usa_idle_03.svg` | body 살짝 위 (stretch), 악세서리 -2px 위 |

**3-2. GIF 프리뷰 생성 → 사용자 보고**

> **프레임 SVG 작성 후, 반드시 GIF를 만들어서 사용자에게 보여준다.**
> 사용자 승인 없이 다음 단계(파츠 분리)로 넘어가지 않는다.

```bash
# 1. 모든 프레임 렌더링
npm run render -- characters/<name>/<name>_idle_00.svg \
  characters/<name>/<name>_idle_01.svg \
  characters/<name>/<name>_idle_02.svg \
  characters/<name>/<name>_idle_03.svg

# 2. GIF 프리뷰 생성 (체커보드 배경)
npm run preview -- <name> idle

# 3. (선택) 필름스트립으로 프레임 나란히 비교
npm run preview:filmstrip -- <name> idle

# 옵션: fps, 크기 조정
npm run preview -- <name> idle --fps=12 --size=128
```

**Read 도구로 GIF를 열어 사용자에게 보여주며 다음을 확인:**
- 프레임 전환이 매끄러운가?
- squash/stretch 변형량이 적절한가?
- 악세서리가 body와 함께 자연스럽게 움직이는가?

**사용자 피드백에 따라:**
- 수정 필요 → 해당 프레임 SVG 수정 → render → preview → 다시 보고
- 승인 → Phase 4로 진행

### Phase 4: 파츠 분리 및 스프라이트 시트 합성

모든 프레임이 완성되면 최종 출력.

**4-1. 파츠 분리 렌더링**

```bash
cd .claude/skills/polandball

# 모든 프레임을 파츠별로 분리
npm run render:parts -- characters/<name>/<name>_idle_00.svg
npm run render:parts -- characters/<name>/<name>_idle_01.svg
npm run render:parts -- characters/<name>/<name>_idle_02.svg
npm run render:parts -- characters/<name>/<name>_idle_03.svg
```

각 프레임에서 생성되는 파츠:
- `_body.png`, `_eyes.png`, `_eyebrows.png`, `_mouth.png`, `_accessories.png`, `_effects.png`

**4-2. 스프라이트 시트 합성**

```bash
npm run sheet -- <name>
```

결과: 파츠별 스프라이트 시트 + JSON 매니페스트

## Part Modification Guide

### body (layer-body + layer-flag + layer-outline)

| 속성 | 효과 | 예시 |
|------|------|------|
| circle r | 전체 크기 | r="118" (기본) |
| circle cx/cy | 위치 이동 | cy="132" (아래로) |
| rx/ry (ellipse 변환) | 찌그러짐 | rx="120" ry="110" (squash) |
| flag rect 높이 | 국기 비율 | height="130" (위 부분 확대) |

> **⚠️ 필수: body를 ellipse로 변형할 때 flag 패턴도 반드시 함께 변형해야 한다.**
>
> body만 ellipse로 바꾸고 flag 내용물(줄무늬, 별, 캔턴 등)을 그대로 두면,
> 몸 윤곽만 바뀌고 국기 패턴은 고정되어 부자연스러운 결과가 나온다.
>
> **해결 방법**: `layer-flag` 내부 콘텐츠를 `<g>` 로 감싸고 body와 동일한 변환 적용:
>
> ```xml
> <g id="layer-flag" clip-path="url(#body-clip)">
>   <g transform="translate(128,CY) scale(RX/118, RY/118) translate(-128,-128)">
>     <!-- 모든 flag 콘텐츠: 줄무늬, 캔턴, 별 등 -->
>   </g>
> </g>
> ```
>
> | body 상태 | CY | scale | 변환 예시 |
> |-----------|-----|-------|----------|
> | 기본 (circle r=118) | 128 | 1.0, 1.0 | 변환 불필요 |
> | squash (rx=120, ry=116) | 130 | 1.017, 0.983 | `translate(128,130) scale(1.017,0.983) translate(-128,-128)` |
> | stretch (rx=116, ry=120) | 126 | 0.983, 1.017 | `translate(128,126) scale(0.983,1.017) translate(-128,-128)` |
>
> 기본 프레임(원형)은 변환이 필요 없으므로 `<g>` 래핑 없이 그대로 둔다.

### eyes (layer-eyes)

**눈은 흰자만으로 구성. 동공(pupil)은 절대 추가하지 않는다.**

| 속성 | 효과 | 예시 |
|------|------|------|
| ellipse ry | 깜빡임 | ry="1" (감은 눈) |
| ellipse rx/ry | 눈 크기/형태 | rx="20" ry="14" (놀란 큰 눈) |
| 전체 g transform | 눈 위치 | translate(0, -5) (위로) |

### eyebrows (layer-eyebrows)

| 속성 | 효과 | 예시 |
|------|------|------|
| y1/y2 차이 | 각도 (표정) | y1="112" y2="124" (화남) |
| stroke-width | 굵기 | stroke-width="5" (강조) |
| 제거 | 무표정 | 빈 g 태그 |

### mouth (layer-mouth)

| 표정 | SVG |
|------|-----|
| 미소 | `<path d="M108,160 Q128,175 148,160" stroke="#000" stroke-width="3" fill="none"/>` |
| 놀람 | `<ellipse cx="128" cy="162" rx="8" ry="10" fill="#000"/>` |
| 슬픔 | `<path d="M108,168 Q128,155 148,168" stroke="#000" stroke-width="3" fill="none"/>` |
| 화남 | `<line x1="110" y1="160" x2="146" y2="160" stroke="#000" stroke-width="3"/>` |

### accessories (layer-accessories)

모자 등 악세서리는 캐릭터 원 바깥에 위치 가능. body-clip 적용하지 않음.

### effects (layer-effects)

그림자, 반짝임, 땀방울 등 이펙트. opacity로 강도 조절 가능.

> **⚠️ 필수: 연기·이펙트는 프레임마다 고유한 경로(path)를 가져야 한다.**
>
> 같은 path 데이터를 translate만 해서 재사용하면 이펙트가 고정된 것처럼 보인다.
> 특히 연기(smoke)는 프레임마다 다음을 변화시켜야 자연스럽다:
>
> 1. **경로 control point**: Q 커브의 x좌표를 ±2~4px 좌우로 흔들어 바람에 흔들리는 느낌
> 2. **경로 끝점 y좌표**: 프레임이 진행될수록 위로 올라가거나 변화시켜 상승하는 느낌
> 3. **opacity**: 0.15~0.47 범위에서 미세하게 달라지게 하여 농도 변화 표현
>
> ```
> Frame 00: M222,186 Q226,174 ... opacity=0.45  (기본 위치)
> Frame 02: M224,184 Q230,170 ... opacity=0.40  (오른쪽으로 흔들림)
> Frame 06: M219,188 Q222,175 ... opacity=0.47  (왼쪽으로 흔들림)
> ```

## Animation Frame Strategies

> **⚠️ 핵심 원칙: 모든 시각 요소가 각자의 방식으로 동시에 움직여야 자연스럽다.**
>
> 프레임을 만들 때 다음 요소를 **빠짐없이** 변형해야 한다:
>
> | 요소 | 변형 방법 | 빠뜨리면? |
> |------|-----------|-----------|
> | body (clipPath + 원형 + outline) | circle → ellipse (rx, ry, cy) | 몸 윤곽이 안 변함 |
> | **flag 패턴** | `translate(cx,cy) scale(rx/118,ry/118) translate(-128,-128)` | **줄무늬·별이 고정되어 부자연스러움** |
> | accessories | `transform="translate(0, deltaY)"` on layer-accessories | 선글라스/모자가 공중에 떠 있음 |
> | **effects (연기 등)** | **프레임마다 고유한 path 데이터 + opacity** | **연기가 정지해 보임** |
> | eyes/eyebrows/mouth | body 이동에 맞춰 translate | 얼굴이 몸과 분리됨 |

### Idle (호흡)
- 4~8프레임, 8fps
- body: cy를 +-2 범위에서 변형, ry를 116~120 범위에서 squash/stretch
- **flag 패턴**: body와 동일한 비율로 scale 변환 (위 핵심 원칙 참조)
- eyes/eyebrows/mouth: body에 맞춰 같이 이동 (transform translate)
- accessories: body에 맞춰 같이 이동 (transform translate)
- **effects (연기 등)**: 프레임마다 path control point ±2~4px 흔들기 + opacity 미세 변화

### Blink (깜빡임)
- 3프레임, 12fps
- eyes: ry를 12→3→12 변화
- 나머지 파츠: 변화 없음

### Bounce (튀기)
- 6프레임, 10fps
- body: squash→stretch→squash 주기적 변형
- **flag 패턴**: body와 동일한 비율로 scale 변환
- accessories: 위치 약간 지연 (follow-through)
- **effects**: 프레임마다 고유한 path + opacity

## File Naming Convention

SVG: `<character>_<animation>_<frame>.svg`
Parts PNG: `<character>_<animation>_<frame>_<part>.png`
Sheet: `<character>_<part>.png` + `<character>_<part>.json`
