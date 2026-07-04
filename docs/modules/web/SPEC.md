# 前端 React UI —— 1:1 复刻上游 实现就绪规格 (Issue #12)

> **状态**:v1 硬要求规格。**已有功能的 UI 与交互必须 1:1 复刻上游 palmier-pro。**
> **范围**:`web/`（React + TypeScript + Vite + Zustand）的全部可视层与交互层。**不含** Rust core / Tauri command 的内部实现（只定义对接契约）。
> **证据基准**:全部数值/行为均引自 `palmier-pro-upstream/Sources/PalmierPro/`，并标注 `文件:行号`。凡本规格与上游源码冲突，**以上游源码为准**。
> **方法**:上游本质是「单一可观测状态容器 `EditorViewModel` + 帧本位不可变值模型 `Timeline` + AppKit 投影」。UI 是纯消费者（读 store、发命令）。本规格把这层「投影 + 手势」精确转写为 React。

---

## 目录

0. [总体复刻原则与验收方式](#0-总体复刻原则与验收方式)
1. [设计令牌表（AppTheme → CSS variables）](#1-设计令牌表apptheme--css-variables)
2. [窗口外壳与五面板布局（NSSplitViewController → React）](#2-窗口外壳与五面板布局)
3. [组件地图（上游视图 → React 组件）](#3-组件地图)
4. [Toolbar（工具条）](#4-toolbar工具条)
5. [Timeline（时间线）—— 核心](#5-timeline时间线核心)
6. [Inspector（检查器）](#6-inspector检查器)
7. [MediaPanel（媒体面板）](#7-mediapanel媒体面板)
8. [Preview（预览）](#8-preview预览)
9. [交互细节逐项清单（1:1 关键）](#9-交互细节逐项清单11-关键)
10. [Zustand 状态结构（只读镜像 + UI-only 态）](#10-zustand-状态结构只读镜像--ui-only-态)
11. [Tauri command / event 对接点](#11-tauri-command--event-对接点)
12. [数据模型镜像（TS 类型）](#12-数据模型镜像ts-类型)
13. [实施清单与 1:1 验收方式](#13-实施清单与-11-验收方式)

---

## 0. 总体复刻原则与验收方式

### 0.1 三条不可动摇的原则

1. **像素与帧的换算公式逐字照搬**。时间线 X 轴：`x = headerWidth + frame * pixelsPerFrame`（`TimelineGeometry.swift:138-140`）。`pixelsPerFrame == editor.zoomScale`，初值 `Defaults.pixelsPerFrame = 4.0`（`Constants.swift:61`）。
2. **所有阈值、命中区、磁吸距离、动画时长照搬常量**。不允许"差不多"。
3. **真相源在 Rust**。前端仅持 `Timeline` 只读镜像 + UI-only 态（selection/zoom/hover/tab 等）。每次编辑命令后由 Rust 广播 `timeline_changed{version}`，前端据此重取（对应上游 `timelineRenderRevision`，`EditorViewModel.swift:76,27-29`）。

### 0.2 上游唯一可观测容器 → 前端 store 映射

上游 `EditorViewModel`（`@Observable @MainActor`，`EditorViewModel.swift:21-23`）同时持有①持久化态（`timeline`/`mediaManifest`/`generationLog`）②面板焦点态③大量瞬态 UI 态。OpenTake 跨进程拆分：①持久化态 = Rust 真相，前端持镜像；②③ = 前端 Zustand UI 态。**§10 给出完整字段拆分。**

### 0.3 验收方式（每个面板都要做）

- **截图对拍**：同一工程在上游 macOS app 与 OpenTake 中并排，逐面板对比布局/间距/字号/颜色。断点至少 1600×1000（`AppTheme.Window.projectDefault`，`AppTheme.swift:234`）。
- **行为对拍**：§9 每条交互逐项手动验证（拖拽落点、磁吸、右键菜单项、快捷键）。
- **几何对拍**：给定 `fps/zoom/clip` 集合，断言 clip rect / playhead x / 刻度位置与上游公式输出一致（可单测 geometry 纯函数）。

---

## 1. 设计令牌表（AppTheme → CSS variables）

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

---

## 2. 窗口外壳与五面板布局

> 来源:`Editor/EditorView.swift`(NSSplitViewController 实现)、`Constants.swift`(LayoutPreset + Layout 尺寸)、`Editor/EditorWindowController.swift`、`Editor/TitleBarView.swift`、`App/MainMenu.swift`。

### 2.1 五个面板 + LayoutPreset

五个叶面板（`EditorViewModel.FocusedPanel`，`EditorViewModel.swift:35-44`）：**agent / media / preview / inspector / timeline**。
（注：`agent` 面板属另一个 Issue，本规格只在布局位置上为其预留列，内部 UI 不展开。）

三种布局预设（`LayoutPreset`，`Constants.swift:3-23`），可在 View 菜单/⌘1/⌘2/⌘3 切换，持久化到 `localStorage["layoutPreset"]`（上游 `UserDefaults`，`EditorViewModel.swift:99-107`）：

| preset | label | 结构 |
|---|---|---|
| `.default` | Default | 见 2.2 |
| `.media` | Media | 见 2.3 |
| `.vertical` | Vertical | 见 2.4 |

最外层永远是「**agent 列（左）｜ preset 子树（右）**」的水平 split（`EditorView.swift:182-202`）。agent 列默认折叠（`agentPanelVisible` 默认 `false`，`EditorViewModel.swift:135-137`），宽度区间 `[agentPanelMin=240, agentPanelMax=640]`（`Constants.swift:35-36`）；preset 子树 `minimumThickness=400`（`EditorView.swift:201`）。

### 2.2 Default 布局（`EditorView.swift:207-230`）

```
┌──────────────────────────── preset 子树 ────────────────────────────┐
│  上半(水平 split):  [Media] | [Preview] | [Inspector]               │  ← 高度 = 70%
│  ───────────────────────────────────────────────────────────────── │  (target.height*0.7)
│  下半:  [Toolbar(38) + Timeline]                                    │  ← 30%
└─────────────────────────────────────────────────────────────────────┘
```

- 垂直 split 分割位:`setPosition(round(targetH*0.7), dividerAt:0)`（`EditorView.swift:226`）。
- 上半水平三栏:Media 宽 `mediaPanelDefault=500`（左,`Constants.swift:27`,`EditorView.swift:227`）;Inspector 宽 `inspectorDefault=260`（右,`Constants.swift:31`,`EditorView.swift:228`）;Preview 居中吃剩余。
- 上半 `minimumThickness = previewMinHeight = 320`（`EditorView.swift:217`,`Constants.swift:57`）。

### 2.3 Media 布局（`EditorView.swift:235-261`）

```
┌─────────┬──────────────────────────────────────┐
│         │  上(水平): [Preview] | [Inspector]      │ ← 高 55%
│ [Media] │  ──────────────────────────────────── │
│  (30%)  │  下: [Toolbar + Timeline]              │ ← 45%
└─────────┴──────────────────────────────────────┘
```

- 最外水平:Media 宽 = `round(targetW*0.3)`（`EditorView.swift:256-257`）。
- 右侧垂直 split:上半 = `round(rightH*0.55)`（`258`）。
- 上半水平:Inspector 宽 `inspectorDefault=260`,Preview 吃剩余（`259`）。

### 2.4 Vertical 布局（`EditorView.swift:266-288`）

```
┌────────────────────────────────┬───────────┐
│ 上(水平): [Media] | [Inspector]  │           │
│ ────────────────────────────── │ [Preview] │
│ 下: [Toolbar + Timeline]        │           │
└────────────────────────────────┴───────────┘
```

- 最外水平:左子树宽 = `round(targetW*0.5)`,右 = Preview（`EditorView.swift:284`）。
- 左侧垂直:上半 = `round(leftH*0.55)`（`285`）。
- 上半水平:Media 宽 `mediaPanelDefault=500`,Inspector 吃剩余（`286`）。

> **实现建议**：用 CSS Grid + 可拖拽分隔条（如 `react-resizable-panels` 或 `allotment`），并在 React 内**复刻初始百分比/像素分割位**。三种 preset = 三套不同 grid 模板。分隔条命中区加宽（上游 `effectiveRect` 把分隔条 hit 区上下/左右各扩 `panelGap/2 = 2.5px`，`EditorView.swift:24-36`）。

### 2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）

每个叶面板被包成：
1. 内容 `background = --bg-surface`（rgb 22）；
2. `clipShape(RoundedRectangle(cornerRadius: radius.sm=6, style:.continuous))` → CSS `border-radius: 6px`；
3. `padding(panelGap/2 = 2.5px)`；
4. 外层 `background = --bg-base`（rgb 10）—— 即面板之间露出的"沟槽"是 base 色，每个面板是 surface 色的圆角卡片浮在上面；
5. 焦点环叠加（见 2.6）。

> 视觉净效果:深色底(base)上漂浮一组 surface 圆角卡片,卡片间 5px 沟槽。**必须照搬**:这是上游辨识度最高的外观特征。

### 2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）

- 当前聚焦面板:`RoundedRectangle(radius.sm=6).strokeBorder(accent.primary, lineWidth: 1.5).opacity(0.6)`,非聚焦 `opacity 0`,过渡 `easeOut 0.2s`。
- `allowsHitTesting(false)`(不拦截鼠标)。
- 聚焦由点击决定:`EditorWindowController.handlePanelClick`（`EditorWindowController.swift:183-194`）—— 点击落在哪个面板,该面板的 `accessibilityIdentifier`(= `"<panel>Panel"`,`EditorViewModel.swift:38`)即设为 `focusedPanel`。**副作用**:点进 media 面板清空 `selectedClipIds`;点进 timeline 面板清空 `selectedMediaAssetIds`（`EditorWindowController.swift:188-189`）。

### 2.7 面板可见性 / 折叠 / 最大化

- **可见性切换**:media（⌘0）/ inspector（⌘⌥0）/ agent（⌘⌥A）（`MainMenu.swift:104-114`）。状态持久化:`mediaPanelVisible`(默认 true)、`inspectorPanelVisible`(默认 true)、`agentPanelVisible`(默认 false)（`EditorViewModel.swift:135-151`）。preview / timeline 不可折叠(`canCollapse=false` 仅 media/inspector/agent;preview/timeline 永远在)。
- **最大化**:聚焦面板 + 反引号键 `` ` ``（无修饰键）→ `maximizedPanel`（`MainMenu.swift:118-120`,`EditorWindowController.swift:123-128`）。最大化时折叠所有兄弟,Esc 退出（`EditorWindowController.swift:152-155`）。前端等价:把目标面板 grid 区域扩满,其余 `display:none`。
- 折叠用动画(`item.animator().isCollapsed`,`EditorView.swift:164-166`)→ CSS 宽/高过渡。

### 2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）

Tauri 自定义标题栏区域：
- **Leading（左）**：Agent 面板切换按钮 —— 图标 `bubble.left(.fill)`，`fontSize=md(13)`，`foregroundStyle = aiGradient`，可见时 `opacity 1` 否则 `0.55`，frame `IconSize.lg(26)`，`hoverHighlight`，help "Toggle Agent Panel"（`TitleBarView.swift:6-19`）。
- **Trailing（右）**：`Spacer` → UpdateBadge(可选,OpenTake 用自己的更新机制,可暂略) → **Export 按钮**（图标 `square.and.arrow.up` 上移 1px + 文字 "Export"，`fontSize=sm(11) medium`，`text-secondary`，水平 padding `sm(6)`，高 `IconSize.lg(26)`，`hoverHighlight`，help "Export (⌘E)"，点击 `showExportDialog=true`）→ UserAvatar(账户,属另一 Issue)（`TitleBarView.swift:22-48`）。
- 窗口默认尺寸 `projectDefault = 1600×1000`，最小 `projectMin = 960×600`，标题栏 trailing 预留宽 280（`AppTheme.swift:234-236`）。

### 2.9 主菜单 / 应用菜单（`App/MainMenu.swift`）

Tauri 原生菜单（macOS）/ 应用内菜单（Win/Linux）需复刻以下项与快捷键（**§9.6 是完整快捷键表**）。菜单结构（`MainMenu.swift:8-163`）：

- **App**：About / Check for Updates… / Settings…(⌘,) / Quit(⌘Q)
- **File**：New(⌘N) / Open…(⌘O) / Save(⌘S) / Save As…(⇧⌘S) / Import Media…(⌘I) / Export…(⌘E)
- **Edit**：Undo(⌘Z) / Redo(⇧⌘Z) / Cut(⌘X) / Copy(⌘C) / Paste(⌘V) / Select All(⌘A) / Split at Playhead(⌘K) / Trim Start to Playhead(Q) / Trim End to Playhead(W) / Delete(⌫)
- **View**：Media Panel(⌘0) / Inspector(⌘⌥0) / Agent Panel(⌘⌥A) / Maximize Focused Panel(`` ` ``) / Layout ▸ {Default ⌘1, Media ⌘2, Vertical ⌘3} / Enter Full Screen(⌘F)
- **Help**：Tutorial / Keyboard Shortcuts(⌘?) / MCP Instructions / Send Feedback…

菜单项勾选态:layout 三项、三个面板可见性、最大化态都带 checkmark/state(`EditorWindowController.validateMenuItem`,`EditorWindowController.swift:270-303`)。

---

## 3. 组件地图

### 3.1 顶层装配（上游视图树 → React 组件树）

```
AppShell (Tauri window chrome)
├─ TitleBar
│   ├─ TitleBarLeading      ← TitleBarView.swift:3  (Agent 切换)
│   └─ TitleBarTrailing     ← TitleBarView.swift:22 (Export, Avatar)
└─ EditorSplit              ← EditorView / EditorSplitViewController
    ├─ AgentPanel           ← AgentPanelView      (另一 Issue,占位)
    └─ PresetRoot           ← buildDefault/Media/VerticalLayout
        ├─ MediaPanel       ← MediaPanelView.swift
        ├─ PreviewContainer ← PreviewContainerView.swift
        ├─ Inspector        ← InspectorView.swift
        └─ TimelineRegion
            ├─ Toolbar      ← ToolbarView.swift  (高 38)
            └─ TimelineContainer ← TimelineContainerView.swift
```

每个叶面板用 `<PanelShell panel="media|preview|inspector|timeline">`（实现 2.5/2.6 的 surface 卡片 + 焦点环 + 点击聚焦）。

### 3.2 完整组件清单（上游 → React，含层级/布局/关键交互）

| 上游 SwiftUI/AppKit | React 组件 | 层级/布局 | 关键交互 |
|---|---|---|---|
| `EditorSplitViewController` | `EditorSplit` | grid，3 preset | 拖拽分隔条、折叠、最大化 |
| `PanelFocusRing` | `PanelShell` 内 ring | 叠加层 | 点击聚焦切环 |
| `ToolbarView` | `Toolbar` | flex 行，高38 | undo/redo、工具切换、split/trim、加字、缩放滑块 |
| `TimelineContainerView` | `TimelineContainer` | 固定 header 列(100) + 滚动区 | 滚动同步、双向滚动条 |
| `TimelineHeaderView`(AppKit draw) | `TrackHeaderColumn`(canvas 或 DOM) | 左固定列宽100 | mute/hide/sync 切换、轨道高度拖拽 |
| `TimelineView`(AppKit draw) | `TimelineCanvas`(Canvas2D) | document 视图,宽=帧*zoom | 全部时间线手势(见§5) |
| `TimelineRuler` | (画在 TimelineCanvas 顶) | ruler 高24 | —— |
| `PlayheadOverlay`(CAShapeLayer) | `Playhead`(绝对定位 SVG/div) | zIndex 100 | 跟随 activeFrame |
| `SnapIndicatorOverlay` | `SnapIndicator` | zIndex 90,虚线 | 拖拽时显隐 |
| `ClipGeneratingOverlay` | `ClipGeneratingOverlay` | 覆盖在 clip rect 上 | 生成中动画 |
| `InspectorView` | `Inspector` | 标题栏 + tab + 滚动内容 | tab 切换、字段编辑 |
| `ScrubbableNumberField` | `ScrubbableNumberField` | 行内 | 拖拽改值/点击输入 |
| `InspectorPositionFields` | `PositionFields` | 行内双字段 | X/Y 拖拽 |
| `KeyframesPanel`/`KeyframesLane` | `KeyframesPanel` | Inspector 右半 | 关键帧 ruler+lane 拖拽/右键 |
| `MediaPanelView` | `MediaPanel` | 左标签轨(38) + tab 内容 | tab 切换、hover 标签 |
| `MediaTab` | `MediaTab` | toolbar + grid | 导入/生成/搜索/筛选/拖拽/选择/右键 |
| `AssetThumbnailView` | `AssetTile` | grid cell | 拖到时间线、双击、右键 |
| `FolderTileView` | `FolderTile` | grid cell | 双击进入、拖入、重命名 |
| `CaptionTab` | `CaptionTab` | 表单 + 预览 | 字幕生成参数 |
| `MusicTab` | `MusicTab` | 列表/生成 | 音乐选取/生成 |
| `PreviewContainerView` | `PreviewContainer` | tab栏 + 画布 + scrub + transport | 见§8 |
| `PreviewView`/`TransformOverlayView`/`CropOverlayView` | `PreviewCanvas`+`TransformOverlay`+`CropOverlay` | 画布层叠 | 拖动变换/裁剪手柄 |
| `CapsuleButtonStyle` | `CapsuleButton` | —— | hover/press |
| `HoverHighlight` | `useHoverHighlight`/`<HoverArea>` | —— | hover 背景渐显 |
| `GeneratingOverlay` | `GeneratingOverlay` | 覆盖 | 生成中 |

### 3.3 SF Symbol → 前端图标映射（逐处复刻；用统一 SVG 图标集）

> 上游用 SF Symbols。前端选一套**视觉接近 SF 的图标集**（推荐 Lucide，缺的用 Phosphor/自绘）。下表给出**用到的 symbol** → 推荐替代 + 用处。命名以 Lucide 为例。

| SF Symbol | 用处 | 替代(Lucide) |
|---|---|---|
| arrow.uturn.backward / .forward | Undo/Redo | rotate-ccw / rotate-cw |
| cursorarrow | Pointer 工具 | mouse-pointer-2 |
| scissors | Razor 工具 | scissors |
| square.split.2x1 | Split | split-square-horizontal / scissors-line-dashed |
| minus/plus.magnifyingglass | 缩放 ± | zoom-out / zoom-in |
| folder | Media tab / 文件夹 | folder |
| captions.bubble | Captions tab | captions |
| music.note | Music tab | music |
| slider.horizontal.3 | Inspector/设置 | sliders-horizontal |
| info.circle | Inspector Timeline 态 | info |
| diamond / diamond.fill | 关键帧戳 | diamond (fill 用实心) |
| chevron.left/right/down/up | 导航/折叠 | chevron-* |
| arrow.counterclockwise | Reset | rotate-ccw |
| arrow.left.and.right / up.and.down | Flip H/V | flip-horizontal / flip-vertical |
| eye / eye.slash | 轨道隐藏 | eye / eye-off |
| speaker.wave.2.fill / speaker.slash.fill | 轨道静音 | volume-2 / volume-x |
| link / personalhotspot.slash | sync-lock 开/关 | link / unlink |
| play.fill / pause.fill | 播放/暂停 | play / pause (实心) |
| backward.end.fill / forward.end.fill | 跳首/尾 | skip-back / skip-forward |
| backward.frame.fill / forward.frame.fill | 逐帧 | step-back / step-forward (或自绘 frame) |
| camera | 截帧 | camera |
| square.and.arrow.up | Export/分享 | share / upload |
| bubble.left(.fill) | Agent | message-square |
| sparkles | Generate / Lottie | sparkles |
| plus | Import | plus |
| line.3.horizontal.decrease | 筛选 | filter |
| arrow.up.arrow.down | 排序 | arrow-up-down |
| rectangle.grid.2x2 / square.grid.2x2 | 视图模式 | layout-grid |
| ellipsis | 溢出菜单 | more-horizontal |
| xmark | 关闭 | x |
| exclamationmark.triangle.fill | 错误 | alert-triangle |
| waveform | 音频 | audio-waveform |
| film / photo / textformat | clip 类型 | film / image / type |
| arrow.left.arrow.right | Swap Media | arrow-left-right |
| arrow.clockwise | Retry | refresh-cw |
| rectangle.split.3x1 / sidebar.left / sidebar.right | layout 预设图标 | panels-* / sidebar |

> 图标颜色一律用 `currentColor` + 上游对应 `foregroundStyle`（text.secondary/tertiary/primary/muted 或 accent）。

---

## 4. Toolbar（工具条）

> 来源:`Toolbar/ToolbarView.swift`(全文)。高度 `toolbarHeight=38`（`Constants.swift:41`），位于 timeline 面板内、时间线上方（`EditorView.swift:54-60`）。

布局:水平 `HStack(spacing: md=10)`,左对齐组 + `Spacer` + 右侧缩放组,水平 padding `md=10`（`ToolbarView.swift:8,63`）。

### 4.1 左侧按钮组（从左到右，组间用竖直 Divider，高 `Spacing.xl=20`，`ToolbarView.swift:15-16`）

1. **Undo / Redo**（`ToolbarView.swift:10-13`）：图标 `arrow.uturn.backward` / `arrow.uturn.forward`。help "Undo (⌘Z)" / "Redo (⇧⌘Z)"。动作 = 走 Tauri `undo`/`redo`（上游发 `undo:`/`redo:` selector）。
2. **工具模式**（`ToolbarView.swift:19-22`）：
   - Pointer：`cursorarrow`，help "Pointer (V)"，active 态 = `toolMode==.pointer`。
   - Razor：`scissors`，help "Razor (C)"。
   - active 按钮:图标 `text-primary` + `hoverHighlight(isActive:true)`;非 active:`text-tertiary`（`ToolbarView.swift:87-98`）。
3. **Split / Trim**（`ToolbarView.swift:28-32`）：
   - Split：`square.split.2x1`，help "Split at Playhead (⌘K)"，动作 `splitAtPlayhead`。
   - Trim Start：字形按钮 `[`（等宽 16px semibold），help "Trim Start to Playhead (Q)"，动作 `trimStartToPlayhead`。
   - Trim End：字形按钮 `]`，help "Trim End to Playhead (W)"，动作 `trimEndToPlayhead`。
4. **加内容**（`ToolbarView.swift:38-40`）：Text 字形按钮 `T`（serif，17px bold，`ToolbarView.swift:103`），help "Add Text"，动作 `addTextClip`。

### 4.2 按钮样式（三种，`ToolbarView.swift:67-122`）

- **图标按钮**：`Image(systemName).font(size: md=13).foregroundStyle(text-secondary).frame(24×24).hoverHighlight()`。
- **字形按钮 bracket**：`Text("[").font(16, semibold, monospaced).foregroundStyle(text-secondary).frame(24×24).hoverHighlight()`。
- **Text 字形**：`Text("T").font(17, bold, serif)...frame(24×24).hoverHighlight()`。
- 所有按钮 `buttonStyle(.plain)` + `.help(...)`（tooltip）。

### 4.3 右侧缩放滑块（`ToolbarView.swift:45-61`）

- 结构：`HStack(spacing: xs=4)`：`minus.magnifyingglass`(text-tertiary, fontSize sm=11) → Slider → `plus.magnifyingglass`。
- Slider：**对数映射**（滑块行程对每个缩放倍率均匀），`get: log(zoomScale)`，`set: zoomScale = exp(value)`（`ToolbarView.swift:50-53`）。range = `log(minZoomScale)...log(Zoom.max=40)`（`Constants.swift:84`）。`controlSize(.mini)`，`tint = accent.primary`，`width=100`（`ToolbarView.swift:54-57`）。
- `minZoomScale`:由 `EditorViewModel` 计算(适配时间线全长到可视宽,见 §5 zoom)。前端需复刻其计算或从 Rust/镜像取。

> 前端实现:自定义 `<input type=range>` 或自绘滑块,**用对数刻度**;tint 用 `--accent-primary`;两侧放大缩小图标。

---

## 5. Timeline（时间线）—— 核心

> 来源:`Timeline/` 全目录。这是上游用 **AppKit 直接绘制(CGContext)** + `TimelineInputController` 手势的部分。OpenTake 前端**强烈建议用 `<canvas>` 2D 绘制 clip/轨道/刻度**(性能 + 像素级一致),playhead/snap 指示用绝对定位 DOM/SVG 叠加层,生成中 overlay 用 DOM 叠加。
> **核心原因**:clip 渲染含缩略图条/波形/音量橡皮筋/淡变楔形/关键帧菱形,DOM 逐元素会卡;Canvas 2D 能逐字照搬 `ClipRenderer.draw` 的绘制顺序与坐标。

### 5.1 容器结构（`TimelineContainerView.swift:6-57`）

```
TimelineContainer (relative)
├─ TrackHeaderColumn   ← 左固定列, 宽 trackHeaderWidth=100, 高随内容
├─ 竖直分隔线          ← x = 100-1, 宽1, 色 border-primary  (TimelineContainerView.swift:30-35)
└─ ScrollArea          ← x=100 起, 双向滚动(横+纵), 自动隐藏滚动条, mini 滚动条
     └─ TimelineCanvas ← document 视图, 宽=内容宽, 高=内容高
```

- 滚动条:横纵都有,`autohidesScrollers=true`,`controlSize=.mini`,`drawsBackground=false`（`TimelineContainerView.swift:17-20`）。
- **header 列与画布纵向滚动联动**:画布纵向滚动时,header 列 `setBoundsOrigin(y)` 跟随（`TimelineContainerView.swift:115-122`），即 header 不随横向滚,但随纵向滚。前端:header 列纵向 `translateY(-scrollTop)`,横向固定。
- **内容尺寸**（`TimelineView.swift:116-129`）：宽 = `zoomScale * totalFrames + visibleWidth*0.5`（右侧留半屏空白）;高 = `max(visibleHeight, 最后一轨底 + dropZoneHeight)`。

### 5.2 几何（必须逐字照搬，`TimelineGeometry.swift`）

所有公式（`headerWidth` 在画布坐标里通常传 0，因 header 是独立列）：

- `pixelsPerFrame = zoomScale`。
- **轨道 Y 起点**:第一轨顶 = `rulerHeight(24) + dropZoneHeight(60)`,之后累加各轨 `displayHeight`（`TimelineGeometry.swift:39-46`）。即**刻度下方有 60px 的顶部 drop zone**,再开始第一条轨道。
- `trackY(at:i)` = 累积 Y（`53-55`）；`trackHeight(at:i)` = `displayHeight`（默认 50，`Track.displayHeight` `Timeline.swift:34`；范围 `[TrackSize.minHeight=32, maxHeight=200]` `Constants.swift:76-77`）。
- **clip rect**（`TimelineGeometry.swift:62-69`）：
  ```
  x = headerWidth + clip.startFrame * pixelsPerFrame
  y = trackY(i) + 2
  width = clip.durationFrames * pixelsPerFrame
  height = trackHeight(i) - 4          // 上下各留 2px
  ```
- `frameAt(x)` = `max(0, Int((x - headerWidth)/pixelsPerFrame))`（`71-73`，**截断取整**）。
- `trackAt(y)`：找第一个 `y < cumulativeY[i]+height[i]` 的 i（`75-80`）。
- `xForFrame(frame)` = `headerWidth + frame*pixelsPerFrame`（`138-140`）。
- 常量:`Layout.rulerHeight=24, dropZoneHeight=60, trackHeaderWidth=100, insertThreshold=10, dragThreshold=3`（`Constants.swift:46-53`）;`Trim.handleWidth=4, clipCornerRadius=3`（`Constants.swift:99-102`）。

### 5.3 刻度 Ruler（`TimelineRuler.swift`）

- 高 `rulerHeight=24`，背景 `--bg-surface`，底部 1px 分隔线 `border-primary`（`TimelineRuler.swift:13-21`）。
- **主刻度间隔**:目标 ~80px,从 `[1,2,5,10,15,30,60,120,300,600,1200,1800,3600]*fps` 选第一个 `>= 80/pixelsPerFrame` 的（`TimelineRuler.swift:87-94`）。
- **次刻度细分**:在 `[10,5,4,2]` 中选第一个使每格 `>=12px` 的（`97-106`）。
- 次刻度:`text-muted α0.4`,线宽 0.5;中点(偶数细分的一半处)高 6px,其余 4px（`44-60`）。
- 主刻度:`text-muted`,线宽 1,高 8px;标签 = `formatTimecode(frame, fps)`,等宽数字,`fontSize=xs(10)`,色 `text-tertiary`,绘制在 `(x+3, top+2)`（`63-83`）。
- 刻度随横向滚动:绘制矩形 `x=scrollOffsetX`,宽=可视宽（`TimelineView.swift:256-262`）—— 即**刻度固定在可视区顶部,内容横滚时刻度数字滚动**。前端:刻度画在可视区顶层(sticky),依 scrollLeft 重绘。

### 5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻

绘制顺序（在 clip rect 内，`cornerRadius = 3`）：

1. **底色填充**（`ClipRenderer.swift:74-81`）：`fill = sourceClipType.themeColor`，selected α0.45，否则 α0.30。圆角矩形。
2. **可视内容**（`95-106`，contentX = `minX+stripWidth(3)+1`，contentWidth = `width-3-1-handleW(4)`，contentY = `minY+labelBarHeight(16)`）：
   - video：缩略图条（`drawThumbnailStrip`，`494-536`，按 trim 映射可见时段，水平平铺缩略图，按 clip 圆角裁剪）。
   - image：平铺图片（`drawTiledImage`，`540-560`）。
   - audio：波形（`drawWaveform`，`195-263`，dB 归一峰值检测，音量移 dB 轴）。
3. **音量橡皮筋 / 不透明度淡变**（`108-112`）：audio → `drawVolumeRubberBand`(白线音量曲线 + 淡入淡出楔形 + 选中时菱形关键帧/方块拐点)（`267-380`）；非 audio → `drawOpacityFades`(淡变楔形,选中时拐点方块)（`382-436`）。
4. **左色条**（`114-119`）：`x=minX, 宽 stripWidth=3, 高=全高`，圆角，色 `sourceClipType.themeColor`（实心，比底色更饱和）。
5. **边框**（`121-132`）：selected → `white α0.9, 线宽 1.5`；否则 `border-primary, 线宽 0.5`。
6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。
7. **标签栏**（`drawLabelBar`，`594-621`）：clip 宽 `>20` 才画。文本 = `"<首个非空行的名字>  <时长时间码>"`，`fontSize=xs(10) medium`，`text-primary`，左 inset 6，垂直居中于 16px 标签栏；若 `linkGroupId != nil` 给名字部分加**下划线**（`607-609`）。文本按标签栏裁剪。
8. **out-of-sync 偏移徽标**（`147-149`，`drawOffsetBadge` `627-655`）：linkOffset≠0 时右上角红色圆角徽标 `"+N"/"-N"`，色 `rgb(255,71,71)`，`fontSize=xs(10) semibold`。
9. **关键帧菱形标记**（`drawKeyframeMarkers`，`163-191`）：clip 底部（`y = maxY-5`）一排黄色小菱形（半径3），`systemYellow α0.95` 填 + `black α0.5` 描 0.5；位置 = 各 opacity/position/scale/crop 关键帧的绝对帧（**volume 关键帧不在这,画在橡皮筋上**）。
10. **trim 手柄**（`drawTrimHandles`，`659-666`）：左右各一条 `width=handleWidth(4)` 的 `text-muted` 竖条，贴 clip 左右边。

**音量橡皮筋细节**（audio，`drawVolumeRubberBand` `267-380`）：
- body = 标签栏下方区域（`clipBodyRect`：`y=minY+16, height=clip高-16-1`，`ClipRenderer.swift:18-25`）。
- dB↔Y：`volumeRubberBandTopDb=6, bottomDb=-60`，高 dB→小 Y（`y(forDb:)` `28-34`）。
- 音量线:有关键帧 → 折线(linear/hold/smooth 分段,smooth 用 12 步)；无 → 平直线在静态音量 dB（`281-318`）。线色 selected `white α0.95` 否则 α0.75，线宽 1.5。
- 淡变楔形:`drawFadeWedge`（拐点在 body 顶部 `fadeKneeTopInset=4` 处的"fade lane"，silenceY=body 底）。
- 选中时画：音量关键帧菱形（`volumeKeyframeSize=7`）+ 左右淡变拐点方块（`7×7`）（`351-379`）。

> **前端实现要点**:`ClipRenderer` 整体移植为一个 `drawClip(ctx, clip, rect, {isSelected, opacity, isMissing, isGenerating, displayName, fps, cache, linkOffset})` 函数,**绘制顺序与每个子绘制函数逐一对应**。缩略图/波形来自 Rust 媒体缓存(见 §11,经 event/asset 协议拿到 sprite/采样)。

### 5.5 轨道头列（`TimelineHeaderView.swift`，AppKit draw）

左固定列宽 100，背景 `--bg-surface`（`TimelineHeaderView.swift:7`）。每条轨道（`55-98`）：
- 顶部空 `rulerHeight=24`（与刻度对齐，header 列顶部 24px 是空 + 1px 分隔线在 `rulerHeight-0.5` 处，`37-39`）；header 内容从 `rulerHeight` 下开始裁剪绘制（`42-43`）。
- 左侧 3px 色条 = `track.type.themeColor`（`59-61`）。
- 轨道标签:`timelineTrackDisplayLabel(i)`(如 "V1"/"A1"),`fontSize=sm(11) medium`,色 `text-secondary`,绘制在 `(stripWidth+6, 垂直居中)`（`63-67`，labelAttrs `8-11`）。
- 右侧图标(14px,SF config 11pt,`48-50`):
  - sync-lock 切换:`active=syncLocked` → `link` 否则 `personalhotspot.slash`（`74-77`）。位置 = 最右减一格。
  - audio 轨:静音切换 `active=!muted` → `speaker.wave.2.fill` 否则 `speaker.slash.fill`（`78-82`）。
  - 非 audio:隐藏切换 `active=!hidden` → `eye` 否则 `eye.slash`（`83-88`）。
  - 图标 tint:active = `text-secondary`;非 active = `text-secondary α0.3`（`116`）。命中区 = 图标外扩 4px（`118`）。
- 每轨上下 1px 白色边框(第一轨顶 + 每轨底,`border-primary`,`91-97`)。
- **视频区/音频区之间粗分隔**:若同时有视频轨和音频轨,在首个音频轨顶画 2px `border-divider` 线（`101-106`）。

**轨道头交互**（`TimelineHeaderView.swift:150-210`）：
- 点击 mute/hide/sync 图标 → 切对应状态（命令：`toggleTrackMute/Hidden/SyncLock`）。
- **轨道高度拖拽**:命中轨道底边 ±`TrackSize.resizeHandleZone=6px`（`Constants.swift:78`）→ 拖拽改 `displayHeight`,clamp `[32,200]`（`180-190`）;拖拽中只改本地 displayHeight,松手才 commit `setTrackHeight`（`192-201`）。游标:命中 resize 区显示 `resizeUpDown`,否则 `arrow`（`203-210`）。
- **displayHeight 不持久化**(开工程重置默认 50,`Timeline.swift:33-34`)→ 前端作为 UI-only 态,不进 Rust 镜像。

### 5.6 Playhead（`PlayheadOverlay.swift`）

- 颜色 `systemRed`，线宽 1（`PlayheadOverlay.swift:38-40`）。
- 形状（`Playhead.appendPath` `7-23`）：竖线从 `top=rulerHeight(24)` 到 `bottom=可视高`；顶端一个**朝下三角**（`triangleSize=8`，宽=8、高=8，尖朝下）。
- X = `playheadState.timelineFrame * pixelsPerFrame - viewport.minX`（`51`）—— **相对可视区**，即 playhead 是固定在可视层的叠加，按 scrollLeft 偏移。
- zIndex 100，跟随 `playheadState.timelineFrame` 与 `zoomScale` 变化（`observe()` `68-78`）。
- **播放时自动滚动**:播放中若 playhead 接近可视边缘(margin 60px)→ 滚动使其到 `可视宽*0.25` 处（`TimelineContainerView.swift:75-90`）。

> 前端:用绝对定位的 SVG(竖线+三角)或两个 div,`left = frame*zoom - scrollLeft`,zIndex 高于 canvas,`pointer-events:none`。

### 5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）

- 黄色虚线（`systemYellow`，线宽 1，`lineDashPattern [4,4]`，zIndex 90，`SnapIndicatorOverlay.swift:14-19`），从 `rulerHeight` 到内容底。
- 两个 X 源:本地拖拽 `localX` 与外部拖入 `externalX`,localX 优先（`40`）。
- 显隐:磁吸命中时设 X 显示,否则隐藏。

**磁吸算法 SnapEngine**（`SnapEngine.swift`，**逐字照搬**）：
- 收集目标 = 所有 clip 的 startFrame/endFrame（排除被拖的 clip）+ 可选 playhead（`collectTargets` `31-48`）。
- 阈值:`baseThreshold = Snap.thresholdPixels=8`(`Constants.swift:70`);换算成帧 = `8/pixelsPerFrame`;clipEdge 用基础阈值,playhead 用 `*Snap.playheadMultiplier=1.5`（`81-84`，`Constants.swift:72`）。
- **黏滞**:已吸附则保持到移出 `*Snap.stickyMultiplier=1.5` 阈值（`64-74`，`Constants.swift:71`）。
- 命中时触发 `alignment` 触觉反馈（`93`）→ 前端无触觉,**忽略或可选轻微视觉脉冲**。
- 多探针:move 时用所有选中 clip 的 start/end 偏移做探针,取最近（`findSnap` probeOffsets，`TimelineInputController.swift:244-264`）。

### 5.8 时间线全部手势（`TimelineInputController.swift` + `TimelineView` 输入转发）

> 前端在 TimelineCanvas 上挂 `pointerdown/move/up`,**逐分支照搬 mouseDown 的判定树**。

**mouseDown 判定树**（`TimelineInputController.swift:32-194`）：
1. **双击 clip**（在刻度下方）→ 选中该 clip 的源媒体资产 + 在 Media 面板 reveal（`36-49`）。
2. 切换 preview tab 回 timeline（若当前非 timeline）（`51-53`）。
3. **点在刻度区**（`scrollOffsetY ≤ y < +rulerHeight`，`55-65`）：
   - 命中 range 边缘 → 拖 range 边（`beginTimelineRangeEdgeDrag`）。
   - 按住 Shift → 开始拉时间线 range 选区（`beginTimelineRangeSelection`）。
   - 否则 → 拖动 playhead scrub（`beginPlayheadScrub`）。
4. **Razor 工具**且命中 clip → 在点击帧 split（用 razorPreviewFrame 或当前帧）（`70-78`）。
5. **命中 clip**（pointer 工具，`80-182`）：
   - **选择逻辑**（linkedOn = !Option，`84-104`）：
     - Shift：已选则减(linked 则减整链)，否则加(linked 加整链)。
     - Option 且未选：仅选此 clip。
     - 无修饰且未选：选 linked 整组（或仅此 clip，若 Option）。
   - **子区域命中（按优先级）**：
     - 淡变拐点命中 → `fadeKnee` 拖拽（`109-118`）。
     - audio 音量关键帧命中 → `audioVolumeKf` 拖拽（`119-131`）。
     - ⌘ + audio + body 命中 → 添加音量关键帧（`132-134`）。
     - 非 Option 且 localX ≤ handleWidth(4) → `trimLeft`（`135-145`）。
     - 非 Option 且 localX ≥ width-handleWidth → `trimRight`（`146-156`）。
     - 否则 → `moveClip`（收集所有选中 clip 作 companions，记 grabOffset）（`157-182`）。
6. **点空白**（`183-190`）：非 Shift 清空选择；命中 gap 则选 gap；开始 marquee 框选。

**mouseDragged 分支**（`198-391`）：
- scrubPlayhead → seek（含边缘自动横滚 `autoScrollHorizontallyForTimelineDrag`，`continuePlayheadScrub`）。
- timelineRange → 拉 range（带磁吸到 clip 边/playhead）。
- moveClip → 算 deltaFrames（多探针磁吸）+ 算落轨（`dropTargetAt(y)`：existingTrack 或 newTrackAt 插入新轨）；跨轨受类型兼容钳制（`clampedTrackDelta`）。
- trimLeft/Right → 算 deltaFrames（磁吸 + clamp：不能 <1 帧；非 image/text 不能超出源素材）。
- audioVolumeKf → 移动音量关键帧（钳制在相邻关键帧之间 + dB 范围）。
- fadeKnee → 算淡变长度（clamp `[0, duration-对边淡变]`）。
- marquee → 矩形相交选择（非 Option 则扩到 linked 整组）；只重绘变化区域。

**mouseUp**（`395-490`）：依据 dragState commit 对应命令（move→`moveClips`/`duplicateClipsToPositions`，新轨 move→建轨+移动；trim→`commitTrim`；volumeKf→`commitMoveVolumeKeyframe`；fade→`commitFade`；range→保留有效 range；marquee 结束）。**无修改的拖拽不发命令**（如 move 落回原位 delta=0 直接 break，`399-403`）。

**mouseMoved（游标）**（`494-557`）：
- 刻度区：range 边缘→`resizeLeftRight`；Shift→`crosshair`；否则→`pointingHand`。
- Razor 工具在轨道区：算 razorPreviewFrame（带磁吸）+ `crosshair` + 画橙色虚线预览（见 5.9）。
- 命中 trim 区→`resizeLeftRight`；淡变拐点→`resizeLeftRight`；audio 音量关键帧→`openHand`；否则→`arrow`。

**scrollWheel / 缩放 / 平移**（`667-711`）：
- **Option + 滚轮 → 缩放**（以光标为锚点），factor = `exp(deltaY * Zoom.scrollSensitivity=0.04)`（`Constants.swift:85`）。
- **⌘ + 滚轮 → 横向平移**（delta = `raw * Zoom.panSpeed=5`，`Constants.swift:87`）。
- 普通滚轮 → 转发给外层滚动视图（双向滚动）。
- **触控板捏合 → 缩放**（factor = `1 + magnification * Zoom.magnifySensitivity=1.5`，`Constants.swift:86`）。
- **缩放锚定**：缩放时保持光标下的帧位置不变（`applyZoom` `693-711`）；`newScale = clamp(minZoomScale, Zoom.max=40, zoomScale*factor)`。
- **playhead 锚定缩放**:非光标缩放(如滑块)时,保持 playhead 视口位置（`applyPlayheadAnchoredScroll` `180-196`）。

> 前端缩放/平移:监听 `wheel`(带 `ctrlKey`/`metaKey`/`altKey`)与 `gesturechange`(Safari 捏合)或 pointer 双指;**逐字复刻锚定与灵敏度常量**。

### 5.9 时间线叠加绘制（`TimelineView.drawContent`，`TimelineView.swift:201-265`）

绘制顺序（在画布上）：
1. 轨道背景（`drawTrackBackgrounds` `591-613`，每轨 `--bg-surface` + 上下 1px 边 + 视频/音频区粗分隔）。
2. range 选区轨道填充（`385-397`，`text-primary α0.06`）。
3. clips（`drawClips`，含 move/trim 的 ghost 预览，`271-381`）。
4. gap 选区（`436-451`，白虚线框 `white α0.12` 填 + `white α0.9` 描 3,3 虚线）。
5. 生成中 overlay 同步（DOM 叠加，`syncGeneratingClipOverlays` `456-473`）。
6. 外部拖入 ghost clips（`drawExternalDragGhosts` `486-538`，opacity 0.5）。
7. marquee 框（`219-228`，`white α0.6` 描 + `white α0.1` 填，3,3 虚线）。
8. 新轨插入线（`230-243`，`systemYellow` 线宽 2 横线）。
9. Razor 预览线（`245-254`，`systemOrange α0.8` 线宽 1，4,4 虚线，竖线）。
10. ruler（`256-262`）。
11. range 选区刻度填充 + 边（`263-264`：刻度区 `text-primary α0.10` 填；边 `accent.timecode α0.80` 线宽 1.5 竖线）。

**move 时 ghost 渲染**（`drawClips` `307-344`）：原位 clip 画淡（opacity 0.3，复制态 1.0），ghost 画 0.7 + 选中边。

**Ripple-insert 指示**（⌘ 拖入时，`drawRippleInsertIndicator` `567-587`）：白色竖线 + 顶部朝右箭头。

### 5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）

> 前端用自定义右键菜单（contextmenu 事件 + 浮层）。**菜单项与分组逐字照搬。**

命中位置决定菜单：
- **淡变拐点上**（`652-669`）：Linear / Smooth（当前项打勾），设淡变插值。
- **audio 音量关键帧上**（`672-692`）：Linear / Smooth / Hold（打勾）+ 分隔 + Delete Keyframe。
- **命中 clip**（`694-764`，未选则先选 linked 整组）：分组（组间分隔线）：
  - **Timeline 组**：Copy；Paste(若可粘贴，带 trackIndex+frame)；Link(若可链接)；Unlink(若可解链)。
  - **AI 组**：Add to Chat；AI Edit(子菜单，若有)。
  - **Media 组**：Swap Media(非 text 且单链组)；Save as Media(video/audio)。
  - 若点击落在 range 内：追加 range 项。
- **空白区**（`766-781`）：Paste(若可)；range 项(若点在 range 内)。
- **range 项**（`783-799`）：Add Range to Chat / Save Range as Media / Clear Range。

### 5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）

- 接受类型 `.string`(内部 asset payload) 和 `.fileURL`(Finder)（`TimelineView.swift:26`）。
- draggingEntered/Updated（`906-929`）：解析 asset payload → 算落点(`dropTargetAt(y)`) + 磁吸帧(`applyExternalSnap`，探针 0 与总时长) + 是否 ripple-insert(⌘)；画 ghost。
- performDrop（`966-1020`）：⌘=ripple-insert，否则普通 add；视觉资产与纯音频分别落轨；走 `addClipsWithSettingsCheck`（导入设置不匹配时弹对话框）。

> **macOS 拖放陷阱**（`AGENTS.md`）：上游警告 SwiftUI `.onDrop` 父视图会遮蔽子 drop。前端无此问题，但**跨面板拖拽**（Media→Timeline）建议用 HTML5 DnD 或 pointer 自管拖拽，payload 用资产 id 列表 + 可选源时段。

---

## 6. Inspector（检查器）

> 来源:`Inspector/InspectorView.swift`(44KB) + `Components/` + `Keyframes/KeyframesLane.swift` + `TextTab.swift` + `AIEditTab.swift`。

### 6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）

`VStack(spacing:0)`：标题栏 + 内容（依选择态四选一）。

**标题栏**（`37-47`）：`HStack(spacing: xs=4)`：图标 + 标题 + Spacer，水平 padding `lg=14`，应用 `panelHeaderBar()`（高 28，背景 `--bg-raised`，底部 1px `border-primary`，`AppTheme.swift:295-302`）。
- 标题/图标随态变（`23-31`）：选中 clip → "Inspector" + `slider.horizontal.3`；选中媒体资产 → "Source" + `info.circle`；否则 → "Timeline" + `info.circle`；框选中 → "Inspector" + `slider.horizontal.3`。

**内容四态**（`49-57`）：
1. 框选中 → marquee 摘要（"N selected" 居中，`text-tertiary`，`71-80`）。
2. 选中 clip(visual/audio) → clip 检查器（`clipInspectorContent`）。
3. 选中媒体资产 → 资产检查器（`mediaAssetInspectorContent`）。
4. 否则 → 工程元数据（`projectMetadataContent`）。

### 6.2 工程元数据态（`projectMetadataContent`，`95-161`）

`ScrollView` → 分节（spacing `xl=20`），每节标题全大写 `fontSize=xxs(9) semibold` + `tracking wide(1.5)` + `text-muted`（`metadataSection` `125-138`）。行 = label(`text-tertiary` xs) + Spacer + value(`text-secondary` xs，可选中，尾部截断)（`plainMetadataRow` `140-161`）。
- Project 节：Name / Path。
- Format 节：Resolution（`W × H`）/ Frame Rate（`N fps`）/ Aspect Ratio（约分，`gcd`）/ Duration（`formatDuration`）。

### 6.3 Clip 检查器 + Tab 栏（`clipInspectorContent` `212-241`，tab `170-283`）

**可用 tab**（`availableTabs` `170-183`，依选择）：
- 单个 text clip → `Text`。
- 有非 text 视觉 clip → `Video`。
- 有 audio clip → `Audio`。
- 单个 AI-可编辑视觉 clip → `AI Edit`（`aiEditEligible` `187-194`）。

**tab 栏**（`genericTabBar` `255-283`）：`HStack(spacing: md=10)`，每 tab = 文字（active `medium` 否则 `regular`）+ 底部下划线（active 显，`bw-medium=1.5`）。active 色 `text-primary`，非 active `text-tertiary`；**"AI Edit" tab 用 aiGradient**（active α1，否则 α0.6，`260-262`）。水平 padding `lg=14`，顶 padding `xs=4`。
- tab>1 才显示 tab 栏（`216`）。

**内容**：`ScrollView` → `VStack(spacing: lg=14)` → 依 tab：Text/Video/Audio/AI（`219-238`），内容 padding `lg=14`。

### 6.4 Video tab（`videoTabContent` `285-311`）

- Transform 节 + Playback(Speed) 节；若单 clip 且关键帧面板开 → 右侧并排 `KeyframesPanel`（`291-304`）。
- 底部 Keyframes 切换条（`keyframesToggleBar` `313-336`）：右对齐按钮 `diamond(.fill)` + "Keyframes"，开 `text-primary` 否则 `text-tertiary`，仅单 clip 可用。

**Transform 节**（`transformSection` `483-509`）：
- 可折叠标题 "TRANSFORM"（全大写小标题 + chevron + Reset 按钮）（`collapsibleHeader` `674-697`）。
- 行（每行高 `KeyframesMetrics.rowHeight=22`）：
  - Position（`InspectorPositionFields`，双字段 X/Y）。
  - Scale（`scaleScrubField`：value=宽度，×100 显示 `%`，`612-630`）。
  - Rotation（`rotationScrubField`：`°`，range -3600…3600，`632-650`）。
  - Opacity（`opacityScrubField`：×100 `%`，range 0…1，`652-670`）。
  - Crop 行 + Flip 行。
- 每个可动画行尾带**关键帧控件**（`keyframeControls` `530-563`）：← prev keyframe｜diamond 戳（on/off，在关键帧上 `accent.timecode` 否则 `text-tertiary`，戳/删，`stampButtonWidth=22`）｜→ next keyframe（navButton 宽 6）。playhead 不在 clip 内时禁用（`inRange`）。

**Flip 行**（`flipRow` `736-765`）：H/V 两个图标切换按钮（`arrow.left.and.right` / `arrow.up.and.down`）。

**Speed 节**（`speedSection` `441-463`）：标题 "PLAYBACK" + Speed 字段（range 0.25…4.0，`x`，`447-459`）。

### 6.5 Audio tab（`audioTabContent` `338-383`）

- Levels 节：Volume 行（dB，`-∞ dB` 特殊显示，range `floorDb…ceilingDb`，`386-408`）+ Fade In / Fade Out 行（秒，`411-438`）。
- 若无视觉 clip：Playback(Speed) 节。
- 单 clip + 关键帧面板开 → 右侧 `KeyframesPanel`。
- 底部 Keyframes 切换条。

### 6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件

- 显示态:暖色文字(`accent.primary`),等宽数字,右对齐;mixed(多值不同)显 `—`(`text-tertiary`)（`57-63`,`displayText` `32-36`）。
- **拖拽改值**:水平拖动,灵敏度 `dragSensitivity`(每像素改的显示单位);**Shift ×10,⌘ ×0.1**（`101-110`）;clamp 到 range;拖动中走 `onChanged`,松手走 `onCommit`（`96-117`）。
- **点击进入输入**:`onClick` 切到 TextField,回车/失焦提交,Esc 取消（`118-121`,`commitEdit` `126-140`：去后缀、逗号转点、解析、clamp）。
- 拖拽阈值 3px 才算拖（`ScrubMouseArea.mouseDragged` `194-204`）；游标 `resizeLeftRight`（`185-187`）。
- 各字段参数(width/format/suffix/sensitivity)见 §6.4/6.5 各处。Volume 用 `fieldWidth=56`,`displayTextOverride` 把 `≤floorDb` 显示成 `-∞ dB`（`InspectorView.swift:397-398`）。

> 前端:`<ScrubbableNumberField>` 用 pointer 拖拽改值 + 点击转 `<input>`;复刻 Shift/⌘ 灵敏度倍率与 3px 阈值;游标 `ew-resize`。

### 6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）

- 指标（`KeyframesMetrics` `4-26`）：ruler 18 + strip 14（header 32）；row 22；stamp 22；nav 6；diamond 8。
- `ClipRulerBlock`（`29-66`）：顶部 ruler + 下方彩色 clip 条（tint α0.35）带 clip 名（xxs medium），点击/拖动 seek。
- `KeyframesLaneRow`（`69-120+`）：每属性一行，Canvas 画黄色菱形（实心 tint + black α0.4 描 0.5），可拖关键帧（带磁吸，阈值 4px，`snapThresholdPixels` `86`），每关键帧不可见命中区(±7px) 带右键菜单（插值/删除）。

### 6.8 小组件

- `InspectorSection`（小标题 + 内容容器）、`InspectorRow`（icon + label + 控件）、`ColorField`（颜色选择）、`FontPickerField`（字体选择，TextTab 用）、`TextContentField`（文本内容）、`GenerationReferencesStrip`（生成参考图条）。逐个对照源码复刻（这些是 Text/AI tab 的子件，结构同 §6 模式：label + 字段 + AppTheme 间距/字号）。

---

## 7. MediaPanel（媒体面板）

> 来源:`MediaPanel/MediaPanelView.swift` + `MediaTab/` + `CaptionsTab/` + `MusicTab.swift`。

### 7.1 外层 + 标签轨（`MediaPanelView.swift`）

`HStack(spacing:0)`：左**标签轨**(垂直，宽 `tabRailWidth=38`) + 右**内容区**（`MediaPanelView.swift:20-35`）。右边缘 1px `border-primary`（`36-38`）。

**标签轨**（`panelTabRail` `55-78`）：`VStack(spacing: xs=4)` + 三个标签：Media(`folder`)/Captions(`captions.bubble`)/Music(`music.note`)（`PanelTab` `9-18`）。背景 `--bg-raised`，右边 1px `border-primary`，水平/上/下 padding `sm=6`。
- 标签按钮（`panelTabButton` `80-109`）：图标 `fontSize=md(13)`（selected semibold 否则 medium），frame `IconSize.lg(26)`，selected `text-primary` 否则 `text-tertiary`，`hoverHighlight(radius.sm, isActive:selected)`；**selected 左侧竖 capsule**（`border-primary`，宽 `bw-thick=2`，高 `IconSize.sm=18`）（`92-98`）。
- **hover 标签提示**（`hoverLabel` `111-129`）：hover 时右侧浮出 capsule 标签（`bg-prominent` + `border-primary` 描 + `shadow.sm`），`fontSize=xs(10) medium`。
- tab 切换动画 `easeInOut 0.2s`（`84`）。

### 7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）

**结构**（`84-156`）：`VStack`：toolbar + (可选 swap banner) + grid 区(ZStack：drop area + toast) + (可选 GenerationView 底部展开)。

**Toolbar**（`254-264`）：`VStack(spacing: xs=4)`：actionsRow + searchControlsRow + contextBar。水平 padding `sm=6`，上 `sm`，下 `xs`，背景 `--bg-surface`。
- **actionsRow**（`266-284`，高 `panelHeaderHeight=28`）：Import 按钮(`plus`) + Generate 按钮(`sparkles`，filled，aiGradient) + 溢出菜单 + Spacer + 搜索索引状态。
- **searchControlsRow**（`286-294`，高 28）：搜索框 + 显示控制（视图模式/缩略图大小/排序/筛选 menu 图标）。
- **displayControls**（`354-404`）：
  - 视图模式 menu（`rectangle.grid.2x2`）：Folders/Flat/Grouped（`ViewMode` `37-55`）+ 缩略图大小 Small80/Medium110/Large150/XL200（`ThumbnailPreset` `61-80`）。
  - 排序 menu（`arrow.up.arrow.down`）：SortMode 各项。
  - 筛选 menu（`line.3.horizontal.decrease`，有筛选时 `accent.primary`）：按类型(video/audio/image) + AI Generated + Clear Filters。
- **contextBar**（`312-322`，高 `contextRowHeight=22`）：面包屑/视图名 + Spacer + 项数。面包屑（`breadcrumbBar` `337-352`）：Library ▸ folder ▸ …，chip 可点导航 + 可拖入。

**网格**（三种视图，`MediaTab+Grids.swift`）：folder/flat/grouped。cell = `AssetTile`（缩略图）或 `FolderTile`。
- 缩略图大小由 `thumbnailSize`(80–200) 控制。
- **搜索结果**（`MediaTab+Search.swift`）：非空查询时显示视觉/语音命中分区。

**AssetTile**（`MediaTab/AssetThumbnailView.swift`）：缩略图卡片，含类型角标、AI 标记、时长、选中态、生成中 overlay。**交互**：拖到时间线（payload=asset id + 可选时段，`MediaTab+Drag.swift`）、双击（预览 tab）、右键菜单、点击选择（Shift/⌘ 多选，方向键移动选择见 §9.6）。

**FolderTile**（`FolderTileView.swift`）：双击进入、拖入资产、重命名（inline）、右键。

**Drop area**（`MediaPanelDropArea.swift`，**AppKit 原生**，因 SwiftUI 父 onDrop 遮蔽问题）：Finder 文件拖入整面板。前端用整区 drop handler。

### 7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）

`ScrollView` 表单（`CaptionTab.swift:53-84`）：Source 节（音频源选择）+ Style 节（字体/大小/颜色/描边样式，实时预览 "Captions will look like this"）+ Placement 节（位置，中心吸附 0.02）+ 底部 Generate 条。生成中显示 `GeneratingOverlay("Transcribing…")`。参数：textCase/censorProfanity/locale/translate 语言列表（`42-45`）。字号默认 48（`AppTheme.Caption.defaultFontSize`），范围 12–300。

### 7.4 Music tab（`MusicTab.swift`）

音乐选取/生成（结构同 tab 模式）。逐项对照源码复刻。

---

## 8. Preview（预览）

> 来源:`Preview/PreviewContainerView.swift`(34KB) + `PreviewView.swift` + `TransformOverlayView.swift` + `CropOverlayView.swift` + `PreviewTab.swift`。

### 8.1 结构（`PreviewContainerView.swift:12-60`）

`VStack(spacing:0)`：
1. **tab 栏**（水平 padding `sm=6` + `panelHeaderBar()` 高28）。
2. **画布区**（`GeometryReader`，居中，按宽高比 fit + canvasZoom 缩放）。
3. **scrub 条** + **transport 条**（非 image）或 **image 设置条**（image）。
- 背景 `--bg-surface`。

### 8.2 画布区（`18-51`）

- 宽高比 = `timeline.width/height`（生成中用生成宽高比，`19,313-318`）。
- fit 到容器（`fitSize` `300-306`）× `canvasZoom`（`21-22`）。
- 层叠（ZStack）：`PreviewView`(Rust 合成帧上屏) + (image 预览) + (失败/生成中/离线 overlay) + Transform/Crop overlay（`23-42`）。
- canvasZoom<1 时画边框 `white α0.25`（`44-47`）。
- 位置：居中 + `canvasOffset` 平移（`48-49`）。

> **`PreviewView` = Rust 合成帧显示层**。OpenTake 用 `<canvas>`/WebGL 显示 Rust 经 `preview_frame` event 推来的帧（见 §11 + ARCHITECTURE §2）。DOM 叠字另算（上游 `TextLayerController`，前端可 DOM 叠加或并入 Rust 合成）。

### 8.3 tab 栏（`tabBar` `529-559`）

`HStack(spacing: xs=4)`：← → 导航按钮 + 水平滚动 tab 列表 + 溢出菜单。
- tab（`tabItem` `561-594`）：名字（active semibold 否则 medium，active/hover `text-primary` 否则 `text-secondary`）+ 可关闭 tab 的 ✕；底部下划线（active 显，`bw-medium`，色 = tab.underlineColor：timeline=`accent.primary`，媒体 tab=类型色，`PreviewTab.swift:50-55`）。
- 始终有 "Timeline" tab（不可关，`PreviewTab.swift:30`）；媒体资产可开成 tab。
- 溢出菜单（`609-628`）：Close All Tabs。

### 8.4 transport 条（`transportBar` `64-101`，高 36）

`HStack(spacing: sm=6)`：时间码文本 + Spacer + 传输按钮组 + Spacer + 截帧按钮(video/timeline) + 工程设置组。
- **传输按钮**（`78-90`，`HStack spacing md=10`）：跳首 `backward.end.fill` / 逐帧退 `backward.frame.fill` / 播放暂停 `play.fill`↔`pause.fill` / 逐帧进 `forward.frame.fill` / 跳尾 `forward.end.fill`。timeline tab 用 `togglePlayback`，源 tab 用 `toggleSourcePlayback`。
- 时间码（`PreviewTimecodeText`）：当前/总时长，等宽数字。
- **截帧按钮**（`116-127`）：`camera`，help "Capture Frame to Media"，动作 `captureCurrentFrameToMedia`。
- **工程设置组**（`projectSettingsGroup` `131-154`，`ViewThatFits`：宽则展开 4 个 badge，窄则收成单 `slider.horizontal.3` menu）：
  - Aspect（badge `W:H`，菜单 `AspectPreset` 各项打勾，`156-171`）。
  - Frame Rate（badge `N`，菜单 24/25/30/50/60，`173-188`）。
  - Quality（badge HD/FHD/2K/4K，`QualityPreset`，`190-206`，badge 算法 `245-251`）。
  - Zoom（badge `Fit`/`N%`，`ZoomPreset`，`208-232`）。
  - badge 样式（`badgeLabel` `270-276`）：`fontSize=xxs(9) bold rounded`，`text-secondary`，水平 padding `sm`，高 `IconSize.mdLg=24`。

### 8.5 scrub 条（`scrubBar` `651-...`）

- 进度条（Capsule 背景 `white α0.10`）+ 进度填充 + 拖拽 thumb。
- hover/拖拽时变粗:thumb 6→10,bar 3→4（`655-657`）。游标 `pointingHand`（`677`）。
- 拖拽 = scrub seek（暂停-拖-恢复语义，同时间线 scrub）。

### 8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）

- **TransformOverlay**:选中视觉 clip 时画变换框 + 8 个缩放手柄 + 旋转 + 拖动移动;边到画布中心/边缘吸附(`Transform.snapToCanvasEdges`/`snapCenterToCanvasCenter`，`Timeline.swift:455-497`)。
- **CropOverlay**:裁剪态(`cropEditingActive`)画裁剪框 + 手柄,带宽高比锁(`CropAspectLock`，`Timeline.swift:513-540`：Custom/Original/16:9/9:16/1:1/4:3/3:4/21:9)。
- 二选一显示（`PreviewContainerView.swift:37-41`）：cropEditingActive → Crop，否则 Transform。
- 这两层是**坐标变换 + 手柄拖拽**，前端用绝对定位 SVG/div 手柄 + pointer 拖拽，**复刻吸附阈值与手柄命中区**（对照两文件源码）。

### 8.7 离线/失败/生成中 overlay（`326-525`）

- 离线媒体（`offlinePreview` `430-480`）：`exclamationmark.triangle.fill`(display 36, status.error) + 标题 + 说明 + 路径 + Relink…/Relink Folder… 按钮（capsule prominent/secondary）。
- 失败（`failedPreview` `482-525`）：生成失败 + 错误文本（可选中滚动）+ Retry Download（若有）。
- 生成中（`generatingPreview` `392-405`）：参考图模糊背景 + `black α0.55` + `GeneratingOverlay(label, .preview)`。

---

## 9. 交互细节逐项清单（1:1 关键）

> 本节是验收的硬清单。每条标注上游来源。前端必须逐条等价实现。

### 9.1 选择（clip）

| 操作 | 行为 | 来源 |
|---|---|---|
| 单击 clip | 选中 linked 整组（无修饰）| `TimelineInputController.swift:102-104` |
| ⌘+单击 clip（未选）| 仅选此 clip（不扩 linked）| `100-101` |
| Shift+单击 | 切换该 clip（linked 则整链加/减）| `88-99` |
| 单击空白 | 清空选择（非 Shift）| `184-186` |
| 框选 marquee | 矩形相交选；非 Option 扩 linked 整组 | `349-383` |
| Esc | 清空选择 + 清 range + 工具回 pointer | `EditorWindowController.swift:156-159` |
| 双击 clip | 选中其源媒体资产 + Media 面板 reveal | `TimelineInputController.swift:36-49` |
| 点击 gap | 选中 gap（空隙）| `187`、`hitTestGap 730-746` |

### 9.2 拖放（移动/复制/落轨）

| 操作 | 行为 | 来源 |
|---|---|---|
| 拖 clip 横向 | 移动（带磁吸到 clip 边/playhead，多探针）| `234-279` |
| Option+拖 | 复制（duplicate）| `isDuplicate`，`180-181`、`423` |
| 拖到轨间隙 | 新建轨插入（黄线指示）| `dropTargetAt`、`425-438` |
| 跨轨 | 仅落到类型兼容轨（钳制）| `clampedTrackDelta 918-935` |
| linked 伙伴 | 跟随移动但留在各自轨（pinned）| `pinnedCompanionIds 901-915` |
| 落回原位 delta=0 | 不发命令 | `399-403` |
| 拖拽阈值 | 3px（`dragThreshold`）| `Constants.swift:53` |

### 9.3 Trim（修剪）

| 操作 | 行为 | 来源 |
|---|---|---|
| 拖左手柄（localX≤4）| trimLeft（带磁吸）| `135-145`、`281-307` |
| 拖右手柄（localX≥w-4）| trimRight | `146-156`、`309-341` |
| 最小 1 帧 | 不能 trim 到 <1 帧 | `304`、`334` |
| image/text | 可自由延长（无源素材上限）| `hasNoSourceMedia`、`305`、`335-339` |
| 默认传播 linked | trim 同步链伙伴（Option 关）| `propagateToLinked = !Option`，`144` |
| 手柄宽 | 4px（`Trim.handleWidth`）| `Constants.swift:100` |
| 游标 | trim 区 `resizeLeftRight` | `542-544` |

### 9.4 Razor / Split / 关键帧 / 淡变 / 音量

| 操作 | 行为 | 来源 |
|---|---|---|
| Razor 工具 + 点 clip | 在点击帧 split（带磁吸预览）| `70-78`、`511-532` |
| Razor 预览线 | 橙虚线 4,4 | `TimelineView.swift:245-254` |
| ⌘K | playhead 处 split | `MainMenu.swift:76` |
| ⌘+点 audio body | 添加音量关键帧 | `132-134`、`addVolumeKeyframeOnClick 646-661` |
| 拖音量关键帧 | 移动（钳制相邻 kf + dB 范围）| `343-344`、`582-618` |
| 拖淡变拐点 | 改淡变长（clamp 对边）| `346-347`、`621-643` |
| 右键音量 kf | Linear/Smooth/Hold + Delete | `TimelineView.swift:672-692` |
| 右键淡变拐点 | Linear/Smooth | `652-669` |

### 9.5 Playhead / scrub / 缩放

| 操作 | 行为 | 来源 |
|---|---|---|
| 点刻度 | scrub playhead | `55-65`、`beginPlayheadScrub 763-771` |
| 拖刻度（边缘）| scrub + 自动横滚 | `continuePlayheadScrub 783-801` |
| Shift+拖刻度 | 拉时间线 range 选区 | `59-61`、`beginTimelineRangeSelection 849-856` |
| 拖 range 边 | 调整 range | `57-58`、`834-847` |
| Option+滚轮 | 缩放（光标锚定）| `668-672` |
| ⌘+滚轮 | 横向平移 | `674-682` |
| 捏合 | 缩放 | `magnify 688-691` |
| 缩放灵敏度 | scroll 0.04 / magnify 1.5 / pan 5 | `Constants.swift:85-87` |
| 缩放范围 | minZoomScale … 40 | `Constants.swift:84` |

### 9.6 键盘快捷键全表（`EditorWindowController.handleKeyDown` `38-164` + `MainMenu.swift`）

> 文本输入聚焦时**不拦截**（`EditorWindowController.swift:39-41,176-181`）。前端：编辑 input/textarea 时跳过全局快捷键。

| 键 | keyCode | 动作 | 条件 |
|---|---|---|---|
| Space | 49 | 播放/暂停 | `55-57` |
| ← | 123 | 退一帧（Shift→退 5 帧）| `60-62` |
| → | 124 | 进一帧（Shift→进 5 帧）| `64-66` |
| ⌫ Delete | 51 | 删除选中 clip；选中文件夹/资产则删之；Shift→ripple 删（clip 或 gap）| `68-85` |
| C | 8 | Razor 工具（非 ⌘）| `87-92` |
| V | 9 | Pointer 工具（非 ⌘）| `94-99` |
| I | 34 | 标记 range 起点（无 ⌘⌥⌃）| `101-106` |
| O | 31 | 标记 range 终点 | `108-113` |
| [ | 33 | Trim Start to Playhead | `115-117` |
| ] | 30 | Trim End to Playhead | `119-121` |
| `` ` `` | 50 | 切换面板最大化（无修饰）| `123-128` |
| Return | 36 | media 面板：进文件夹（单选）；裁剪态：退出 | `130-141` |
| Esc | 53 | 取消 swap / 退裁剪 / 退最大化 / 清选择+range+回pointer | `143-159` |
| 方向键（media 聚焦）| 123/124/125/126 | 移动媒体选择（左右上下，非 Shift）| `49-53,166-174` |
| ⌘Z / ⇧⌘Z | | Undo / Redo | `MainMenu.swift:67-68` |
| ⌘C / ⌘X / ⌘V | | Copy/Cut/Paste（timeline 聚焦时作用于 clip；media 聚焦时 paste 导入）| `EditorWindowController.swift:227-248` |
| ⌘K | | Split at Playhead | `MainMenu.swift:76` |
| Q / W | | Trim Start/End（菜单，无修饰）| `MainMenu.swift:80-86` |
| ⌘N/O/S/⇧⌘S/⌘I/⌘E | | New/Open/Save/SaveAs/Import/Export | `MainMenu.swift:41-56` |
| ⌘0 / ⌘⌥0 / ⌘⌥A | | 切 Media/Inspector/Agent 面板 | `MainMenu.swift:104-114` |
| ⌘1/⌘2/⌘3 | | Layout Default/Media/Vertical | `MainMenu.swift:134-144` |
| ⌘F | | 全屏 | `MainMenu.swift:125` |
| ⌘? | | 快捷键帮助 | `MainMenu.swift:157` |
| ⌘, | | 设置 | `MainMenu.swift:29` |

> **跨平台键位**:macOS ⌘ → Win/Linux Ctrl;⌥ → Alt。Tauri 下用 `accelerator` 字符串复刻。keyCode 是 macOS 物理键码,前端用 `event.code`(如 `Space`/`KeyC`/`BracketLeft`/`Backquote`/`ArrowLeft`)更可靠,**逐键对照上表语义**。

### 9.7 Hover / 焦点 / 游标态

| 元素 | hover 效果 | 来源 |
|---|---|---|
| 图标按钮 | 背景渐显（faint 0.08；active 时 soft 0.10→muted 0.15）| `HoverHighlight.swift:31-38` |
| 面板 | 点击聚焦 → 焦点环 opacity 0.6 | `EditorView.swift:384-396` |
| MediaPanel 标签 | hover 浮出名字 capsule | `MediaPanelView.swift:111-129` |
| Preview tab | hover 文字转 primary | `PreviewContainerView.swift:566-567` |
| 时间线刻度 | `pointingHand`；Shift `crosshair` | `498-505` |
| clip trim 区 | `resizeLeftRight` | `542-544` |
| 轨道高度边 | `resizeUpDown` | `TimelineHeaderView.swift:203-209` |
| scrub 条 | `pointingHand` + 变粗 | `PreviewContainerView.swift:655,677` |
| 数字字段 | `resizeLeftRight`（ew-resize）| `ScrubbableNumberField.swift:185-187` |

### 9.8 右键菜单（汇总，见 §5.10 详表）

时间线 clip / 空白 / range / 淡变拐点 / 音量 kf / 关键帧泳道，菜单项逐字照搬（`TimelineView.swift:641-799` + `KeyframesLane.swift:contextMenu`）。Media 面板资产/文件夹右键（`MediaTab` 各处）。

---

## 10. Zustand 状态结构（只读镜像 + UI-only 态）

> 拆分依据:上游 `EditorViewModel` 字段（`EditorViewModel.swift`）按「Rust 真相镜像」vs「纯前端 UI 态」分流。**镜像态只能由 `timeline_changed` event 更新,前端绝不直接改;UI 态前端自由改。**

### 10.1 镜像态（来自 Rust，只读）—— `useProjectStore`

```ts
interface ProjectMirror {
  // 由 timeline_changed{version} 驱动重取(get_timeline)
  timelineVersion: number;          // ← 上游 timelineRenderRevision (EditorViewModel.swift:76)
  timeline: Timeline;               // fps/width/height/tracks (见 §12)
  // 媒体库(运行时富对象, 来自 Rust)
  mediaAssets: MediaAsset[];        // EditorViewModel.swift:110
  folders: MediaFolder[];
  offlineMediaRefs: Set<string>;    // :111
  unprocessableMediaRefs: Set<string>; // :112
  // 工程信息
  projectUrl: string | null;
  projectId: string | null;
  isDocumentEdited: boolean;        // :185
  // 能力/账户(只读)
  canGenerate: boolean;
}
```

### 10.2 UI-only 态 —— `useEditorUiStore`（前端自管）

```ts
interface EditorUiState {
  // —— 播放/播放头 (上游 EditorViewModel.swift:55-98) ——
  currentFrame: number;             // 提交后的播放头帧
  activeFrame: number;              // = playheadState.timelineFrame(scrub 时的实时帧, :58)
  sourcePlayheadFrame: number;      // 源预览播放头(:96)
  isPlaying: boolean;               // :59
  isScrubbing: boolean;             // :77

  // —— 选择 (上游 :60-66) ——
  selectedClipIds: Set<string>;     // :61
  isMarqueeSelecting: boolean;      // :62
  selectedGap: GapSelection | null; // :63
  selectedTimelineRange: TimelineRange | null; // :64
  selectedMediaAssetIds: Set<string>; // :65
  selectedFolderIds: Set<string>;   // :66

  // —— 时间线视图 (上游 :68-77) ——
  zoomScale: number;                // = pixelsPerFrame, 初值 4.0 (:68)
  minZoomScale: number;             // 由可视宽+总帧算; 前端复刻或从镜像取
  timelineVisibleWidth: number;     // :75
  scrollLeft: number; scrollTop: number; // 滚动位置(上游隐含在 NSScrollView)
  toolMode: 'pointer' | 'razor';    // :78 (ToolMode)
  trackDisplayHeights: Record<string, number>; // 轨道高(不持久, 默认 50; 上游 Track.displayHeight)

  // —— 画布(Preview) (上游 :69-74) ——
  canvasZoom: number;               // :69 (≤1 时 offset 归零)
  canvasOffset: { width: number; height: number }; // :74
  cropEditingActive: boolean;       // :90
  cropAspectLock: CropAspectLock;   // :91

  // —— 面板 (上游 :46-47, 135-157) ——
  focusedPanel: Panel | null;       // :46
  maximizedPanel: Panel | null;     // :47
  layoutPreset: 'default'|'media'|'vertical'; // :99 (持久化 localStorage)
  agentPanelVisible: boolean;       // :135 (默认 false, 持久化)
  mediaPanelVisible: boolean;       // :141 (默认 true, 持久化)
  inspectorPanelVisible: boolean;   // :147 (默认 true, 持久化)
  keyframesPanelVisible: boolean;   // :153 (默认 false, 持久化)

  // —— Preview tabs (上游 :92-95) ——
  previewTabs: PreviewTab[];        // 初值 [timeline]
  activePreviewTabId: string;       // :93
  previewTabHistory: string[]; previewTabHistoryIndex: number; // :94-95(前进/后退)

  // —— Media 面板导航 (上游 :161-170) ——
  mediaPanelCurrentFolderId: string | null;
  mediaPanelRevealAssetId: string | null;
  mediaPanelScrollTarget: string | null;
  mediaPanelToast: string | null;

  // —— 对话框/生成 (上游 :79-89) ——
  showExportDialog: boolean;        // :79
  showGenerationPanel: boolean;     // :80 (打开时切到 Media tab)
  pendingReplacements: Set<string>; // :89 (生成中的 clip id)
  pendingSwapClipId: string | null; // :66

  // —— 剪贴板(可由 Rust 持有, 前端只读能否粘贴) ——
  canPasteClips: boolean;
}

type Panel = 'agent'|'media'|'preview'|'inspector'|'timeline';
```

### 10.3 派生选择器（前端纯函数，不进 store）

复刻上游计算属性：`totalFrames`(Timeline.swift:16-22)、clip rect / playhead x（§5.2 geometry）、`zones`(视频/音频区划分，TimelineHeaderView 用)、`validSelectedTimelineRange`、各 Inspector 的 `selectedVisualClip(s)`/`selectedAudioClip(s)`/`availableTabs`(InspectorView.swift:170-200)。**这些是纯函数,从镜像 + UI 态算,不存储**(避免冗余,符合上游"派生不存"原则)。

### 10.4 持久化

- `localStorage`：`layoutPreset` / `agentPanelVisible` / `mediaPanelVisible` / `inspectorPanelVisible` / `keyframesPanelVisible`（对应上游 `UserDefaults`，`EditorViewModel.swift:99-157`）。
- 其余 UI 态会话内存即可。

---

## 11. Tauri command / event 对接点

> 依据 ARCHITECTURE.md §2「真相源在 Rust,前端持镜像」+ §5 EditCommand 枚举。前端**所有编辑都经 invoke 发命令**,**绝不本地改镜像**;镜像由 event 更新。

### 11.1 命令（invoke）—— 编辑 / 读取 / 播放 / 工程

> 上游所有编辑手势最终调 `EditorViewModel` 的方法(如 `moveClips`/`commitTrim`/`splitClip`/`addClips`...),OpenTake 归一到 `EditCommand`(ARCHITECTURE §5)。前端把 §9 各手势的"提交"映射到对应 invoke。

| 前端动作（来自 §9 手势/菜单） | invoke 命令（建议名）| 上游对应方法 |
|---|---|---|
| 移动 clip 提交 | `edit_apply` { MoveClips } | `moveClips` (TimelineInputController:862-868) |
| 复制 clip 到位 | `edit_apply` { DuplicateClips } | `duplicateClipsToPositions` |
| trim 提交 | `edit_apply` { TrimClips } | `commitTrim` (:440-458) |
| split | `edit_apply` { SplitClip } | `splitClip` (:74) |
| 删除 / ripple 删 | `edit_apply` { RemoveClips / RippleDeleteRanges } | `deleteSelectedClips`/`rippleDelete*` |
| 拖入添加 / ripple 插 | `edit_apply` { AddClips / InsertClips } | `addClips`/`rippleInsertClips` (:966-1020) |
| 改属性(scale/rotation/opacity/volume/speed/fade/flip/crop) | `edit_apply` { SetClipProperties } | `commitScale/Rotation/Opacity/Volume/ClipSpeed/Fade` 等 (InspectorView) |
| 关键帧 戳/删/移/插值 | `edit_apply` { SetKeyframes } | `stampKeyframe`/`removeKeyframe`/`commitMoveVolumeKeyframe`/`setInterpolation` |
| 加文字 | `edit_apply` { AddTexts } | `addTextClip` (ToolbarView:39) |
| 加字幕 | `edit_apply` { AddCaptions } | CaptionTab 生成 |
| Link / Unlink | `edit_apply` { Link / Unlink } | `linkClips`/`unlinkClips` (:840-848) |
| 轨道 mute/hide/sync/高度 | `edit_apply` { SetTrackProps } | `toggleTrackMute/Hidden/SyncLock`/`setTrackHeight` (TimelineHeaderView) |
| 删轨 | `edit_apply` { RemoveTracks } | `removeTracks` |
| 文件夹 建/移/重命名/删 | `edit_apply` { CreateFolder/MoveToFolder/... } | EditorViewModel+Folders |
| Undo / Redo | `undo` / `redo`（或 `edit_apply{Undo/Redo}`）| `undo:`/`redo:` (ToolbarView:79-85) |
| Copy / Paste clip | `clip_copy` / `clip_paste`{trackIndex,frame} | `copySelectedClipsToClipboard`/`pasteClips` |
| 工程 新建/打开/保存/另存 | `project_new`/`project_open`/`project_save`/`project_save_as` | NSDocument (MainMenu) |
| 导入媒体 | `import_media`{urls} | `importMedia` |
| 导出 | `export_start`{preset} | `showExportDialog`→导出 |
| seek(播放头) | `seek`{frame, mode} | `seekToFrame` (EditorViewModel:259-267) |
| 播放/暂停 | `play`/`pause`/`toggle_playback` | `togglePlayback` (:227-233) |
| 改工程设置(fps/分辨率/宽高比) | `set_timeline_settings`{fps,width,height} | `applyTimelineSettings` (PreviewContainerView:160) |
| 截帧到媒体 | `capture_frame` | `captureCurrentFrameToMedia` (:117) |
| Swap/Save as Media | `swap_media`/`save_clip_as_media` | `beginMediaSwap`/`saveClipAsMedia` |
| 读时间线 | `get_timeline` → Timeline + version | (镜像同步用) |
| 读媒体库 | `get_media` → assets/folders | (镜像同步用) |
| relink 离线 | `relink_asset`{id,url} / `relink_folder`{url} | `relinkAsset`/`relinkOfflineAssets` (PreviewContainerView:367-390) |

> `edit_apply` 返回 `EditResult`{changed, action_name, affected_clip_ids, timeline_version, summary}（ARCHITECTURE §5）。前端据 `changed/timeline_version` 决定是否重取。

### 11.2 事件（listen）—— Rust → 前端推送

| event | payload | 前端处理 |
|---|---|---|
| `timeline_changed` | `{ version: u64 }` | 若 version > 本地 → `get_timeline` 重取镜像 → 触发重绘（对应上游 `timelineRenderRevision` 自增→刷新，`EditorViewModel.swift:27-29,304-312`）|
| `preview_frame` | 帧数据(纹理/位图引用 或 共享内存句柄) | 上屏到 `PreviewCanvas`（对应上游 `videoEngine` 帧回调）|
| `playhead_changed` | `{ frame }` | 播放时更新 `activeFrame`（上游 playheadState 由 engine 推进）|
| `media_changed` | `{}` 或增量 | 重取 `get_media`（缩略图/波形就绪、导入完成、生成完成）|
| `media_thumbnails` | `{ assetId, sprite/samples }` | 更新 timeline clip 缩略图/波形缓存（上游 `MediaVisualCache`）|
| `generation_progress` | `{ assetId, status, ... }` | 更新生成中 overlay / 失败态 |
| `export_progress` | `{ progress, ... }` | 导出进度 UI |
| `settings_mismatch` | `{ ... }` | 弹导入设置不匹配对话框（上游 `pendingSettingsMismatch`，`EditorViewModel.swift:221`）|

### 11.3 缩略图 / 波形 / 帧 的传输

上游 `MediaVisualCache`(`Timeline/MediaVisualCache.swift`)在内存持 sprite 网格 + PCM 降采样。OpenTake：Rust 媒体层算好（ffmpeg 抽帧 sprite + symphonia 波形），经 `media_thumbnails` event 或专门 command 传给前端（图片用 data-url / blob / 共享内存）。前端缓存后供 TimelineCanvas 的 `drawClip` 使用。**预览帧** `preview_frame` 优先共享内存/零拷贝（WebGL 纹理）以满足 scrub 实时性。

### 11.4 命令节流（复刻上游 debounce）

上游对快速重建做合并（`notifyTimelineChangedDebounced` 120ms，`EditorViewModel.swift:315-324`），对 scrub 用 `interactiveScrub` 模式只更 `playheadState` 不提交（`:261-265`）。前端：scrub/拖拽过程中**本地乐观更新 UI 态**（playhead、ghost），**松手才发 `edit_apply`**；连续属性拖拽（ScrubbableNumberField onChanged）可节流或仅在 onCommit 发命令。

---

## 12. 数据模型镜像（TS 类型）

> 来源:`Models/Timeline.swift` / `Keyframe.swift` / `ClipType.swift`。**字段名与 Rust serde / 工程 JSON 保持一致**(ARCHITECTURE §4)。前端 TS 类型 = 镜像反序列化目标。**前端不实现派生算法**(在 Rust),但需要少量纯 UI 派生(clip rect / 标签),可读这些字段。

```ts
type ClipType = 'video' | 'audio' | 'image' | 'text' | 'lottie';
type Interpolation = 'linear' | 'hold' | 'smooth';

interface Timeline {              // Timeline.swift:9-23
  fps: number;                    // 默认 30
  width: number;                  // 默认 1920
  height: number;                 // 默认 1080
  settingsConfigured: boolean;
  tracks: Track[];
}

interface Track {                 // Timeline.swift:25-59
  id: string;
  type: ClipType;
  muted: boolean;                 // 默认 false
  hidden: boolean;                // 默认 false
  syncLocked: boolean;            // 默认 true
  clips: Clip[];
  // displayHeight 不在 JSON, 前端 UI 态(默认 50, 范围 32..200)
}

interface Keyframe<V> {           // Keyframe.swift:7-11
  frame: number;                  // ★ 存储用 clip 相对偏移
  value: V;
  interpolationOut: Interpolation; // 默认 smooth
}
interface KeyframeTrack<V> { keyframes: Keyframe<V>[]; }
interface AnimPair { a: number; b: number; } // 位置(x,y)/缩放(w,h), Keyframe.swift:53-63

interface Transform {             // Timeline.swift:364-498
  centerX: number; centerY: number; // 默认 0.5/0.5
  width: number; height: number;    // 默认 1/1 (归一画布比例)
  rotation: number;                 // 度, 顺时针正
  flipHorizontal: boolean; flipVertical: boolean;
}
interface Crop {                  // Timeline.swift:501-510 (归一边距 0..1)
  left: number; top: number; right: number; bottom: number;
}

interface Clip {                  // Timeline.swift:75-117
  id: string;
  mediaRef: string;               // = asset id, 永不存路径
  mediaType: ClipType;            // 默认 video
  sourceClipType: ClipType;       // 色彩用, 默认 video
  startFrame: number;
  durationFrames: number;
  trimStartFrame: number;         // 默认 0
  trimEndFrame: number;           // 默认 0
  speed: number;                  // 默认 1.0
  volume: number;                 // 默认 1.0 (线性)
  fadeInFrames: number; fadeOutFrames: number; // 默认 0
  fadeInInterpolation: Interpolation;  // 默认 linear
  fadeOutInterpolation: Interpolation; // 默认 linear
  opacity: number;                // 默认 1.0
  transform: Transform;
  crop: Crop;
  linkGroupId?: string;           // A/V 链接组
  captionGroupId?: string;
  textContent?: string;           // text clip
  textStyle?: TextStyle;          // (见 TextStyle.swift)
  opacityTrack?: KeyframeTrack<number>;
  positionTrack?: KeyframeTrack<AnimPair>;
  scaleTrack?: KeyframeTrack<AnimPair>;
  rotationTrack?: KeyframeTrack<number>;
  cropTrack?: KeyframeTrack<Crop>;
  volumeTrack?: KeyframeTrack<number>; // 值是 dB
}
```

**前端需要的派生（纯 UI，读字段即可）**：
- `endFrame = startFrame + durationFrames`（`Timeline.swift:119`）。
- clip 时长时间码（标签栏用）。
- 是否 linked（`linkGroupId != null` → 标签下划线）。
- **关键帧偏移↔绝对帧**:存储是相对偏移,绘制 clip 上的关键帧标记要 `+startFrame`（`ClipRenderer.swift:165-169`）;Inspector 关键帧控件用绝对帧。
- 采样/插值(`*_at`、smoothstep、fade)**全部在 Rust 算**;前端预览帧来自 Rust,Inspector 显示的"当前帧值"可由 Rust 提供或前端按同公式算(若需即时反馈)。**若前端算,必须逐字复刻** `KeyframeTrack.sample`(端点 clamp 无外插 + 按左端点 interpolationOut，`Keyframe.swift:231-250`)、`smoothstep(t)=t*t*(3-2t)`(`:40`)、`fadeMultiplier`(in/out 取 min，`Timeline.swift:211-226`)。

**VolumeScale**（音量 dB↔线性，`Models` 内 `VolumeScale`）：floor=-60dB（→线性 0 硬截断）、ceiling=15dB；`dbFromLinear`/`linearFromDb`。Inspector/橡皮筋显示用（`ClipRenderer.swift:236,314`）。前端需取这两个常量与转换（从 Rust 取或复刻）。

---

## 13. 实施清单与 1:1 验收方式

### 13.1 落地顺序（建议）

1. **设计令牌**:把 §1 全部写成 `web/src/styles/tokens.css`(CSS 变量)。**这是地基**,先做,后续组件只引用变量。配 `lib/theme.ts` 导出数值常量(给 canvas 绘制用,canvas 不能用 CSS 变量)。
2. **PanelShell + EditorSplit + 三 preset 布局**（§2）：先把五面板框架 + surface 卡片 + 沟槽 + 焦点环 + 三种布局的初始分割位搭好。用真实 Rust 镜像或 mock Timeline 填充。
3. **Zustand store**（§10）：建 `useProjectStore`(镜像) + `useEditorUiStore`(UI 态) + 派生选择器。接 `timeline_changed`/`get_timeline`。
4. **Toolbar**（§4）：相对简单，先做，验证令牌与按钮样式。
5. **TimelineContainer + 几何 + 刻度 + 轨道头 + clip 渲染**（§5.1-5.5）：Canvas 2D 绘制，**先把静态渲染对拍像素**（给定 timeline/zoom，clip/刻度/轨道位置与上游一致）。
6. **Timeline 手势**（§5.8 + §9）：playhead/scrub → 选择 → 拖移/落轨 → trim → 缩放/平移 → 磁吸 → razor/split → 关键帧/淡变/音量 → 右键菜单 → 拖放。逐组实现 + 逐条对拍。
7. **Inspector**（§6）：四态 + tab + ScrubbableNumberField + 关键帧面板。
8. **Preview**（§8）：tab + 画布(接 `preview_frame`) + transport + scrub + 设置 + Transform/Crop overlay。
9. **MediaPanel**（§7）：标签轨 + Media/Captions/Music tab + 网格 + 拖拽/选择/右键。
10. **键盘快捷键全表**（§9.6）+ 菜单（§2.9）。
11. **全面板截图对拍 + 行为对拍**。

### 13.2 1:1 验收清单（逐项打勾）

**A. 视觉（截图对拍，1600×1000）**
- [ ] 五面板布局(default/media/vertical)的分栏比例、沟槽 5px、surface 圆角 6px、焦点环与上游一致。
- [ ] 所有间距/字号/圆角/颜色取自令牌且与 §1 数值一致（抽查每面板 ≥5 处）。
- [ ] Toolbar 按钮图标/分隔/缩放滑块(对数)外观一致。
- [ ] 时间线:刻度间隔/次刻度/标签、轨道头(色条/标签/三图标/区分隔粗线)、clip(底色/左色条/标签/trim 手柄/波形/缩略图/音量橡皮筋/淡变/关键帧菱形)、playhead(红线+下三角)、磁吸黄虚线、marquee 白虚线、新轨黄线、razor 橙虚线 —— 逐项与上游同。
- [ ] Inspector 四态、tab 下划线、AI Edit 渐变、各字段、关键帧泳道一致。
- [ ] MediaPanel 标签轨(选中竖 capsule + hover 标签)、网格、面包屑一致。
- [ ] Preview tab(下划线色)、transport 按钮、scrub(hover 变粗)、设置 badge、overlay 一致。

**B. 行为（逐条对拍 §9）**
- [ ] §9.1 选择全 8 条。
- [ ] §9.2 拖放全 7 条（含落回原位不发命令、跨轨类型钳制、linked 跟随）。
- [ ] §9.3 trim 全 7 条（含最小 1 帧、image 自由延长、linked 传播）。
- [ ] §9.4 razor/split/关键帧/淡变/音量全 8 条。
- [ ] §9.5 playhead/scrub/缩放全 9 条（含光标锚定缩放、灵敏度常量）。
- [ ] §9.6 快捷键全表逐键。
- [ ] §9.7 hover/游标全表。
- [ ] §9.8 右键菜单项与分组逐字（§5.10）。

**C. 几何（单测）**
- [ ] geometry 纯函数（clipRect/frameAt/xForFrame/trackY/dropTargetAt/insertionLineY）对给定输入与上游公式输出一致。
- [ ] 刻度间隔/次刻度选择算法输出一致。
- [ ] 磁吸 findSnap（含黏滞 1.5×、playhead 1.5× 阈值）行为一致。

**D. 状态/契约**
- [ ] 镜像只由 `timeline_changed`→`get_timeline` 更新；前端无任何直接改 timeline 的路径。
- [ ] 每个编辑手势映射到正确 `edit_apply` 命令（§11.1）。
- [ ] 持久化键（layoutPreset/三面板可见性/keyframes）跨会话保留。

### 13.3 关键陷阱备忘（防止 1:1 偏差）

1. **帧↔秒用截断**（`Int(s*fps)`），非四舍五入（ARCHITECTURE §4、MODULE-PORT-MAP）。
2. **浮点 round 用「四舍五入 .5 远离 0」**（= Rust `f64::round`），如 `sourceFramesConsumed`、缩放后关键帧位（`Timeline.swift:122,287`）。
3. **clip rect 上下各留 2px**（`y+2, height-4`）—— 别漏（`TimelineGeometry.swift:64-67`）。
4. **第一轨前有 60px 顶部 drop zone**（在刻度 24 之下）（`TimelineGeometry.swift:41`）。
5. **trim 手柄 4px、磁吸 8px、拖拽阈值 3px、轨道 resize 区 6px、insert 阈值 10px** —— 逐个照搬常量。
6. **磁吸黏滞 1.5×、playhead 优先 1.5×** —— 别用单一阈值（`SnapEngine.swift:67,82-83`）。
7. **缩放对数滑块 + 光标/playhead 锚定** —— scrub 滑块行程要均匀（`ToolbarView.swift:50-53`）。
8. **selection linked 默认开，Option 为单次关闭**（`TimelineInputController.swift:86`）—— 选择/移动/trim/marquee 全受此影响。
9. **volume 关键帧画在橡皮筋上，其他关键帧画在 clip 底部**（`ClipRenderer.swift:162-169`）。
10. **clip 标签 = 名字 + 双空格 + 时长时间码，linked 名字加下划线**（`ClipRenderer.swift:598-609`）。
11. **playhead/snap/刻度是「相对可视区」叠加**（按 scrollLeft 偏移），不是画在 document 全宽（`PlayheadOverlay.swift:51`、`TimelineRuler` 绘制矩形用 scrollOffset）。
12. **轨道 displayHeight 不持久化**（开工程重置 50）—— 当 UI 态。
13. **面板点击副作用**:点 media 清 clip 选择,点 timeline 清资产选择（`EditorWindowController.swift:188-189`）。
14. **文本输入聚焦时全局快捷键失效**（`EditorWindowController.swift:39-41`）。
15. **SF Symbols 必须换等价 SVG 图标**（§3.3），颜色用 currentColor + 上游 foregroundStyle。
16. **无触觉反馈**（上游磁吸触发 `NSHapticFeedbackManager`，`SnapEngine.swift:93`）—— 前端忽略，不要因此漏掉磁吸逻辑本身。

---

## 附:本规格的源码证据索引(便于核对)

| 主题 | 上游文件 |
|---|---|
| 设计令牌 | `UI/AppTheme.swift` |
| 布局常量 | `Utilities/Constants.swift` |
| 五面板/preset/shell/焦点环 | `Editor/EditorView.swift` |
| 窗口/快捷键/聚焦/菜单动作 | `Editor/EditorWindowController.swift` |
| 主菜单/快捷键定义 | `App/MainMenu.swift` |
| 标题栏 | `Editor/TitleBarView.swift` |
| 工具条 | `Toolbar/ToolbarView.swift` |
| 时间线容器/滚动 | `Timeline/TimelineContainerView.swift` |
| 时间线几何 | `Timeline/TimelineGeometry.swift` |
| 时间线绘制/叠加/右键/拖放 | `Timeline/TimelineView.swift` |
| 时间线手势/缩放/命中 | `Timeline/TimelineInputController.swift` |
| clip 渲染 | `Timeline/ClipRenderer.swift` |
| 刻度 | `Timeline/TimelineRuler.swift` |
| playhead | `Timeline/PlayheadOverlay.swift` |
| 磁吸 | `Timeline/SnapEngine.swift` + `SnapIndicatorOverlay.swift` |
| 拖拽状态 | `Timeline/DragState.swift` |
| 轨道头 | `Timeline/TimelineHeaderView.swift` |
| Inspector | `Inspector/InspectorView.swift` + `Components/` + `Keyframes/KeyframesLane.swift` |
| 可拖拽数字字段 | `Inspector/Components/ScrubbableNumberField.swift` |
| Media 面板 | `MediaPanel/MediaPanelView.swift` + `MediaTab/` |
| Captions | `MediaPanel/CaptionsTab/CaptionTab.swift` |
| Preview | `Preview/PreviewContainerView.swift` + `PreviewTab.swift` + overlays |
| 数据模型 | `Models/Timeline.swift` / `Keyframe.swift` / `ClipType.swift` |
| 小组件 | `UI/HoverHighlight.swift` / `CapsuleButton.swift` |
| 目标架构(Tauri/React/Zustand/命令-事件) | `OpenTake/docs/ARCHITECTURE.md` |
