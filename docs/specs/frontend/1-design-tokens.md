# 设计令牌表（AppTheme → CSS variables）

> 来源:`UI/AppTheme.swift` 全文。上游 `AGENTS.md` 强制「所有 UI 样式必须用 AppTheme 常量，不得硬编码」。OpenTake 前端同样:**全部令牌定义为 CSS 变量，组件只引用变量。**
> 颜色:上游用 0–255 或 0–1 的 sRGB。下方给出等价 `rgb()/rgba()`。深色主题(上游唯一主题,不做浅色)。

### 1.1 背景 Background（`AppTheme.swift:8-23`）

| Token | 上游值 | CSS 变量 | 值 |
|---|---|---|---|
| base | rgb(10,10,10) | `--bg-base` | `rgb(10,10,10)` |
| surface | rgb(22,22,22) | `--bg-surface` | `rgb(22,22,22)` |
| raised | rgb(30,30,30) | `--bg-raised` | `rgb(30,30,30)` |
| prominent | rgb(44,44,44) | `--bg-prominent` | `rgb(44,44,44)` |
| placeholder | = raised | `--bg-placeholder` | `rgb(30,30,30)` |
| previewCanvas | black | `--bg-preview-canvas` | `#000` |

### 1.2 边框 Border（`AppTheme.swift:27-43`）

| Token | 上游值 | CSS 变量 |
|---|---|---|
| primary | white α0.16 | `--border-primary: rgba(255,255,255,0.16)` |
| subtle | white α0.12 | `--border-subtle: rgba(255,255,255,0.12)` |
| divider | white α0.44 | `--border-divider: rgba(255,255,255,0.44)` |
| width.hairline | 0.5 | `--bw-hairline: 0.5px` |
| width.thin | 1 | `--bw-thin: 1px` |
| width.medium | 1.5 | `--bw-medium: 1.5px` |
| width.thick | 2 | `--bw-thick: 2px` |

> 注:hairline `0.5px` 在非 retina 会被取整;按上游意图保留 0.5px,由浏览器/缩放处理。

### 1.3 文本 Text（`AppTheme.swift:104-114`）

| Token | 值 | CSS 变量 |
|---|---|---|
| primary | white α1.0 | `--text-primary: rgba(255,255,255,1)` |
| secondary | white α0.80 | `--text-secondary: rgba(255,255,255,0.80)` |
| tertiary | white α0.62 | `--text-tertiary: rgba(255,255,255,0.62)` |
| muted | white α0.34 | `--text-muted: rgba(255,255,255,0.34)` |

### 1.4 强调色 Accent / 状态 / 玻璃（`AppTheme.swift:47-100`）

