# Flag Patterns Reference

국가별 깃발 SVG 패턴 레퍼런스. `layer-flag`에 적용하며, 반드시 `clip-path="url(#body-clip)"`으로 원형 클리핑.

## 가로 줄무늬 (Horizontal Stripes)

### Poland (폴란드)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="128" fill="#FFFFFF"/>
  <rect x="0" y="128" width="256" height="128" fill="#DC143C"/>
</g>
```

### Germany (독일)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="85" fill="#000000"/>
  <rect x="0" y="85" width="256" height="86" fill="#DD0000"/>
  <rect x="0" y="171" width="256" height="85" fill="#FFCC00"/>
</g>
```

### Russia (러시아)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="85" fill="#FFFFFF"/>
  <rect x="0" y="85" width="256" height="86" fill="#0039A6"/>
  <rect x="0" y="171" width="256" height="85" fill="#D52B1E"/>
</g>
```

### Netherlands (네덜란드)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="85" fill="#AE1C28"/>
  <rect x="0" y="85" width="256" height="86" fill="#FFFFFF"/>
  <rect x="0" y="171" width="256" height="85" fill="#21468B"/>
</g>
```

### Austria (오스트리아)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="85" fill="#ED2939"/>
  <rect x="0" y="85" width="256" height="86" fill="#FFFFFF"/>
  <rect x="0" y="171" width="256" height="85" fill="#ED2939"/>
</g>
```

### Hungary (헝가리)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="85" fill="#CE2939"/>
  <rect x="0" y="85" width="256" height="86" fill="#FFFFFF"/>
  <rect x="0" y="171" width="256" height="85" fill="#477050"/>
</g>
```

## 세로 줄무늬 (Vertical Stripes)

### France (프랑스)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="85" height="256" fill="#002395"/>
  <rect x="85" y="0" width="86" height="256" fill="#FFFFFF"/>
  <rect x="171" y="0" width="85" height="256" fill="#ED2939"/>
</g>
```

### Italy (이탈리아)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="85" height="256" fill="#009246"/>
  <rect x="85" y="0" width="86" height="256" fill="#FFFFFF"/>
  <rect x="171" y="0" width="85" height="256" fill="#CE2B37"/>
</g>
```

### Ireland (아일랜드)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="85" height="256" fill="#169B62"/>
  <rect x="85" y="0" width="86" height="256" fill="#FFFFFF"/>
  <rect x="171" y="0" width="85" height="256" fill="#FF883E"/>
</g>
```

### Belgium (벨기에)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="85" height="256" fill="#000000"/>
  <rect x="85" y="0" width="86" height="256" fill="#FAE042"/>
  <rect x="171" y="0" width="85" height="256" fill="#ED2939"/>
</g>
```

## 십자/특수 패턴

### Japan (일본)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="256" fill="#FFFFFF"/>
  <circle cx="128" cy="128" r="50" fill="#BC002D"/>
</g>
```

### South Korea (한국)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="256" fill="#FFFFFF"/>
  <!-- 태극 (간략화) -->
  <circle cx="128" cy="128" r="40" fill="#CD2E3A"/>
  <path d="M128,88 A40,40 0 0,1 128,168 A20,20 0 0,1 128,128 A20,20 0 0,0 128,88" fill="#0047A0"/>
  <!-- 건곤감리는 복잡하므로 간략화하거나 생략 가능 -->
</g>
```

### USA (미국)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <!-- 빨간/흰 줄무늬 -->
  <rect x="0" y="0" width="256" height="256" fill="#B22234"/>
  <rect x="0" y="20" width="256" height="20" fill="#FFFFFF"/>
  <rect x="0" y="59" width="256" height="20" fill="#FFFFFF"/>
  <rect x="0" y="98" width="256" height="20" fill="#FFFFFF"/>
  <rect x="0" y="138" width="256" height="20" fill="#FFFFFF"/>
  <rect x="0" y="177" width="256" height="20" fill="#FFFFFF"/>
  <rect x="0" y="216" width="256" height="20" fill="#FFFFFF"/>
  <!-- 파란 사각형 -->
  <rect x="0" y="0" width="102" height="138" fill="#3C3B6E"/>
</g>
```

### UK (영국)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="256" fill="#012169"/>
  <!-- 대각선 (St. Andrew/St. Patrick) -->
  <line x1="0" y1="0" x2="256" y2="256" stroke="#FFFFFF" stroke-width="30"/>
  <line x1="256" y1="0" x2="0" y2="256" stroke="#FFFFFF" stroke-width="30"/>
  <line x1="0" y1="0" x2="256" y2="256" stroke="#C8102E" stroke-width="15"/>
  <line x1="256" y1="0" x2="0" y2="256" stroke="#C8102E" stroke-width="15"/>
  <!-- 십자 (St. George) -->
  <rect x="108" y="0" width="40" height="256" fill="#FFFFFF"/>
  <rect x="0" y="108" width="256" height="40" fill="#FFFFFF"/>
  <rect x="113" y="0" width="30" height="256" fill="#C8102E"/>
  <rect x="0" y="113" width="256" height="30" fill="#C8102E"/>
</g>
```

### Sweden (스웨덴)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="256" fill="#006AA7"/>
  <rect x="80" y="0" width="35" height="256" fill="#FECC00"/>
  <rect x="0" y="111" width="256" height="35" fill="#FECC00"/>
</g>
```

### Switzerland (스위스)
```xml
<g id="layer-flag" clip-path="url(#body-clip)">
  <rect x="0" y="0" width="256" height="256" fill="#FF0000"/>
  <rect x="108" y="58" width="40" height="140" fill="#FFFFFF"/>
  <rect x="58" y="108" width="140" height="40" fill="#FFFFFF"/>
</g>
```

## 주의사항

- 모든 flag 패턴은 256x256 캔버스 기준
- `body-clip` clipPath로 원형 마스킹 필수
- 복잡한 문양(문장, 별 등)은 간략화하여 Polandball 스타일에 맞춤
- 색상은 공식 국기 색상 코드 사용
