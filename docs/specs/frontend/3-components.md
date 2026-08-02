# 组件地图

### 3.1 顶层装配（上游视图树 → React 组件树）

```
AppShell (Tauri window chrome)
├─ TitleBar
│   ├─ TitleBarLeading      ← TitleBarView.swift:3  (Agent 切换)
│   └─ TitleBarTrailing     ← TitleBarView.swift:22 (Export, Avatar)
└─ EditorSplit              ← EditorView / EditorSplitViewController
    ├─ AgentPanel           ← AgentPanelView      (实时会话、生成确认与草稿交接)
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
| `AIEditView` | `AiEditTab` | Inspector 的 AI Edit tab | 生成建议、审阅、拒绝、一次命令应用、撤销/取消/失败恢复 |
| `ScrubbableNumberField` | `ScrubbableNumberField` | 行内 | 拖拽改值/点击输入 |
| `InspectorPositionFields` | `PositionFields` | 行内双字段 | X/Y 拖拽 |
| `KeyframesPanel`/`KeyframesLane` | `KeyframesPanel` | Inspector 右半 | 关键帧 ruler+lane 拖拽/右键 |
| `MediaPanelView` | `MediaPanel` | 左标签轨(38) + tab 内容 | tab 切换、hover 标签 |
| `MediaTab` | `MediaTab` | toolbar + grid | 导入/生成/搜索/筛选/拖拽/选择/右键 |
| `AssetThumbnailView` | `AssetTile` | grid cell | 拖到时间线、双击、右键 |
| `FolderTileView` | `FolderTile` | grid cell | 双击进入、拖入、重命名 |
| `CaptionTab` | `CaptionTab` | 表单 + 预览 | 字幕生成参数 |
| `MusicTab` | `MusicTab` | 项目音乐 + 全局音乐库 + 生成入口 | 搜索、导入、放入时间线；生成前把模型/成本确认草稿交给 Agent |
| `TransitionTab` | `TransitionTab` | 相邻可视片段切点编辑器 | 交叉溶解、时长钳制、应用/移除、撤销；时间线显示切点标记 |
| `PreviewContainerView` | `PreviewContainer` | tab栏 + 画布 + scrub + transport | 见§8 |
| `PreviewView`/`TransformOverlayView`/`CropOverlayView` | `PreviewCanvas`+`TransformOverlay`+`CropOverlay` | 画布层叠 | 拖动变换/裁剪手柄 |
| `CapsuleButtonStyle` | `CapsuleButton` | —— | hover/press |
| `HoverHighlight` | `useHoverHighlight`/`<HoverArea>` | —— | hover 背景渐显 |
| `GeneratingOverlay` | `GeneratingOverlay` | 覆盖 | 生成中 |

### 3.2.1 当前精确所有权与实时边界

以下条目是本组件地图的可执行所有权索引。组件不能维护第二套编辑状态；所有持久化编辑都进入共享命令链，预览与导出读取同一项目模型。

| 能力 | 精确 React 所有者 | 实时命令/状态边界 | 可见验收所有者 |
|---|---|---|---|
| AI Edit 建议与审阅 | `AiEditTab` → `web/src/components/inspector/AiEditTab.tsx` | `edit.setClipProperties` / `edit.undo`；AbortSignal 取消不提交命令 | `AiEditTab.test.tsx`、`Inspector.test.tsx` |
| 音乐浏览、导入与放置 | `MusicTab` → `web/src/components/media/MusicTab.tsx` | `addMediaToTimeline`、`libraryStore.importToProject`、`chatStore.composerDraft` | `MusicTab.test.tsx` |
| 转场选择与应用 | `TransitionTab` → `web/src/components/media/TransitionTab.tsx` | `setTransition` → Tauri `edit_apply` → Rust `EditCommand::SetTransition` | `TransitionTab.test.tsx`、`timelineOverlays.test.ts` |
| 画布变换覆盖层 | `TransformOverlay` → `web/src/components/preview/TransformOverlay.tsx` | `edit.setClipProperties`（一次指针提交对应一次撤销项） | `TransformOverlay.test.tsx` |
| 画布裁剪覆盖层 | `CropOverlay` → `web/src/components/preview/CropOverlay.tsx` | 静态裁剪用 `edit.setClipProperties`；动画裁剪用 `edit.upsertKeyframe` | `cropOverlay.test.ts`、`Preview.test.tsx` |

转场模型由 `crates/opentake-domain/src/transition.rs` 持久化；命令验证、相邻关系清理与撤销由 `crates/opentake-ops/src/command.rs` 所有；交叉溶解的预览/导出帧计划由 `crates/opentake-render/src/plan/build.rs` 所有。这样 UI 标记、保存重开、预览和导出不会各自形成不一致的旁路。

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
