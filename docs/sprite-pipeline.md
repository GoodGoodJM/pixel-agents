# Sprite Pipeline

Pixellab(https://www.pixellab.ai/)에서 스프라이트를 만들고, 어셈블 스크립트로 엔진용 스프라이트시트에 합치는 파이프라인.

## 빠른 시작

```bash
# 1. raw-sprites/ 폴더에 이미지 배치 (아래 가이드 참고)
# 2. 한 번에 전부 조립
npm run assemble

# 또는 개별 실행
npm run assemble:characters
npm run assemble:floors
npm run assemble:walls
```

---

## 캐릭터 (Characters)

### Pixellab 설정

| 항목 | 값 |
|------|-----|
| 캔버스 크기 | **16 x 32 px** |
| 스타일 | Top-down RPG character |
| 방향 | 아래(front), 위(back), 오른쪽(right) |

> 왼쪽은 오른쪽을 런타임에 좌우 반전해서 사용하므로 만들 필요 없음.

### 만들어야 할 프레임 (캐릭터당 21장)

각 방향마다 7프레임:

| 프레임 | 용도 | 설명 |
|--------|------|------|
| `walk1` | 걷기 1 | 왼발 앞 |
| `walk2` | 서 있기/걷기 2 | 기본 스탠딩 포즈 (idle에도 사용) |
| `walk3` | 걷기 3 | 오른발 앞 |
| `type1` | 타이핑 1 | 앉아서 키보드 치는 모션 1 |
| `type2` | 타이핑 2 | 앉아서 키보드 치는 모션 2 |
| `read1` | 읽기 1 | 책/모니터 보는 모션 1 |
| `read2` | 읽기 2 | 책/모니터 보는 모션 2 |

### 파일 배치

```
raw-sprites/characters/char_0/
  down_walk1.png     # 아래 방향 걷기 1
  down_walk2.png     # 아래 방향 걷기 2 (= idle 포즈)
  down_walk3.png     # 아래 방향 걷기 3
  down_type1.png     # 아래 방향 타이핑 1
  down_type2.png     # 아래 방향 타이핑 2
  down_read1.png     # 아래 방향 읽기 1
  down_read2.png     # 아래 방향 읽기 2
  up_walk1.png       # 위 방향 걷기 1
  up_walk2.png
  up_walk3.png
  up_type1.png
  up_type2.png
  up_read1.png
  up_read2.png
  right_walk1.png    # 오른쪽 방향 걷기 1
  right_walk2.png
  right_walk3.png
  right_type1.png
  right_type2.png
  right_read1.png
  right_read2.png
```

- 폴더 이름이 곧 출력 파일명: `char_0/` → `char_0.png`
- 최대 6개 팔레트 (char_0 ~ char_5). 그 이상은 런타임에 hue shift 적용됨.
- 누락된 프레임은 빈 칸으로 처리됨 (경고 출력).

### 출력

```
webview-ui/public/assets/characters/char_0.png  (112 x 96)
```

112 x 96 스프라이트시트:
```
         walk1  walk2  walk3  type1  type2  read1  read2
         [16px] [16px] [16px] [16px] [16px] [16px] [16px]
down  [32px] ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┐
             │      │      │      │      │      │      │      │
             └──────┴──────┴──────┴──────┴──────┴──────┴──────┘
up    [32px] ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┐
             │      │      │      │      │      │      │      │
             └──────┴──────┴──────┴──────┴──────┴──────┴──────┘
right [32px] ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┐
             │      │      │      │      │      │      │      │
             └──────┴──────┴──────┴──────┴──────┴──────┴──────┘
```

---

## 바닥 타일 (Floors)

### Pixellab 설정

| 항목 | 값 |
|------|-----|
| 캔버스 크기 | **16 x 16 px** |
| 스타일 | 타일링 가능한 바닥 패턴 |
| 색상 | **그레이스케일** (런타임에 HSBC 컬러라이즈 적용) |

> 반드시 그레이스케일로! 에디터에서 Hue/Saturation/Brightness/Contrast 슬라이더로 색을 입힘.

### 파일 배치

```
raw-sprites/floors/
  floor_0.png    # 패턴 0 (예: 나무 바닥)
  floor_1.png    # 패턴 1 (예: 타일)
  floor_2.png    # 패턴 2 ...
  ...
  floor_6.png    # 패턴 6 (기본 7개)
```

- 번호는 0부터 순서대로.
- 기본 7개. 개수를 바꾸면 코드에서 `FLOOR_PATTERN_COUNT` 수정 필요.

### 출력

```
webview-ui/public/assets/floors.png  (112 x 16, 기본 7패턴 기준)
```

가로로 이어붙인 strip:
```
[floor_0][floor_1][floor_2][floor_3][floor_4][floor_5][floor_6]
  16x16    16x16    16x16    16x16    16x16    16x16    16x16
```

---

## 벽 타일 (Walls)

### Pixellab 설정

| 항목 | 값 |
|------|-----|
| 캔버스 크기 | **16 x 32 px** |
| 구성 | 위 16px = 3D 입면, 아래 16px = 평면 |
| 색상 | **그레이스케일** (런타임에 HSBC 컬러라이즈 적용) |

벽은 인접 벽 방향에 따라 자동으로 타일이 선택됨 (auto-tiling). 4비트 비트마스크로 16가지 조합:

### 비트마스크 가이드

```
bit 0 (1) = 북쪽에 벽 있음
bit 1 (2) = 동쪽에 벽 있음
bit 2 (4) = 남쪽에 벽 있음
bit 3 (8) = 서쪽에 벽 있음
```

| 파일 | 값 | 연결 방향 | 설명 |
|------|-----|-----------|------|
| `wall_0.png` | 0000 | 없음 | 독립된 벽 기둥 |
| `wall_1.png` | 0001 | N | 북쪽만 연결 |
| `wall_2.png` | 0010 | E | 동쪽만 연결 |
| `wall_3.png` | 0011 | N+E | 북+동 코너 |
| `wall_4.png` | 0100 | S | 남쪽만 연결 |
| `wall_5.png` | 0101 | N+S | 남북 직선 |
| `wall_6.png` | 0110 | E+S | 동+남 코너 |
| `wall_7.png` | 0111 | N+E+S | T자 (서쪽 빠짐) |
| `wall_8.png` | 1000 | W | 서쪽만 연결 |
| `wall_9.png` | 1001 | N+W | 북+서 코너 |
| `wall_10.png` | 1010 | E+W | 동서 직선 |
| `wall_11.png` | 1011 | N+E+W | T자 (남쪽 빠짐) |
| `wall_12.png` | 1100 | S+W | 남+서 코너 |
| `wall_13.png` | 1101 | N+S+W | T자 (동쪽 빠짐) |
| `wall_14.png` | 1110 | E+S+W | T자 (북쪽 빠짐) |
| `wall_15.png` | 1111 | N+E+S+W | 십자 교차 |

### 파일 배치

```
raw-sprites/walls/
  wall_0.png     # 독립 기둥
  wall_1.png     # 북 연결
  ...
  wall_15.png    # 모든 방향 연결
```

### 출력

```
webview-ui/public/assets/walls.png  (64 x 128)
```

4x4 그리드, 각 셀 16x32:
```
     col0     col1     col2     col3
    ┌────────┬────────┬────────┬────────┐
r0  │ wall_0 │ wall_1 │ wall_2 │ wall_3 │  32px
    ├────────┼────────┼────────┼────────┤
r1  │ wall_4 │ wall_5 │ wall_6 │ wall_7 │  32px
    ├────────┼────────┼────────┼────────┤
r2  │ wall_8 │ wall_9 │wall_10 │wall_11 │  32px
    ├────────┼────────┼────────┼────────┤
r3  │wall_12 │wall_13 │wall_14 │wall_15 │  32px
    └────────┴────────┴────────┴────────┘
      16px     16px     16px     16px
```

---

## 전체 폴더 구조

```
raw-sprites/                          ← 원본 (git에 포함 안 됨)
├── characters/
│   ├── char_0/                       ← 캐릭터 0 (21장)
│   │   ├── down_walk1.png
│   │   ├── down_walk2.png
│   │   └── ...
│   ├── char_1/                       ← 캐릭터 1
│   └── ...
├── floors/
│   ├── floor_0.png
│   └── ...
└── walls/
    ├── wall_0.png
    └── ...

webview-ui/public/assets/             ← 빌드 결과 (git에 포함)
├── characters/
│   ├── char_0.png                    ← 112x96 스프라이트시트
│   └── ...
├── floors.png                        ← Nx16 strip
└── walls.png                         ← 64x128 그리드
```

---

## Pixellab 워크플로우 팁

### 캐릭터 만들기

1. Pixellab에서 16x32 캔버스로 캐릭터 기본 포즈 생성
2. 같은 캐릭터로 방향별/애니메이션별 변형 생성
3. 각 프레임을 `{방향}_{애니메이션}.png`로 저장
4. `walk2`가 기본 서 있는 포즈 — 가장 자연스러운 스탠딩으로
5. `type1`/`type2`는 앉아서 타이핑하는 모습 (의자에 앉힘)
6. `read1`/`read2`는 모니터를 보는 모습

### 바닥/벽 만들기

1. 그레이스케일로 제작 (명암만 표현)
2. 바닥은 타일링이 자연스럽게 — 가장자리가 이어지도록
3. 벽은 위 16px에 입체감 있는 면, 아래 16px에 평면도 표현
4. 벽 연결 부분이 자연스럽게 이어지도록 주의

### 빌드 후

```bash
npm run assemble     # 스프라이트시트 생성
npm run build        # 앱 빌드 (또는 dev 서버가 자동 반영)
```