| Token | 值 | CSS 变量 |
|---|---|---|
| accent.timecode | rgb(242,153,51) ≈ (0.95,0.6,0.2) | `--accent-timecode: rgb(242,153,51)` |
| accent.primary（暖米白）| rgb(245,239,228) ≈ (0.961,0.937,0.894) | `--accent-primary: rgb(245,239,228)` |
| accent.spotlight | rgb(255,69,69) | `--accent-spotlight: rgb(255,69,69)`（仅 Tour） |
| status.error | rgb(229,79,79) (#E54F4F) | `--status-error: rgb(229,79,79)` |
| glass.primaryTint | accent.primary α0.05 | `--glass-tint: rgba(245,239,228,0.05)` |

- **aiGradient**（银色微光，`AppTheme.swift:68-77`）：`linear-gradient(135deg, #fff 0%, #c7c7c7 45%, #999 55%, #fff 100%)`（white 1.0/0.78/0.60/1.0）。用于 AI 相关按钮/标签（Agent 气泡图标、Generate 按钮、AI Edit tab）。CSS 变量 `--ai-gradient`。
- spotlightGradient（仅 Tour，`AppTheme.swift:56-64`）：`linear-gradient(135deg, rgb(255,87,77), rgb(242,38,71), rgb(255,122,56))`。

### 1.5 轨道类型色 TrackColor（`AppTheme.swift:133-139`；`ClipType.themeColor` 映射 `AppTheme.swift:307-317`）

| ClipType | 值 | CSS 变量 |
|---|---|---|
| video | rgb(0,145,194) (#0091C2) | `--track-video: rgb(0,145,194)` |
| audio | rgb(88,168,34) (#58A822) | `--track-audio: rgb(88,168,34)` |
| image | rgb(183,45,210) (#B72DD2) | `--track-image: rgb(183,45,210)` |
| text | rgb(183,45,210) (#B72DD2)（同 image）| `--track-text: rgb(183,45,210)` |
| lottie | rgb(224,168,0) (#E0A800) | `--track-lottie: rgb(224,168,0)` |

### 1.6 圆角 Radius（`AppTheme.swift:143-155`）

| Token | px | 变量 |
|---|---|---|
| xs | 3 | `--radius-xs` |
| xsSm | 4 | `--radius-xs-sm` |
| sm | 6 | `--radius-sm` |
| md | 10 | `--radius-md` |
| mdLg | 12 | `--radius-md-lg` |
| lg | 14 | `--radius-lg` |
| xl | 20 | `--radius-xl` |

> `concentric(outer,padding)=max(outer-padding,0)`（`AppTheme.swift:152-154`）—— 嵌套圆角时用，TS 写工具函数。

### 1.7 间距 Spacing（`AppTheme.swift:159-171`）

| Token | px | 变量 |
|---|---|---|
| xxs | 2 | `--space-xxs` |
| xs | 4 | `--space-xs` |
| sm | 6 | `--space-sm` |
| smMd | 8 | `--space-sm-md` |
| md | 10 | `--space-md` |
| mdLg | 12 | `--space-md-lg` |
| lg | 14 | `--space-lg` |
| lgXl | 16 | `--space-lg-xl` |
| xl | 20 | `--space-xl` |
| xlXxl | 24 | `--space-xl-xxl` |
| xxl | 28 | `--space-xxl` |

### 1.8 字号 FontSize（`AppTheme.swift:175-188`）+ 字重/字距

| Token | px | 变量 |
|---|---|---|
| micro | 8 | `--fs-micro` |
| xxs | 9 | `--fs-xxs` |
| xs | 10 | `--fs-xs` |
| sm | 11 | `--fs-sm` |
| smMd | 12 | `--fs-sm-md` |
| md | 13 | `--fs-md` |
| mdLg | 14 | `--fs-md-lg` |
| lg | 15 | `--fs-lg` |
| xl | 18 | `--fs-xl` |
| title1 | 22 | `--fs-title1` |
| title2 | 28 | `--fs-title2` |
| display | 36 | `--fs-display` |

- **字重**（`AppTheme.swift:192-198`）：light/regular/medium(500)/semibold(600)/bold(700)。
- **字距 Tracking**（`AppTheme.swift:202-206`）：tight `-0.5px` / normal `0` / wide `1.5px`（wide 用于全大写小标题 `letter-spacing`）。
- **字体族**：上游用 `NSFont.systemFont`(San Francisco)。前端用系统 UI 字体栈：`-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif`。需 `monospacedDigit` 处用 `font-variant-numeric: tabular-nums`（时间码、可拖拽数字字段）。bracket 按钮用等宽（§4）。Text 字形按钮用 serif（`ToolbarView.swift:103`）。
- **等宽数字字体**：刻度时间码用 `NSFont.monospacedDigitSystemFont`（`TimelineRuler.swift:34`）→ `font-variant-numeric: tabular-nums`。

### 1.9 图标尺寸 IconSize（square frame，`AppTheme.swift:210-220`）

| Token | px | Token | px |
|---|---|---|---|
| xxs | 12 | mdLg | 24 |
| xs | 14 | lg | 26 |
| sm | 18 | lgXl | 28 |
| smMd | 20 | xl | 30 |
| md | 22 | | |

> 上游图标是 SF Symbols。前端**无 SF Symbols**:用一套等价图标库(推荐 Lucide / Phosphor / SF Symbols 风格的 SVG 集),§3.3 给出每个 SF Symbol → 替代图标的映射表。图标"frame"= 容器方框边长(用于 hover 命中区),`font-size`≈frame*0.55(SF Symbol 默认),具体逐处按上游 `.font(.system(size:))` 值设。

### 1.10 不透明度 Opacity（`AppTheme.swift:118-129`）

| Token | 值 | Token | 值 |
|---|---|---|---|
| opaque | 1 | muted | 0.15 |
| subtle | 0.04 | moderate | 0.25 |
| hint | 0.06 | medium | 0.35 |
| faint | 0.08 | strong | 0.55 |
| soft | 0.10 | prominent | 0.80 |

### 1.11 阴影 Shadow（`AppTheme.swift:274-278`，签名 `267-272`）

| Token | color/radius/x/y | CSS（box-shadow） |
|---|---|---|
| sm | black α0.3 / r1 / 0 / 0.5 | `0 0.5px 1px rgba(0,0,0,0.3)` |
| md | black α0.3 / r4 / 0 / 2 | `0 2px 4px rgba(0,0,0,0.3)` |
| lg | black α0.25 / r24 / 0 / 8 | `0 8px 24px rgba(0,0,0,0.25)` |

> SwiftUI shadow `radius` ≈ CSS blur。x/y = 偏移。

### 1.12 动画时长 Anim（`AppTheme.swift:282-285`）

| Token | 值 | 变量 |
|---|---|---|
| hover | 0.15s | `--anim-hover: 150ms` |
| transition | 0.2s | `--anim-transition: 200ms` |

- hover 用 `easeOut`（`HoverHighlight.swift:27`）；面板焦点环用 `easeOut`（`EditorView.swift:394`）；tab/面板切换用 `easeInOut`（多处）。CSS：`transition: ... var(--anim-hover) ease-out`。

### 1.13 复合/局部尺寸常量（散落，必须照搬）

| 用途 | 值 | 来源 |
|---|---|---|
| 面板标题栏高 panelHeaderHeight | 28 | `Constants.swift:40` |
| 工具条高 toolbarHeight | 38 | `Constants.swift:41` |
| 面板间隙 panelGap | 5 | `Constants.swift:43` |
| MediaPanel 标签轨宽 tabRailWidth | IconSize.lg(26)+Spacing.sm(6)*2 = 38 | `AppTheme.swift:261` |
| MediaPanel contextRowHeight | IconSize.md = 22 | `AppTheme.swift:262` |
| 项目卡片 | 150×120 | `AppTheme.swift:226-227` |
| 字幕预览最大高 | 150 | `AppTheme.swift:223` |
| 工具图片预览最大高 | 50 | `AppTheme.swift:225` |
| Caption 字号默认/最小/最大 | 48 / 12 / 300 | `AppTheme.swift:240-242` |
| Caption 中心吸附阈值 | 0.02 | `AppTheme.swift:246` |
| Caption 默认中心 Y | 0.9 | `AppTheme.swift:247` |
| 生成面板 referenceTile | 80×56 | `AppTheme.swift:257-258` |
| Inspector 关键帧 row 高 | 22 | `KeyframesLane.swift:8` |
| Inspector 关键帧 ruler/strip | 18 / 14（header=32）| `KeyframesLane.swift:5-7` |
| Inspector 关键帧 diamond | 8 | `KeyframesLane.swift:12` |
