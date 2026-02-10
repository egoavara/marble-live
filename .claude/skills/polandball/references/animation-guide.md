# Animation Guide

Polandball 캐릭터의 파츠별 애니메이션 변형 가이드.

> **핵심 규칙: 눈은 흰자(sclera)만으로 구성한다. 동공(pupil)은 절대 그리지 않는다.**
> 레퍼런스 이미지(`references/polandball.png`)를 반드시 확인할 것.

## 파츠별 변형 방법

### Body (layer-body + layer-flag + layer-outline)

**Squash/Stretch (찌그러짐/늘어남)**

원래 `<circle>`을 `<ellipse>`로 교체하여 표현:

```xml
<!-- 기본 -->
<circle cx="128" cy="128" r="118"/>

<!-- Squash (납작) -->
<ellipse cx="128" cy="134" rx="122" ry="112"/>

<!-- Stretch (길쭉) -->
<ellipse cx="128" cy="124" rx="114" ry="122"/>
```

주의: body를 ellipse로 변경할 때 `body-clip`, `layer-outline`도 함께 변경해야 함.

**Roll (회전)**

`layer-flag`에 transform 적용:

```xml
<g id="layer-flag" clip-path="url(#body-clip)" transform="rotate(5, 128, 128)">
```

### Eyes (layer-eyes)

**눈은 흰자(sclera)만으로 구성한다. 동공(pupil)은 절대 그리지 않는다.**
눈 = 흰색 타원(`fill="#FFF"`) + 검은 외곽선(`stroke="#000"`)뿐이다.

**Blink (깜빡임)**

ellipse의 ry를 줄여서 표현:

```xml
<!-- 열린 눈 -->
<ellipse cx="98" cy="130" rx="18" ry="12" fill="#FFF" stroke="#000" stroke-width="3"/>

<!-- 반쯤 감은 눈 -->
<ellipse cx="98" cy="130" rx="18" ry="6" fill="#FFF" stroke="#000" stroke-width="3"/>

<!-- 감은 눈 -->
<ellipse cx="98" cy="131" rx="18" ry="1" fill="#FFF" stroke="#000" stroke-width="3"/>
```

**Wide Eyes (놀란 눈)**

흰자 ellipse를 확대:

```xml
<ellipse cx="98" cy="130" rx="20" ry="14" fill="#FFF" stroke="#000" stroke-width="3"/>
```

**Squint (찡그린 눈)**

흰자 ellipse를 축소:

```xml
<ellipse cx="98" cy="130" rx="16" ry="8" fill="#FFF" stroke="#000" stroke-width="3"/>
```

### Eyebrows (layer-eyebrows)

**표정별 각도 변형:**

```xml
<!-- 기본 (약간 기울어진) -->
<line x1="82" y1="118" x2="112" y2="122"/>
<line x1="144" y1="122" x2="174" y2="118"/>

<!-- 화남 (안쪽이 아래로) -->
<line x1="82" y1="122" x2="112" y2="114"/>
<line x1="144" y1="114" x2="174" y2="122"/>

<!-- 슬픔 (안쪽이 위로) -->
<line x1="82" y1="114" x2="112" y2="122"/>
<line x1="144" y1="122" x2="174" y2="114"/>

<!-- 놀람 (높이 올림) -->
<line x1="82" y1="110" x2="112" y2="112"/>
<line x1="144" y1="112" x2="174" y2="110"/>
```

### Mouth (layer-mouth)

**표정별 path 데이터:**

```xml
<!-- 미소 -->
<path d="M108,160 Q128,175 148,160" stroke="#000" stroke-width="3" fill="none"/>

<!-- 활짝 웃음 -->
<path d="M105,158 Q128,180 151,158" stroke="#000" stroke-width="3" fill="#FFF"/>

<!-- 놀란 입 -->
<ellipse cx="128" cy="162" rx="8" ry="10" fill="#000"/>

<!-- 슬픈 입 -->
<path d="M108,168 Q128,155 148,168" stroke="#000" stroke-width="3" fill="none"/>

<!-- 일자 입 (무표정) -->
<line x1="110" y1="160" x2="146" y2="160" stroke="#000" stroke-width="3"/>

<!-- 삐죽 입 (짜증) -->
<path d="M108,160 L118,158 L128,162 L138,156 L148,160" stroke="#000" stroke-width="2.5" fill="none"/>
```

### Accessories (layer-accessories)

**모자 위치 오프셋:**

body가 squash/stretch될 때 모자도 따라 이동:

```xml
<!-- 기본 위치 -->
<g id="layer-accessories">
  <g transform="translate(0, 0)"><!-- hat SVG --></g>
</g>

<!-- body가 아래로 이동 (squash) -->
<g id="layer-accessories">
  <g transform="translate(0, 4)"><!-- hat SVG --></g>
</g>
```

### Effects (layer-effects)

**그림자:**

```xml
<ellipse cx="128" cy="250" rx="80" ry="8" fill="rgba(0,0,0,0.15)"/>
```

**땀방울:**

```xml
<path d="M180,90 Q185,80 182,70 Q188,80 184,90 Z" fill="#87CEEB" opacity="0.8"/>
```

## 애니메이션 시퀀스

### Idle (4프레임, 8fps)

| Frame | body cy | body ry | others translate Y |
|-------|---------|---------|---------------------|
| 00 | 128 | 118 | 0 |
| 01 | 130 | 116 | +2 |
| 02 | 128 | 118 | 0 |
| 03 | 126 | 120 | -2 |

### Blink (3프레임, 12fps)

| Frame | eye ry | 비고 |
|-------|--------|------|
| 00 | 12 | 열린 눈 |
| 01 | 3 | 반감은 눈 |
| 02 | 12 | 다시 열린 눈 |

body, eyebrows, mouth는 변화 없음.

### Bounce (6프레임, 10fps)

| Frame | body ry | body cy | accessories Y |
|-------|---------|---------|---------------|
| 00 | 118 | 128 | 0 |
| 01 | 108 | 138 | +6 |
| 02 | 124 | 120 | -4 |
| 03 | 116 | 130 | +1 |
| 04 | 120 | 126 | -1 |
| 05 | 118 | 128 | 0 |
